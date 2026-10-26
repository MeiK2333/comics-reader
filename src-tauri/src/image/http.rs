//! Shared HTTP client and the platform-agnostic download pipeline.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};

use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;
use url::Url;

use crate::ech::{CloudflareResolver, EchSession, EchStatus};
use crate::error::{AppError, Result};
use crate::logging;

use super::cache::ImageFormat;
use super::platform::{Platform, RequestSpec};

/// Page images are usually a few megabytes; anything past this is treated as a
/// bad response rather than filling the cache with an accidental download.
const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;
/// Search and detail pages are small; this is only a guard against a runaway
/// response body.
const MAX_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// The ECH config is refreshed lazily after this long; the deadline is also
/// reported by `get_ech_status`.
const ECH_CONFIG_TTL: Duration = Duration::from_secs(10 * 60);
/// After a failed refresh, wait this long before trying the bootstrap again.
const ECH_REFRESH_RETRY: Duration = Duration::from_secs(60);
/// A host that rejected ECH but accepted plain TLS is retried with ECH after
/// this long, so a server or config change cannot disable ECH indefinitely.
const ECH_FALLBACK_TTL: Duration = Duration::from_secs(10 * 60);

const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36";
const ACCEPT_VALUE: &str = "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8";

/// Proxy settings as entered in the settings page.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HttpProxyConfig {
    host: String,
    port: u16,
}

impl HttpProxyConfig {
    /// Validate the two form fields and turn them into the URL reqwest expects.
    fn normalized_url(&self) -> Result<String> {
        let host = self.host.trim();
        if host.is_empty() {
            return Err(invalid_proxy("host must not be empty"));
        }
        if host.contains("://")
            || host.contains('/')
            || host.contains('\\')
            || host.contains('@')
            || host.contains('?')
            || host.contains('#')
            || host.chars().any(char::is_whitespace)
        {
            return Err(invalid_proxy(
                "host must be a hostname or IP address without a scheme, path, credentials or whitespace",
            ));
        }
        if self.port == 0 {
            return Err(invalid_proxy("port must be between 1 and 65535"));
        }

        // `url` expects IPv6 literals in brackets; accept the bare form too so
        // users can paste `::1` from another proxy tool.
        let authority = if host.contains(':') && !host.starts_with('[') {
            format!("[{host}]")
        } else {
            host.to_owned()
        };
        let url = Url::parse(&format!("http://{authority}:{}", self.port))
            .map_err(|_| invalid_proxy("host is not a valid hostname or IP address"))?;

        Ok(url.to_string())
    }
}

fn invalid_proxy(reason: impl Into<String>) -> AppError {
    AppError::InvalidInput {
        field: "proxy",
        reason: reason.into(),
    }
}

/// Shared HTTP client and the proxy currently applied to it. Building the
/// client lazily keeps startup infallible while still reusing pooled
/// connections across the many images of a chapter.
#[derive(Default)]
pub(crate) struct HttpClient {
    state: Mutex<ClientState>,
    /// Serializes lazy refreshes so concurrent requests do not bootstrap the
    /// same ECH config more than once.
    refresh: tokio::sync::Mutex<()>,
}

#[derive(Default)]
struct ClientState {
    /// Normalized `http://host:port/` URL, or `None` for reqwest's default.
    proxy: Option<String>,
    normal_client: Option<reqwest::Client>,
    ech: Option<EchRuntime>,
    /// Incremented on every enable, disable and successful refresh. A refresh
    /// that started before a state change must not resurrect the old runtime.
    ech_generation: u64,
    /// Incremented when a command starts changing the ECH state. A bootstrap
    /// that finishes after a newer command must be discarded.
    ech_control_generation: u64,
    /// `https://host:port` keys that rejected ECH but accepted plain TLS. The
    /// cache survives an ECH config refresh but is cleared when ECH is toggled.
    ech_fallback_until: HashMap<String, Instant>,
}

