//! Remote image fetching with an on-disk cache.
//!
//! The webview never talks to image hosts directly. [`fetch_image`] downloads
//! the bytes here (some hosts require a browser-like `Referer`), validates that
//! the payload really is a displayable image, and stores it under the app cache
//! directory. The frontend then loads the cached file through the asset
//! protocol, so visiting the same page again skips the network entirely.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Manager, State};
use url::Url;

use crate::error::{AppError, Result};

/// Bump to invalidate every cache entry after changing how images are stored.
const CACHE_VERSION: &str = "v1";
const CACHE_SUBDIR: &str = "images";
/// Page images are usually a few megabytes; anything past this is treated as a
/// bad response rather than filling the cache with an accidental download.
const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

// Some image hosts return 403 unless the request looks like a browser opening
// the page. Keeping these on the Rust side means the webview never needs
// blanket network permissions for arbitrary hosts.
const REFERER: &str = "https://www.manhuagui.com/";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36";

/// Shared HTTP client. Building it lazily keeps startup infallible while still
/// reusing pooled connections across the many images of a chapter.
#[derive(Default)]
pub struct HttpClient {
    client: Mutex<Option<reqwest::Client>>,
}

impl HttpClient {
    fn get(&self) -> Result<reqwest::Client> {
        let mut slot = self
            .client
            .lock()
            .map_err(|_| AppError::Unexpected("HTTP client lock was poisoned".into()))?;

        if let Some(client) = slot.as_ref() {
            return Ok(client.clone());
        }

        let client = reqwest::Client::builder()
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

/// Result of fetching an image, with the metadata the frontend needs to load
/// the cached file through the asset protocol.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CachedImage {
    /// Absolute path inside the app cache directory.
    pub path: String,
    pub content_type: String,
    pub byte_length: u64,
    /// `true` when no network request was made because the file was cached.
    pub from_cache: bool,
}

/// Download `url` if needed and return the cached file.
#[tauri::command]
pub async fn fetch_image(
    app: AppHandle,
    client: State<'_, HttpClient>,
    url: String,
) -> Result<CachedImage> {
    let url = normalize_url(&url)?;
    let cache_dir = app.path().app_cache_dir()?.join(CACHE_SUBDIR);
    tokio::fs::create_dir_all(&cache_dir).await?;

    let key = cache_key(url.as_str());
    if let Some(path) = find_cached(&cache_dir, &key).await? {
        return cached_image(path, true).await;
    }

    let download = download_image(&client.get()?, &url).await?;
    let path = cache_dir.join(format!("{key}.{}", download.format.extension()));
    write_atomically(&cache_dir, &path, &key, &download.bytes).await?;

    cached_image(path, false).await
}

async fn cached_image(path: PathBuf, from_cache: bool) -> Result<CachedImage> {
    let metadata = tokio::fs::metadata(&path).await?;
    let content_type = path
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(ImageFormat::from_extension)
        .map_or("application/octet-stream", ImageFormat::content_type);

    Ok(CachedImage {
        path: path_to_string(&path)?,
        content_type: content_type.to_string(),
        byte_length: metadata.len(),
        from_cache,
    })
}

struct Download {
    bytes: Vec<u8>,
    format: ImageFormat,
}

async fn download_image(client: &reqwest::Client, url: &Url) -> Result<Download> {
    let mut response = client
        .get(url.as_str())
        .header(reqwest::header::REFERER, REFERER)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
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

/// Probe the extension for each supported format instead of scanning the cache
/// directory, which keeps lookups O(1) as the cache grows.
async fn find_cached(cache_dir: &Path, key: &str) -> Result<Option<PathBuf>> {
    for format in ImageFormat::ALL {
        let candidate = cache_dir.join(format!("{key}.{}", format.extension()));
        match tokio::fs::metadata(&candidate).await {
            Ok(metadata) if metadata.is_file() => return Ok(Some(candidate)),
            Ok(_) => continue,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        }
    }

    Ok(None)
}

/// Write through a temporary file so a crash can never leave a truncated image
/// behind a valid-looking cache name.
async fn write_atomically(cache_dir: &Path, path: &Path, key: &str, bytes: &[u8]) -> Result<()> {
    static TEMP_ID: AtomicU64 = AtomicU64::new(0);

    let temporary = cache_dir.join(format!(
        "{key}.{}.{}.part",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));

    if let Err(error) = tokio::fs::write(&temporary, bytes).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(error.into());
    }

    if let Err(rename_error) = tokio::fs::rename(&temporary, path).await {
        // A concurrent request for the same URL may have cached the file while
        // this one was in flight; the winner is just as valid, so keep it.
        let cached = tokio::fs::try_exists(path).await.unwrap_or(false);
        let _ = tokio::fs::remove_file(&temporary).await;
        if !cached {
            return Err(rename_error.into());
        }
    }

    Ok(())
}

fn path_to_string(path: &Path) -> Result<String> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        AppError::Unexpected(format!("cache path is not valid UTF-8: {}", path.display()))
    })
}

