//! Cache keys, lookup and atomic writes for downloaded images.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use url::Url;

use crate::error::{AppError, Result};
use crate::logging;

use super::CACHE_VERSION;

/// Keep the disposable image cache bounded without touching downloaded works.
const SWEEP_MAX_BYTES: u64 = 512 * 1024 * 1024;
const SWEEP_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
/// Files touched this recently may belong to an in-flight image request.
const SWEEP_PROTECT: Duration = Duration::from_secs(60 * 60);

/// `ImageCache` owns the cache root so the app resolves `app_cache_dir()` once
/// at startup instead of on every image request.
#[derive(Clone)]
pub(crate) struct ImageCache {
    base: PathBuf,
}

impl ImageCache {
    pub(crate) fn new(app_cache_dir: PathBuf) -> Self {
        Self {
            base: app_cache_dir.join(super::CACHE_SUBDIR),
        }
    }

    fn platform_dir(&self, platform_id: &str) -> PathBuf {
        self.base.join(platform_id)
    }

    pub(super) async fn lookup(&self, platform_id: &str, key: &str) -> Result<Option<PathBuf>> {
        find_cached(&self.platform_dir(platform_id), key).await
    }

    pub(super) async fn write(
        &self,
        platform_id: &str,
        key: &str,
        format: ImageFormat,
        bytes: &[u8],
    ) -> Result<PathBuf> {
        let directory = self.platform_dir(platform_id);
        tokio::fs::create_dir_all(&directory).await?;
        let path = directory.join(format!("{key}.{}", format.extension()));
        write_atomically(&directory, &path, key, bytes).await?;
        Ok(path)
    }

    pub(crate) async fn sweep_default(&self) -> Result<()> {
        self.sweep(SWEEP_MAX_BYTES, SWEEP_MAX_AGE).await
    }

    /// Delete files older than `max_age`, then the oldest files until the
    /// remaining bytes fit `max_bytes`. Fresh files are never removed by the
    /// size pass because they may be part of an in-flight request.
    pub(super) async fn sweep(&self, max_bytes: u64, max_age: Duration) -> Result<()> {
        let mut files = Vec::new();
        collect_cache_files(&self.base, &mut files).await?;

        let now = SystemTime::now();
        let mut kept = Vec::new();
        for (path, size, modified) in files {
            if now.duration_since(modified).unwrap_or_default() > max_age {
                remove_cache_file(&path).await;
            } else {
                kept.push((path, size, modified));
            }
        }

        let mut total = kept.iter().map(|(_, size, _)| *size).sum::<u64>();
        if total <= max_bytes {
            return Ok(());
        }

        kept.sort_by_key(|(_, _, modified)| *modified);
        for (path, size, modified) in kept {
            if total <= max_bytes {
                break;
            }
            if now.duration_since(modified).unwrap_or_default() < SWEEP_PROTECT {
                continue;
            }
            if remove_cache_file(&path).await {
                total = total.saturating_sub(size);
            }
        }

        Ok(())
    }
}

async fn collect_cache_files(
    directory: &Path,
    files: &mut Vec<(PathBuf, u64, SystemTime)>,
) -> Result<()> {
    let mut entries = match tokio::fs::read_dir(directory).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };

    while let Some(entry) = entries.next_entry().await? {
        let metadata = match entry.metadata().await {
            Ok(metadata) => metadata,
            Err(error) => {
                logging::warn(
                    "cache",
                    format!(
                        "failed to inspect cache entry {}: {error}",
                        entry.path().display()
                    ),
                );
                continue;
            }
        };
        if metadata.is_dir() {
            Box::pin(collect_cache_files(&entry.path(), files)).await?;
        } else if metadata.is_file() {
            files.push((
                entry.path(),
                metadata.len(),
                metadata.modified().unwrap_or_else(|_| SystemTime::now()),
            ));
        }
    }

    Ok(())
}