/// The two clients used while ECH is enabled. Both resolve targets through
/// Cloudflare DoH and ignore any proxy; only the TLS layer differs.
struct EchRuntime {
    session: Arc<EchSession>,
    ech_client: reqwest::Client,
    plain_client: reqwest::Client,
    expires_at: SystemTime,
    refresh_after: SystemTime,
}

impl HttpClient {
    /// Ordinary client, used for HTTP requests and whenever ECH is off.
    pub(super) fn get(&self) -> Result<reqwest::Client> {
        let mut state = self.lock()?;

        if let Some(client) = state.normal_client.as_ref() {
            return Ok(client.clone());
        }

        let client = build_client(state.proxy.as_deref())?;
        state.normal_client = Some(client.clone());
        Ok(client)
    }

    /// Apply the user's proxy. Building eagerly means an address reqwest
    /// cannot use is rejected before it can replace a working proxy.
    pub(crate) fn set_proxy(&self, proxy: Option<HttpProxyConfig>) -> Result<()> {
        let proxy = proxy.map(|proxy| proxy.normalized_url()).transpose()?;
        let mut state = self.lock()?;

        if state.ech.is_some() {
            if proxy.is_some() {
                return Err(invalid_proxy(
                    "cannot enable the HTTP proxy while ECH is enabled",
                ));
            }
            state.proxy = None;
            return Ok(());
        }

        let client = build_client(proxy.as_deref())?;
        state.proxy = proxy;
        state.normal_client = Some(client);
        Ok(())
    }

    /// Start a user-visible ECH state change. The returned generation must be
    /// passed to [`Self::enable_ech_if_current`] after the (slow) bootstrap so
    /// a newer disable/enable can win the race.
    pub(crate) fn begin_ech_control(&self) -> Result<u64> {
        let mut state = self.lock()?;
        state.ech_control_generation = state.ech_control_generation.wrapping_add(1);
        Ok(state.ech_control_generation)
    }

    fn ech_control_is_current(&self, control: u64) -> Result<bool> {
        Ok(self.lock()?.ech_control_generation == control)
    }

    /// Install the ECH runtime if no newer control command has been issued.
    ///
    /// A previously configured proxy is cleared because both ECH clients are
    /// built with `no_proxy`.
    pub(crate) fn enable_ech_if_current(
        &self,
        control: u64,
        session: Arc<EchSession>,
    ) -> Result<()> {
        if !self.ech_control_is_current(control)? {
            return Ok(());
        }

        let resolver = session.resolver();
        let ech_client = build_ech_client(&session, resolver.clone(), true)?;
        let plain_client = build_ech_client(&session, resolver, false)?;
        let normal_client = build_client(None)?;
        let now = SystemTime::now();

        let mut state = self.lock()?;
        if state.ech_control_generation != control {
            return Ok(());
        }
        state.proxy = None;
        state.normal_client = Some(normal_client);
        state.ech_generation = state.ech_generation.wrapping_add(1);
        state.ech_fallback_until.clear();
        state.ech = Some(EchRuntime {
            session,
            ech_client,
            plain_client,
            expires_at: now + ECH_CONFIG_TTL,
            refresh_after: now + ECH_CONFIG_TTL,
        });
        Ok(())
    }

    /// Drop the ECH runtime; the ordinary client is reused on the next send.
    ///
    /// Bumping the control generation also makes any in-flight bootstrap
    /// discard its result instead of resurrecting ECH.
    pub(crate) fn disable_ech(&self) -> Result<()> {
        let mut state = self.lock()?;
        state.ech = None;
        state.ech_fallback_until.clear();
        state.ech_generation = state.ech_generation.wrapping_add(1);
        state.ech_control_generation = state.ech_control_generation.wrapping_add(1);
        Ok(())
    }