/// Reject anything the downloader should not attempt and drop the fragment,
/// which is never sent to the server and therefore must not affect the key.
fn normalize_url(raw: &str) -> Result<Url> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AppError::InvalidInput {
            field: "url",
            reason: "must not be empty".into(),
        });
    }

    let mut url = Url::parse(trimmed).map_err(|error| AppError::InvalidInput {
        field: "url",
        reason: format!("not a valid URL: {error}"),
    })?;

    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::InvalidInput {
            field: "url",
            reason: "scheme must be http or https".into(),
        });
    }
    if url.host_str().is_none() {
        return Err(AppError::InvalidInput {
            field: "url",
            reason: "missing host".into(),
        });
    }

    url.set_fragment(None);
    Ok(url)
}

fn cache_key(url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CACHE_VERSION.as_bytes());
    hasher.update(b"\0");
    hasher.update(url.as_bytes());
    hex::encode(hasher.finalize())
}

/// Image formats the webview can display. The extension drives the MIME type
/// the asset protocol serves, so it has to match the actual payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImageFormat {
    Jpeg,
    Png,
    Gif,
    WebP,
    Avif,
    Bmp,
    Ico,
    Svg,
}

impl ImageFormat {
    const ALL: [Self; 8] = [
        Self::Jpeg,
        Self::Png,
        Self::Gif,
        Self::WebP,
        Self::Avif,
        Self::Bmp,
        Self::Ico,
        Self::Svg,
    ];

    const fn extension(self) -> &'static str {
        match self {
            Self::Jpeg => "jpg",
            Self::Png => "png",
            Self::Gif => "gif",
            Self::WebP => "webp",
            Self::Avif => "avif",
            Self::Bmp => "bmp",
            Self::Ico => "ico",
            Self::Svg => "svg",
        }
    }

    const fn content_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Gif => "image/gif",
            Self::WebP => "image/webp",
            Self::Avif => "image/avif",
            Self::Bmp => "image/bmp",
            Self::Ico => "image/x-icon",
            Self::Svg => "image/svg+xml",
        }
    }

    fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "png" => Some(Self::Png),
            "gif" => Some(Self::Gif),
            "webp" => Some(Self::WebP),
            "avif" => Some(Self::Avif),
            "bmp" => Some(Self::Bmp),
            "ico" => Some(Self::Ico),
            "svg" => Some(Self::Svg),
            _ => None,
        }
    }

    fn from_content_type(content_type: &str) -> Option<Self> {
        match content_type.to_ascii_lowercase().as_str() {
            "image/jpeg" | "image/jpg" | "image/pjpeg" => Some(Self::Jpeg),
            "image/png" => Some(Self::Png),
            "image/gif" => Some(Self::Gif),
            "image/webp" => Some(Self::WebP),
            "image/avif" => Some(Self::Avif),
            "image/bmp" | "image/x-bmp" => Some(Self::Bmp),
            "image/x-icon" | "image/vnd.microsoft.icon" => Some(Self::Ico),
            "image/svg+xml" => Some(Self::Svg),
            _ => None,
        }
    }

    fn detect(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Some(Self::Png);
        }
        if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            return Some(Self::Jpeg);
        }
        if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            return Some(Self::Gif);
        }
        if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
            return Some(Self::WebP);
        }
        if bytes.starts_with(b"BM") {
            return Some(Self::Bmp);
        }
        if bytes.starts_with(&[0, 0, 1, 0]) {
            return Some(Self::Ico);
        }
        if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
            let brand = &bytes[8..12];
            if brand == b"avif" || brand == b"avis" {
                return Some(Self::Avif);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_is_stable_and_url_specific() {
        let first = cache_key("https://example.com/a.jpg");
        assert_eq!(first, cache_key("https://example.com/a.jpg"));
        assert_ne!(first, cache_key("https://example.com/b.jpg"));
        assert_eq!(first.len(), 64);
    }

    #[test]
    fn normalize_url_trims_and_drops_the_fragment() {
        let url = normalize_url(" https://example.com/a.png#page ").unwrap();
        assert_eq!(url.as_str(), "https://example.com/a.png");
    }

    #[test]
    fn normalize_url_rejects_unsupported_input() {
        assert!(normalize_url("").is_err());
        assert!(normalize_url("   ").is_err());
        assert!(normalize_url("not a url").is_err());
        assert!(normalize_url("ftp://example.com/a.png").is_err());
    }

    #[test]
    fn magic_bytes_identify_supported_formats() {
        assert_eq!(
            ImageFormat::detect(b"\x89PNG\r\n\x1a\nrest"),
            Some(ImageFormat::Png)
        );
        assert_eq!(
            ImageFormat::detect(b"\xff\xd8\xff\xe0rest"),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(ImageFormat::detect(b"GIF89a rest"), Some(ImageFormat::Gif));
        assert_eq!(
            ImageFormat::detect(b"RIFF0000WEBPVP8 "),
            Some(ImageFormat::WebP)
        );
        assert_eq!(
            ImageFormat::detect(b"\0\0\0\x20ftypavifrest"),
            Some(ImageFormat::Avif)
        );
    }

    #[test]
    fn detect_ignores_unknown_payloads() {
        assert_eq!(ImageFormat::detect(b"<html>not an image</html>"), None);
    }

    #[test]
    fn content_type_mapping_covers_cache_round_trips() {
        for format in ImageFormat::ALL {
            assert_eq!(
                ImageFormat::from_content_type(format.content_type()),
                Some(format)
            );
            assert_eq!(
                ImageFormat::from_extension(format.extension()),
                Some(format)
            );
        }
    }
}
