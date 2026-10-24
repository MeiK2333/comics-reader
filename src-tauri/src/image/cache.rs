//! Cache keys, lookup and atomic writes for downloaded images.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};
use url::Url;

use crate::error::{AppError, Result};

use super::CACHE_VERSION;

/// Cache key scoped by platform and profile version. The same URL fetched with
/// a different `Referer` can return different bytes, so it must not share a
/// cache entry with another platform.
pub(super) fn cache_key(platform_id: &str, platform_version: u32, url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(CACHE_VERSION.as_bytes());
    hasher.update(b"\0");
    hasher.update(platform_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(platform_version.to_le_bytes());
    hasher.update(b"\0");
    hasher.update(url.as_bytes());
    hex::encode(hasher.finalize())
}

/// Probe the extension for each supported format instead of scanning the cache
/// directory, which keeps lookups O(1) as the cache grows.
pub(super) async fn find_cached(cache_dir: &Path, key: &str) -> Result<Option<PathBuf>> {
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
pub(super) async fn write_atomically(
    cache_dir: &Path,
    path: &Path,
    key: &str,
    bytes: &[u8],
) -> Result<()> {
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

pub(super) fn path_to_string(path: &Path) -> Result<String> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        AppError::Unexpected(format!("cache path is not valid UTF-8: {}", path.display()))
    })
}

/// Reject anything the downloader should not attempt and drop the fragment,
/// which is never sent to the server and therefore must not affect the key.
pub(super) fn normalize_url(raw: &str) -> Result<Url> {
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

/// Image formats the webview can display. The extension drives the MIME type
/// the asset protocol serves, so it has to match the actual payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageFormat {
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

    pub(crate) const fn extension(self) -> &'static str {
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

    pub(crate) const fn content_type(self) -> &'static str {
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

    pub(crate) fn from_extension(extension: &str) -> Option<Self> {
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

    pub(crate) fn from_content_type(content_type: &str) -> Option<Self> {
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

    pub(crate) fn detect(bytes: &[u8]) -> Option<Self> {
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
    fn cache_key_is_scoped_by_platform_and_version() {
        let url = "https://example.com/a.jpg";
        let key = cache_key("manhuagui", 1, url);

        assert_eq!(key, cache_key("manhuagui", 1, url));
        assert_ne!(key, cache_key("generic", 1, url));
        assert_ne!(key, cache_key("manhuagui", 2, url));
        assert_ne!(key, cache_key("manhuagui", 1, "https://example.com/b.jpg"));
        assert_eq!(key.len(), 64);
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