    /// Re-bootstrap the shared ECH config once its TTL has elapsed.
    ///
    /// The refresh is lazy and runs on the first HTTPS request after the
    /// deadline. A failed bootstrap keeps the old runtime (it may still work)
    /// and postpones the next attempt.
    async fn refresh_ech_if_due(&self) {
        if !self.refresh_is_due() {
            return;
        }

        let _guard = self.refresh.lock().await;

        let Some((generation, bootstrap_doh)) = self.refresh_target() else {
            return;
        };

        let session = match EchSession::bootstrap(&bootstrap_doh).await {
            Ok(session) => Arc::new(session),
            Err(error) => {
                logging::warn("ech", format!("failed to refresh the ECH config: {error}"));
                eprintln!("failed to refresh the ECH config: {error}");
                self.postpone_refresh(generation);
                return;
            }
        };

        if let Err(error) = self.install_refreshed_ech(generation, session) {
            logging::warn(
                "ech",
                format!("failed to install the refreshed ECH runtime: {error}"),
            );
            eprintln!("failed to install the refreshed ECH runtime: {error}");
            self.postpone_refresh(generation);
        }
    }

    fn refresh_is_due(&self) -> bool {
        let Ok(state) = self.lock() else {
            return false;
        };
        state
            .ech
            .as_ref()
            .is_some_and(|runtime| runtime.refresh_after <= SystemTime::now())
    }

    fn refresh_target(&self) -> Option<(u64, String)> {
        let state = self.lock().ok()?;
        let runtime = state.ech.as_ref()?;
        if runtime.refresh_after > SystemTime::now() {
            return None;
        }
        Some((
            state.ech_generation,
            runtime.session.bootstrap_doh().to_owned(),
        ))
    }

    fn install_refreshed_ech(&self, generation: u64, session: Arc<EchSession>) -> Result<()> {
        let resolver = session.resolver();
        let ech_client = build_ech_client(&session, resolver.clone(), true)?;
        let plain_client = build_ech_client(&session, resolver, false)?;
        let now = SystemTime::now();

        let mut state = self.lock()?;
        if state.ech_generation != generation || state.ech.is_none() {
            return Ok(());
        }

        state.ech_generation = state.ech_generation.wrapping_add(1);
        state.ech = Some(EchRuntime {
            session,
            ech_client,
            plain_client,
            expires_at: now + ECH_CONFIG_TTL,
            refresh_after: now + ECH_CONFIG_TTL,
        });
        Ok(())
    }

    fn postpone_refresh(&self, generation: u64) {
        let Ok(mut state) = self.lock() else {
            return;
        };
        if state.ech_generation != generation {
            return;
        }
        if let Some(runtime) = state.ech.as_mut() {
            runtime.refresh_after = SystemTime::now() + ECH_REFRESH_RETRY;
        }
    }

    pub(crate) fn ech_status(&self) -> Result<EchStatus> {
        let state = self.lock()?;
        let Some(runtime) = state.ech.as_ref() else {
            return Ok(EchStatus {
                enabled: false,
                ready: false,
                config_domain: None,
                bootstrap_doh: None,
                expires_at: None,
            });
        };

        let expires_at = runtime
            .expires_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_secs());

