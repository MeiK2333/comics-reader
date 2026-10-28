//! Image and document fetching, including mirror fallback.

use reqwest::Method;
use reqwest::header::ACCEPT;
use url::Url;

use crate::error::{AppError, Result};
use crate::logging;

use super::super::cache::ImageFormat;
use super::{HttpClient, MAX_DOCUMENT_BYTES};
use crate::platforms::{Catalog, ImageProfile, RequestSpec};

const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) struct Download {
    pub(crate) bytes: Vec<u8>,
    pub(crate) format: ImageFormat,
}

pub(crate) async fn download_image(
    http: &HttpClient,
    platform: &dyn ImageProfile,
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
    let mut format = ImageFormat::detect(&bytes);
    if format.is_none()
        && let Some(decoded) = platform.decode_image(url, &bytes)
        && let Some(decoded_format) = ImageFormat::detect(&decoded)
    {
        // Some CDNs store images encrypted; the platform decrypts them here so
        // the format check below sees the real payload.
        bytes = decoded;
        format = Some(decoded_format);
    }
    let format = format
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

/// A document that reached us from a mirror, before platform parsing. The
/// caller records mirror health after parsing: an HTTP 200 with an unparseable
/// body is not a healthy mirror.
pub(crate) struct FetchedDocument {
    pub(crate) url: Url,
    pub(crate) body: Vec<u8>,
}

/// Fetch an HTML or JSON document for a platform search/detail request.
///
/// Mirrors are tried only for errors that another host can plausibly fix: a
/// transport failure, an empty body, or a 403/429/5xx. Parse errors are final
/// because the body already arrived from a host that answered.
pub(crate) async fn fetch_document<P>(
    http: &HttpClient,
    platform: &P,
    spec: &RequestSpec,
) -> Result<FetchedDocument>
where
    P: Catalog + ?Sized,
{
    let mut last_error = None;

    for url in spec.urls() {
        match fetch_document_once(http, platform, spec, url).await {
            Ok(body) => {
                return Ok(FetchedDocument {
                    url: url.clone(),
                    body,
                });
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

async fn fetch_document_once<P>(
    http: &HttpClient,
    platform: &P,
    spec: &RequestSpec,
    url: &Url,
) -> Result<Vec<u8>>
where
    P: Catalog + ?Sized,
{
    let mut response = http
        .send(url, |client| {
            let request = if spec.method == Method::POST {
                client.post(url.as_str())
            } else {
                client.get(url.as_str())
            };
            let request = platform
                .configure(request, url)
                .headers(spec.headers.clone())
                .header(ACCEPT, spec.accept);
            match &spec.body {
                Some(body) => request.body(body.clone()),
                None => request,
            }
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
pub(crate) fn is_mirror_error(error: &AppError) -> bool {
    match error {
        AppError::Network { .. } | AppError::EmptyResponse { .. } => true,
        AppError::HttpStatus { status, .. } => *status == 403 || *status == 429 || *status >= 500,
        _ => false,
    }
}
