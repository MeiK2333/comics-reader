//! Shared HTTP client and the platform-agnostic download pipeline.

use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
use serde::Deserialize;
use url::Url;

use crate::error::{AppError, Result};

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
}

#[derive(Default)]
struct ClientState {
    /// Normalized `http://host:port/` URL, or `None` for reqwest's default.
    proxy: Option<String>,
    client: Option<reqwest::Client>,
}

impl HttpClient {
    pub(super) fn get(&self) -> Result<reqwest::Client> {
        let mut state = self.lock()?;

        if let Some(client) = state.client.as_ref() {
            return Ok(client.clone());
        }

        let client = build_client(state.proxy.as_deref())?;
        state.client = Some(client.clone());
        Ok(client)
    }

    /// Apply the user's proxy. Building eagerly means an address reqwest
    /// cannot use is rejected before it can replace a working proxy.
    pub(crate) fn set_proxy(&self, proxy: Option<HttpProxyConfig>) -> Result<()> {
        let proxy = proxy.map(|proxy| proxy.normalized_url()).transpose()?;
        let client = build_client(proxy.as_deref())?;

        let mut state = self.lock()?;
        state.proxy = proxy;
        state.client = Some(client);
        Ok(())
    }

    fn lock(&self) -> Result<MutexGuard<'_, ClientState>> {
        self.state
            .lock()
            .map_err(|_| AppError::Unexpected("HTTP client lock was poisoned".into()))
    }
}

fn build_client(proxy: Option<&str>) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .default_headers(base_headers())
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .redirect(reqwest::redirect::Policy::limited(10));

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
    client: &reqwest::Client,
    platform: &dyn Platform,
    url: &Url,
) -> Result<Download> {
    let mut response = platform
        .configure(client.get(url.as_str()), url)
        .send()
        .await
        .map_err(|error| AppError::Network {
            url: url.to_string(),
            reason: error.to_string(),
        })?;

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
pub(super) async fn fetch_document(
    client: &reqwest::Client,
    platform: &dyn Platform,
    spec: &RequestSpec,
) -> Result<Vec<u8>> {
    let url = &spec.url;
    let mut response = platform
        .configure(client.get(url.as_str()), url)
        .header(ACCEPT, spec.accept)
        .send()
        .await
        .map_err(|error| AppError::Network {
            url: url.to_string(),
            reason: error.to_string(),
        })?;

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
}