        Ok(EchStatus {
            enabled: true,
            ready: true,
            config_domain: Some(runtime.session.config_domain().to_owned()),
            bootstrap_doh: Some(runtime.session.bootstrap_doh().to_owned()),
            expires_at,
        })
    }

    fn ech_fallback_is_active(&self, key: &str) -> Result<bool> {
        let mut state = self.lock()?;
        match state.ech_fallback_until.get(key).copied() {
            Some(until) if until > Instant::now() => Ok(true),
            Some(_) => {
                state.ech_fallback_until.remove(key);
                Ok(false)
            }
            None => Ok(false),
        }
    }

    fn remember_ech_fallback(&self, key: &str) -> Result<()> {
        let now = Instant::now();
        let mut state = self.lock()?;
        // Expired keys are only useful for diagnosing that the host was tried;
        // prune them here so a long-running app cannot accumulate stale hosts.
        state.ech_fallback_until.retain(|_, until| *until > now);
        state
            .ech_fallback_until
            .insert(key.to_owned(), now + ECH_FALLBACK_TTL);
        Ok(())
    }

    fn forget_ech_fallback(&self, key: &str) {
        if let Ok(mut state) = self.lock() {
            state.ech_fallback_until.remove(key);
        }
    }

    /// Send a request through the ECH client when it is enabled, retrying once
    /// with plain TLS when ECH cannot complete the connection.
    ///
    /// A host that rejected ECH is remembered for [`ECH_FALLBACK_TTL`] after a
    /// successful plain request. Later requests then skip the doomed ECH
    /// handshake while keeping the plain client's connection pool.
    ///
    /// `build` must be able to construct the request again; the fallback path
    /// calls it a second time. Plain HTTP always uses the ordinary client so
    /// ECH cannot change its DNS or proxy behaviour.
    pub(super) async fn send<F>(&self, url: &Url, build: F) -> Result<reqwest::Response>
    where
        F: Fn(&reqwest::Client) -> reqwest::RequestBuilder,
    {
        if url.scheme() == "https" {
            self.refresh_ech_if_due().await;
        }

        let fallback_key = ech_fallback_key(url);
        let normal_client = self.get()?;
        let ech_clients = {
            let state = self.lock()?;
            state
                .ech
                .as_ref()
                .map(|runtime| (runtime.ech_client.clone(), runtime.plain_client.clone()))
        };

        if url.scheme() == "https"
            && let Some((ech_client, plain_client)) = ech_clients
        {
            let skip_ech = match fallback_key.as_deref() {
                Some(key) => self.ech_fallback_is_active(key)?,
                None => false,
            };

            if skip_ech {
                return match build(&plain_client).send().await {
                    Ok(response) => Ok(response),
                    Err(error) => {
                        if let Some(key) = fallback_key.as_deref() {
                            self.forget_ech_fallback(key);
                        }
                        Err(network_error(url, error))
                    }
                };
            }

            match build(&ech_client).send().await {
                Ok(response) => return Ok(response),
                Err(error) if is_fallback_error(&error) => {
                    logging::warn(
                        "ech",
                        format!("ECH connection to {url} failed; retrying with plain TLS: {error}"),
                    );
                    match build(&plain_client).send().await {
                        Ok(response) => {
                            if let Some(key) = fallback_key.as_deref() {
                                match self.remember_ech_fallback(key) {
                                    Ok(()) => logging::info(
                                        "ech",
                                        format!("ECH unavailable for {key}; using plain TLS"),
                                    ),
                                    Err(error) => logging::warn(
                                        "ech",
                                        format!(
                                            "failed to cache the ECH fallback for {key}: {error}"
                                        ),
                                    ),
                                }
                            }
                            return Ok(response);
                        }
                        Err(error) => {
                            if let Some(key) = fallback_key.as_deref() {
                                self.forget_ech_fallback(key);
                            }
                            return Err(network_error(url, error));
                        }
                    }
                }
                Err(error) => return Err(network_error(url, error)),
            }
        }

        build(&normal_client)
            .send()
            .await
            .map_err(|error| network_error(url, error))
    }

    fn lock(&self) -> Result<MutexGuard<'_, ClientState>> {
        self.state
            .lock()
            .map_err(|_| AppError::Unexpected("HTTP client lock was poisoned".into()))
    }
}

fn base_builder() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .default_headers(base_headers())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(10))
}

fn build_client(proxy: Option<&str>) -> Result<reqwest::Client> {
    let mut builder = base_builder();

    if let Some(proxy) = proxy {
        // `ClientBuilder::proxy` also disables reqwest's automatic system
        // proxy, so a configured address is the only proxy in use.
        builder =
            builder.proxy(reqwest::Proxy::all(proxy).map_err(|error| {
                invalid_proxy(format!("proxy address could not be used: {error}"))
            })?);
    }

    builder
        .build()
        .map_err(|error| AppError::Unexpected(format!("failed to build the HTTP client: {error}")))
}

