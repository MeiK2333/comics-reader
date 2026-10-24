//! Shared HTTP client and the platform-agnostic download pipeline.

use std::sync::Mutex;
use std::time::Duration;

use reqwest::header::{ACCEPT, HeaderMap, HeaderValue, USER_AGENT};
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

/// Shared HTTP client. Building it lazily keeps startup infallible while still
/// reusing pooled connections across the many images of a chapter.
#[derive(Default)]
pub(crate) struct HttpClient {
    client: Mutex<Option<reqwest::Client>>,
}

impl HttpClient {
    pub(super) fn get(&self) -> Result<reqwest::Client> {
        let mut slot = self
            .client
            .lock()
            .map_err(|_| AppError::Unexpected("HTTP client lock was poisoned".into()))?;

        if let Some(client) = slot.as_ref() {
            return Ok(client.clone());
        }

        let client = reqwest::Client::builder()
            .default_headers(base_headers())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .map_err(|error| {
                AppError::Unexpected(format!("failed to build the HTTP client: {error}"))
            })?;

        *slot = Some(client.clone());
        Ok(client)
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

    #[test]
    fn base_headers_look_like_a_browser() {
        let headers = base_headers();
        assert_eq!(headers[USER_AGENT].to_str().unwrap(), USER_AGENT_VALUE);
        assert_eq!(headers[ACCEPT].to_str().unwrap(), ACCEPT_VALUE);
    }
}