async fn remove_cache_file(path: &Path) -> bool {
    match tokio::fs::remove_file(path).await {
        Ok(()) => true,
        Err(error) => {
            logging::warn(
                "cache",
                format!("failed to remove cache file {}: {error}", path.display()),
            );
            false
        }
    }
}

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
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                return Ok(Some(candidate));
            }
            // A crash between creating the temporary file and syncing it can
            // leave a zero-byte entry under the final name. Treat it as a miss
            // and remove it so the next request downloads a usable image.
            Ok(metadata) if metadata.is_file() => {
                let _ = tokio::fs::remove_file(&candidate).await;
                continue;
            }
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

    // Flush and sync before the rename. Without it a power loss can leave the
    // final name pointing at a truncated file.
    let synced = async {
        let file = tokio::fs::File::create(&temporary).await?;
        let mut writer = tokio::io::BufWriter::new(file);
        writer.write_all(bytes).await?;
        writer.flush().await?;
        writer.get_ref().sync_all().await?;
        Ok::<(), std::io::Error>(())
    }
    .await;
    if let Err(error) = synced {
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

    fn temp_cache(tag: &str) -> (PathBuf, ImageCache) {
        let root = std::env::temp_dir().join(format!(
            "comics-reader-image-cache-{}-{}-{tag}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cache = ImageCache::new(root.clone());
        (root, cache)
    }

    fn set_age(path: &Path, age: Duration) {
        let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        file.set_modified(SystemTime::now() - age).unwrap();
    }

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

    #[tokio::test]
    async fn image_cache_lookup_and_write_round_trip() {
        let (root, cache) = temp_cache("round-trip");

        let path = cache
            .write("generic", "key", ImageFormat::Png, b"\x89PNG\r\n\x1a\n")
            .await
            .unwrap();
        assert_eq!(path, root.join("images").join("generic").join("key.png"));
        assert_eq!(
            cache.lookup("generic", "key").await.unwrap(),
            Some(path.clone())
        );
        assert_eq!(cache.lookup("generic", "other").await.unwrap(), None);

        let _ = tokio::fs::remove_dir_all(&root).await;
    }

    #[tokio::test]
    async fn sweep_removes_expired_files() {
        let (root, cache) = temp_cache("sweep-age");
        let old = cache
            .write("generic", "old", ImageFormat::Png, b"old")
            .await
            .unwrap();
        let fresh = cache
            .write("generic", "fresh", ImageFormat::Png, b"new")
            .await
            .unwrap();
        set_age(&old, Duration::from_secs(2 * 60 * 60));

        cache
            .sweep(1024 * 1024, Duration::from_secs(60 * 60))
            .await
            .unwrap();

        assert!(!old.exists());
        assert!(fresh.exists());
        let _ = tokio::fs::remove_dir_all(&root).await;
    }

    #[tokio::test]
    async fn sweep_enforces_size_from_the_oldest_files() {
        let (root, cache) = temp_cache("sweep-size");
        let oldest = cache
            .write("generic", "oldest", ImageFormat::Png, &[0u8; 10])
            .await
            .unwrap();
        let middle = cache
            .write("generic", "middle", ImageFormat::Png, &[1u8; 10])
            .await
            .unwrap();
        let newest = cache
            .write("generic", "newest", ImageFormat::Png, &[2u8; 10])
            .await
            .unwrap();
        set_age(&oldest, Duration::from_secs(4 * 60 * 60));
        set_age(&middle, Duration::from_secs(3 * 60 * 60));
        set_age(&newest, Duration::from_secs(2 * 60 * 60));

        cache
            .sweep(15, Duration::from_secs(30 * 24 * 60 * 60))
            .await
            .unwrap();

        assert!(!oldest.exists());
        assert!(!middle.exists());
        assert!(newest.exists());
        let _ = tokio::fs::remove_dir_all(&root).await;
    }

    #[tokio::test]
    async fn sweep_does_not_delete_recent_files_for_capacity() {
        let (root, cache) = temp_cache("sweep-protect");
        let fresh = cache
            .write("generic", "fresh", ImageFormat::Png, &[0u8; 20])
            .await
            .unwrap();

        cache
            .sweep(1, Duration::from_secs(30 * 24 * 60 * 60))
            .await
            .unwrap();

        assert!(fresh.exists());
        let _ = tokio::fs::remove_dir_all(&root).await;
    }

    #[tokio::test]
    async fn ignores_and_removes_empty_cache_entries() {
        use std::sync::atomic::{AtomicU64, Ordering};

        static ID: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "comics-reader-cache-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        tokio::fs::create_dir_all(&dir).await.unwrap();

        let key = "empty-entry";
        let path = dir.join(format!("{key}.jpg"));
        tokio::fs::write(&path, b"").await.unwrap();

        assert!(find_cached(&dir, key).await.unwrap().is_none());
        assert!(!path.exists(), "the empty file should be removed");

        tokio::fs::write(&path, b"\xff\xd8\xff").await.unwrap();
        assert_eq!(find_cached(&dir, key).await.unwrap(), Some(path));

        let _ = tokio::fs::remove_dir_all(&dir).await;
    }
}