fn build_ech_client(
    session: &Arc<EchSession>,
    resolver: Arc<CloudflareResolver>,
    ech: bool,
) -> Result<reqwest::Client> {
    let tls = if ech {
        session.reqwest_ech_tls()?
    } else {
        session.reqwest_plain_tls()?
    };

    base_builder()
        .use_preconfigured_tls(tls)
        .dns_resolver(resolver)
        .no_proxy()
        .build()
        .map_err(|error| {
            AppError::Unexpected(format!("failed to build the ECH HTTP client: {error}"))
        })
}

/// Only transport-level failures are retried. HTTP status errors, body errors
/// and decode errors have already reached the server and must not be repeated.
fn is_fallback_error(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout()
}

/// Cache key for an HTTPS origin. The port is included so a service on a
/// non-default port cannot inherit another service's fallback decision.
fn ech_fallback_key(url: &Url) -> Option<String> {
    if url.scheme() != "https" {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    let port = url.port_or_known_default().unwrap_or(443);
    Some(format!("{host}:{port}"))
}

fn network_error(url: &Url, error: reqwest::Error) -> AppError {
    AppError::Network {
        url: url.to_string(),
        reason: error.to_string(),
    }
}

fn base_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
    headers.insert(ACCEPT, HeaderValue::from_static(ACCEPT_VALUE));
    headers
}

pub(super) struct Download {
    pub(super) bytes: Vec<u8>,
    pub(super) format: ImageFormat,
}

pub(super) async fn download_image(
    http: &HttpClient,
    platform: &dyn Platform,
    url: &Url,
) -> Result<Download> {
    let mut response = http
        .send(url, |client| {
            platform.configure(client.get(url.as_str()), url)
        })
        .await?;

    let status = response.status();
    if !status.is_success() {
        return Err(AppError::HttpStatus {
            url: url.to_string(),
            status: status.as_u16(),
        });
    }

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);

    if let Some(length) = response.content_length()
        && length > MAX_IMAGE_BYTES
    {
        return Err(AppError::TooLarge {
            url: url.to_string(),
            limit: MAX_IMAGE_BYTES,
        });
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| AppError::Network {
        url: url.to_string(),
        reason: error.to_string(),
    })? {
        if bytes.len() as u64 + chunk.len() as u64 > MAX_IMAGE_BYTES {
            return Err(AppError::TooLarge {
                url: url.to_string(),
                limit: MAX_IMAGE_BYTES,
            });
        }
        bytes.extend_from_slice(&chunk);
    }

    if bytes.is_empty() {
        return Err(AppError::EmptyResponse {
            url: url.to_string(),
        });
    }

    // Trust the magic bytes first: a mislabelled `Content-Type` would otherwise
    // give the cached file an extension the webview cannot render.
    let format = ImageFormat::detect(&bytes)
        .or_else(|| {
            content_type
                .as_deref()
                .and_then(ImageFormat::from_content_type)
        })
        .ok_or_else(|| AppError::NotImage {
            url: url.to_string(),
            content_type: content_type.unwrap_or_else(|| "unknown".into()),
        })?;

    Ok(Download { bytes, format })
}

