//! Proxy validation and HTTP client construction.

use std::sync::{Arc, MutexGuard};

use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;
use url::Url;

use crate::ech::{CloudflareResolver, EchSession};
use crate::error::{AppError, Result};
use crate::logging;

use super::{CONNECT_TIMEOUT, ClientState, HttpClient, REQUEST_TIMEOUT, TlsFallback};
pub(super) const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36";
pub(super) const ACCEPT_VALUE: &str =
    "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8";
/// Proxy settings as entered in the settings page.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HttpProxyConfig {
    pub(super) host: String,
    pub(super) port: u16,
}

impl HttpProxyConfig {
    /// Validate the two form fields and turn them into the URL reqwest expects.
    pub(super) fn normalized_url(&self) -> Result<String> {
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

pub(super) fn invalid_proxy(reason: impl Into<String>) -> AppError {
    AppError::InvalidInput {
        field: "proxy",
        reason: reason.into(),
    }
}

impl HttpClient {
    /// Send a request through the ECH client when it is enabled, falling back
    /// to plain TLS and then to TLS without SNI when the previous transport
    /// cannot complete the connection.
    ///
    /// A host that rejected ECH is remembered for `TLS_FALLBACK_TTL` after a
    /// successful fallback. Later requests then skip the transports that are
    /// known to fail while keeping that client's connection pool.
    ///
    /// `build` must be able to construct the request again; the fallback paths
    /// reuse it. Plain HTTP always uses the ordinary client so ECH cannot
    /// change its DNS or proxy behaviour.
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
            state.ech.as_ref().map(|runtime| {
                (
                    runtime.ech_client.clone(),
                    runtime.plain_client.clone(),
                    runtime.no_sni_client.clone(),
                )
            })
        };

        if url.scheme() == "https"
            && let Some((ech_client, plain_client, no_sni_client)) = ech_clients
        {
            let fallback = match fallback_key.as_deref() {
                Some(key) => self.tls_fallback_mode(key)?,
                None => None,
            };

            if fallback == Some(TlsFallback::NoSni) {
                if let Some(client) = no_sni_client.as_ref() {
                    return match build(client).send().await {
                        Ok(response) => Ok(response),
                        Err(error) => {
                            if let Some(key) = fallback_key.as_deref() {
                                self.clear_tls_fallback(key);
                            }
                            Err(network_error(url, error))
                        }
                    };
                }

                // A refreshed runtime can fail to build the fallback client;
                // drop the stale decision and try the full chain again.
                if let Some(key) = fallback_key.as_deref() {
                    self.clear_tls_fallback(key);
                }
            } else if fallback == Some(TlsFallback::Plain) {
                return self
                    .send_plain_then_no_sni(
                        url,
                        fallback_key.as_deref(),
                        fallback,
                        &plain_client,
                        no_sni_client.as_ref(),
                        &build,
                    )
                    .await;
            }

            match build(&ech_client).send().await {
                Ok(response) => return Ok(response),
                Err(error) if is_fallback_error(&error) => {
                    logging::warn(
                        "ech",
                        format!("ECH connection to {url} failed; retrying with plain TLS: {error}"),
                    );
                    return self
                        .send_plain_then_no_sni(
                            url,
                            fallback_key.as_deref(),
                            fallback,
                            &plain_client,
                            no_sni_client.as_ref(),
                            &build,
                        )
                        .await;
                }
                Err(error) => return Err(network_error(url, error)),
            }
        }

        build(&normal_client)
            .send()
            .await
            .map_err(|error| network_error(url, error))
    }

    /// Try plain TLS, then no-SNI. Only transport-level failures advance to
    /// the next transport; an HTTP response or a non-transport error is final.
    async fn send_plain_then_no_sni<F>(
        &self,
        url: &Url,
        key: Option<&str>,
        fallback: Option<TlsFallback>,
        plain_client: &reqwest::Client,
        no_sni_client: Option<&reqwest::Client>,
        build: &F,
    ) -> Result<reqwest::Response>
    where
        F: Fn(&reqwest::Client) -> reqwest::RequestBuilder,
    {
        let plain_error = match build(plain_client).send().await {
            Ok(response) => {
                if fallback != Some(TlsFallback::Plain) {
                    self.remember_tls_fallback(key, TlsFallback::Plain);
                }
                return Ok(response);
            }
            Err(error) => error,
        };

        if !is_fallback_error(&plain_error) {
            self.forget_tls_fallback(key);
            return Err(network_error(url, plain_error));
        }

        let Some(no_sni_client) = no_sni_client else {
            self.forget_tls_fallback(key);
            logging::warn(
                "ech",
                format!(
                    "plain TLS to {url} failed and no no-SNI client is available: {plain_error}"
                ),
            );
            return Err(network_error(url, plain_error));
        };

        logging::warn(
            "ech",
            format!("plain TLS to {url} failed; retrying without SNI: {plain_error}"),
        );

        match build(no_sni_client).send().await {
            Ok(response) => {
                self.remember_tls_fallback(key, TlsFallback::NoSni);
                Ok(response)
            }
            Err(error) => {
                self.forget_tls_fallback(key);
                logging::warn("ech", format!("no-SNI fallback for {url} failed: {error}"));
                Err(network_error(url, plain_error))
            }
        }
    }

    fn remember_tls_fallback(&self, key: Option<&str>, mode: TlsFallback) {
        let Some(key) = key else {
            return;
        };

        match self.cache_tls_fallback(key, mode) {
            Ok(()) => logging::info(
                "ech",
                match mode {
                    TlsFallback::Plain => {
                        format!("ECH unavailable for {key}; using plain TLS")
                    }
                    TlsFallback::NoSni => {
                        format!("ECH and plain TLS unavailable for {key}; using no SNI")
                    }
                },
            ),
            Err(error) => logging::warn(
                "ech",
                format!("failed to cache the TLS fallback for {key}: {error}"),
            ),
        }
    }

    fn forget_tls_fallback(&self, key: Option<&str>) {
        if let Some(key) = key {
            self.clear_tls_fallback(key);
        }
    }

    pub(super) fn lock(&self) -> Result<MutexGuard<'_, ClientState>> {
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

pub(super) fn build_client(proxy: Option<&str>) -> Result<reqwest::Client> {
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

pub(super) fn build_ech_client(
    session: &Arc<EchSession>,
    resolver: Arc<CloudflareResolver>,
    ech: bool,
) -> Result<reqwest::Client> {
    let tls = if ech {
        session.reqwest_ech_tls()?
    } else {
        session.reqwest_plain_tls()?
    };

    build_tls_client(tls, resolver, if ech { "ECH" } else { "plain TLS" })
}

/// Build the last-resort client. It still resolves through Cloudflare DoH and
/// still verifies the certificate against the request host; only the SNI
/// extension is omitted.
pub(super) fn build_no_sni_client(
    session: &Arc<EchSession>,
    resolver: Arc<CloudflareResolver>,
) -> Result<reqwest::Client> {
    build_tls_client(session.reqwest_no_sni_tls()?, resolver, "no-SNI")
}

fn build_tls_client(
    tls: rustls::ClientConfig,
    resolver: Arc<CloudflareResolver>,
    label: &str,
) -> Result<reqwest::Client> {
    base_builder()
        .use_preconfigured_tls(tls)
        .dns_resolver(resolver)
        .no_proxy()
        .build()
        .map_err(|error| {
            AppError::Unexpected(format!("failed to build the {label} HTTP client: {error}"))
        })
}

/// Only transport-level failures are retried. HTTP status errors, body errors
/// and decode errors have already reached the server and must not be repeated.
fn is_fallback_error(error: &reqwest::Error) -> bool {
    error.is_connect() || error.is_timeout()
}

/// Cache key for an HTTPS origin. The port is included so a service on a
/// non-default port cannot inherit another service's fallback decision.
pub(super) fn ech_fallback_key(url: &Url) -> Option<String> {
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

pub(super) fn base_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
    headers.insert(ACCEPT, HeaderValue::from_static(ACCEPT_VALUE));
    headers
}