/// Fetch an HTML or JSON document for a platform search/detail request.
///
/// Mirrors are tried only for errors that another host can plausibly fix: a
/// transport failure, an empty body, or a 403/429/5xx. Parse errors are final
/// because the body already arrived from a host that answered.
pub(super) async fn fetch_document(
    http: &HttpClient,
    platform: &dyn Platform,
    spec: &RequestSpec,
) -> Result<Vec<u8>> {
    let mut last_error = None;

    for url in spec.urls() {
        match fetch_document_once(http, platform, spec, url).await {
            Ok(body) => {
                platform.note_request_outcome(url, true);
                return Ok(body);
            }
            Err(error) if is_mirror_error(&error) => {
                platform.note_request_outcome(url, false);
                logging::warn(
                    "http",
                    format!("request to {url} failed ({error}); trying the next mirror"),
                );
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }

    Err(last_error.unwrap_or_else(|| AppError::Unexpected("request has no URL to try".into())))
}

async fn fetch_document_once(
    http: &HttpClient,
    platform: &dyn Platform,
    spec: &RequestSpec,
    url: &Url,
) -> Result<Vec<u8>> {
    let mut response = http
        .send(url, |client| {
            platform
                .configure(client.get(url.as_str()), url)
                .headers(spec.headers.clone())
                .header(ACCEPT, spec.accept)
        })
        .await?;

    let status = response.status();
    if !status.is_success() {
        return Err(AppError::HttpStatus {
            url: url.to_string(),
            status: status.as_u16(),
        });
    }

    if let Some(length) = response.content_length()
        && length > MAX_DOCUMENT_BYTES
    {
        return Err(AppError::TooLarge {
            url: url.to_string(),
            limit: MAX_DOCUMENT_BYTES,
        });
    }

    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| AppError::Network {
        url: url.to_string(),
        reason: error.to_string(),
    })? {
        if bytes.len() as u64 + chunk.len() as u64 > MAX_DOCUMENT_BYTES {
            return Err(AppError::TooLarge {
                url: url.to_string(),
                limit: MAX_DOCUMENT_BYTES,
            });
        }
        bytes.extend_from_slice(&chunk);
    }

    if bytes.is_empty() {
        return Err(AppError::EmptyResponse {
            url: url.to_string(),
        });
    }

    Ok(bytes)
}

/// Whether trying the same request on another host could succeed.
pub(super) fn is_mirror_error(error: &AppError) -> bool {
    match error {
        AppError::Network { .. } | AppError::EmptyResponse { .. } => true,
        AppError::HttpStatus { status, .. } => *status == 403 || *status == 429 || *status >= 500,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxy(host: &str, port: u16) -> HttpProxyConfig {
        HttpProxyConfig {
            host: host.into(),
            port,
        }
    }

    #[test]
    fn base_headers_look_like_a_browser() {
        let headers = base_headers();
        assert_eq!(headers[USER_AGENT].to_str().unwrap(), USER_AGENT_VALUE);
        assert_eq!(headers[ACCEPT].to_str().unwrap(), ACCEPT_VALUE);
    }

    #[test]
    fn normalizes_ipv4_and_hostname_proxies() {
        assert_eq!(
            proxy("127.0.0.1", 7890).normalized_url().unwrap(),
            "http://127.0.0.1:7890/"
        );
        assert_eq!(
            proxy(" proxy.example.com ", 8080).normalized_url().unwrap(),
            "http://proxy.example.com:8080/"
        );
    }

    #[test]
    fn normalizes_ipv6_proxies() {
        assert_eq!(
            proxy("::1", 7890).normalized_url().unwrap(),
            "http://[::1]:7890/"
        );
        assert_eq!(
            proxy("[::1]", 7890).normalized_url().unwrap(),
            "http://[::1]:7890/"
        );
    }

    #[test]
    fn rejects_invalid_proxy_addresses() {
        for (host, port) in [
            ("", 7890),
            ("   ", 7890),
            ("http://127.0.0.1", 7890),
            ("127.0.0.1:7890", 7890),
            ("proxy.example.com/path", 7890),
            ("user@proxy.example.com", 7890),
            ("proxy example", 7890),
            ("127.0.0.1", 0),
        ] {
            let error = proxy(host, port).normalized_url().unwrap_err();
            assert!(
                matches!(error, AppError::InvalidInput { field: "proxy", .. }),
                "unexpected error for {host:?}: {error}"
            );
        }
    }

    #[test]
    fn set_proxy_replaces_and_clears_the_cached_client() {
        let client = HttpClient::default();

        client.set_proxy(Some(proxy("127.0.0.1", 7890))).unwrap();
        assert_eq!(
            client.lock().unwrap().proxy.as_deref(),
            Some("http://127.0.0.1:7890/")
        );

        client.set_proxy(None).unwrap();
        assert!(client.lock().unwrap().proxy.is_none());
    }

    #[test]
    fn rejected_proxy_keeps_the_previous_configuration() {
        let client = HttpClient::default();
        client.set_proxy(Some(proxy("127.0.0.1", 7890))).unwrap();

        let error = client
            .set_proxy(Some(proxy("http://bad", 7890)))
            .unwrap_err();
        assert!(matches!(
            error,
            AppError::InvalidInput { field: "proxy", .. }
        ));
        assert_eq!(
            client.lock().unwrap().proxy.as_deref(),
            Some("http://127.0.0.1:7890/")
        );
    }

    #[test]
    fn reports_ech_disabled_by_default() {
        let client = HttpClient::default();

        let status = client.ech_status().unwrap();

        assert!(!status.enabled);
        assert!(!status.ready);
        assert!(status.config_domain.is_none());
        assert!(status.bootstrap_doh.is_none());
        assert!(status.expires_at.is_none());
    }

    #[test]
    fn ech_fallback_key_is_scoped_to_the_https_origin() {
        assert_eq!(
            ech_fallback_key(&Url::parse("http://example.com/").unwrap()),
            None
        );
        assert_eq!(
            ech_fallback_key(&Url::parse("https://example.com/path").unwrap()).as_deref(),
            Some("example.com:443")
        );
        assert_eq!(
            ech_fallback_key(&Url::parse("https://EXAMPLE.com:8443/").unwrap()).as_deref(),
            Some("example.com:8443")
        );
    }

    #[test]
    fn caches_and_expires_ech_fallbacks() {
        let client = HttpClient::default();
        let key = "example.com:443";

        assert!(!client.ech_fallback_is_active(key).unwrap());
        client.remember_ech_fallback(key).unwrap();
        assert!(client.ech_fallback_is_active(key).unwrap());

        client
            .lock()
            .unwrap()
            .ech_fallback_until
            .insert(key.into(), Instant::now() - Duration::from_secs(1));
        assert!(!client.ech_fallback_is_active(key).unwrap());
        assert!(client.lock().unwrap().ech_fallback_until.is_empty());
    }

    #[test]
    fn disabling_ech_clears_cached_fallbacks() {
        let client = HttpClient::default();
        let key = "example.com:443";
        client.remember_ech_fallback(key).unwrap();

        client.disable_ech().unwrap();

        assert!(!client.ech_fallback_is_active(key).unwrap());
    }

    #[test]
    fn disabling_ech_invalidates_an_in_flight_enable() {
        let client = HttpClient::default();
        let control = client.begin_ech_control().unwrap();

        client.disable_ech().unwrap();

        assert!(!client.ech_control_is_current(control).unwrap());
    }

    #[tokio::test]
    async fn refresh_is_a_noop_without_ech() {
        let client = HttpClient::default();

        client.refresh_ech_if_due().await;

        assert!(!client.ech_status().unwrap().enabled);
    }

    /// Exercises the real runtime: ECH succeeds for nhentai, a non-Cloudflare
    /// host falls back to plain TLS, the config refreshes on demand, and the
    /// proxy is rejected while ECH is on.
    #[tokio::test]
    #[ignore = "performs live ECH requests"]
    async fn ech_client_fetches_and_falls_back() {
        let http = HttpClient::default();
        let session = Arc::new(
            crate::ech::EchSession::bootstrap("https://dns.alidns.com/dns-query")
                .await
                .unwrap(),
        );
        let control = http.begin_ech_control().unwrap();
        http.enable_ech_if_current(control, session).unwrap();

        let status = http.ech_status().unwrap();
        assert!(status.enabled);
        assert!(status.ready);
        assert_eq!(status.config_domain.as_deref(), Some("cloudflare-ech.com"));

        let nhentai = Url::parse("https://nhentai.net/").unwrap();
        let response = http
            .send(&nhentai, |client| client.get(nhentai.as_str()))
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);

        let qq = Url::parse("https://www.qq.com/").unwrap();
        let response = http
            .send(&qq, |client| client.get(qq.as_str()))
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert!(http.ech_fallback_is_active("www.qq.com:443").unwrap());

        // Force the deadline so the next HTTPS request re-runs the bootstrap
        // and swaps in a fresh runtime.
        let generation = http.lock().unwrap().ech_generation;
        {
            let mut state = http.lock().unwrap();
            state.ech.as_mut().unwrap().refresh_after = SystemTime::now() - Duration::from_secs(1);
        }
        let response = http
            .send(&nhentai, |client| client.get(nhentai.as_str()))
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        assert!(http.lock().unwrap().ech_generation > generation);
        // The fallback decision belongs to the origin, not the ECH runtime.
        assert!(http.ech_fallback_is_active("www.qq.com:443").unwrap());
        let response = http
            .send(&qq, |client| client.get(qq.as_str()))
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);

        let error = http.set_proxy(Some(proxy("127.0.0.1", 7890))).unwrap_err();
        assert!(matches!(
            error,
            AppError::InvalidInput { field: "proxy", .. }
        ));

        http.disable_ech().unwrap();
        assert!(!http.ech_status().unwrap().enabled);
        assert!(!http.ech_fallback_is_active("www.qq.com:443").unwrap());
        http.set_proxy(Some(proxy("127.0.0.1", 7890))).unwrap();
    }

    /// Full business path through ECH: nhentai search, detail, and the detail
    /// thumbnail (served from a separate Cloudflare host).
    #[tokio::test]
    #[ignore = "performs a live nhentai search, detail and image download"]
    async fn nhentai_search_detail_and_image_work_with_ech() {
        let http = HttpClient::default();
        let session = Arc::new(
            crate::ech::EchSession::bootstrap("https://dns.alidns.com/dns-query")
                .await
                .unwrap(),
        );
        let control = http.begin_ech_control().unwrap();
        http.enable_ech_if_current(control, session).unwrap();

        let platform = super::super::platform::find("nhentai").unwrap();

        let spec = platform.search_request("love", 1).unwrap();
        let body = fetch_document(&http, platform, &spec).await.unwrap();
        let search = platform.parse_search("love", 1, &body).unwrap();
        let item = search.result.first().expect("nhentai returned no results");

        let spec = platform.detail_request(&item.id).unwrap();
        let body = fetch_document(&http, platform, &spec).await.unwrap();
        let detail = platform.parse_detail(&item.id, &body).unwrap();
        assert!(!detail.title.is_empty());

        let thumb = Url::parse(&detail.thumb).unwrap();
        let download = download_image(&http, platform, &thumb).await.unwrap();
        assert!(!download.bytes.is_empty());
    }

    #[test]
    fn mirror_errors_are_the_ones_another_host_can_fix() {
        assert!(is_mirror_error(&AppError::Network {
            url: "https://example.com".into(),
            reason: "timeout".into(),
        }));
        assert!(is_mirror_error(&AppError::EmptyResponse {
            url: "https://example.com".into(),
        }));

        for status in [403u16, 429, 500, 502, 520] {
            assert!(is_mirror_error(&AppError::HttpStatus {
                url: "https://example.com".into(),
                status,
            }));
        }
        for status in [400u16, 401, 404, 410] {
            assert!(!is_mirror_error(&AppError::HttpStatus {
                url: "https://example.com".into(),
                status,
            }));
        }

        // A valid but unusable body is not a mirror problem for documents;
        // the image pipeline adds `notImage` separately.
        assert!(!is_mirror_error(&AppError::NotImage {
            url: "https://example.com".into(),
            content_type: "text/html".into(),
        }));
        assert!(!is_mirror_error(&AppError::TooLarge {
            url: "https://example.com".into(),
            limit: 1,
        }));
    }
}
