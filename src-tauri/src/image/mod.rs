//! Image fetching, platform search/detail and on-disk caching.
//!
//! Platform-specific request headers and page parsers live in [`platform`];
//! this module owns the platform-agnostic pipelines: image download/cache and
//! search/detail request orchestration.

mod cache;
mod http;
mod model;
mod platform;

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::error::{AppError, Result};

use cache::ImageFormat;
pub(crate) use http::HttpClient;
use http::HttpProxyConfig;
use model::{ComicDetail, SearchResponse};
use platform::{find as find_platform, resolve as resolve_platform, unknown_platform};

/// Bump only when the cache layout itself changes. Per-platform header changes
/// use `Platform::cache_version` instead.
const CACHE_VERSION: &str = "v2";
const CACHE_SUBDIR: &str = "images";

/// Result of fetching an image, with the metadata the frontend needs to load
/// the cached file through the asset protocol.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CachedImage {
    /// Absolute path inside the app cache directory.
    pub path: String,
    pub content_type: String,
    pub byte_length: u64,
    /// `true` when no network request was made because the file was cached.
    pub from_cache: bool,
    /// Platform profile that produced the cached file.
    pub platform: String,
}

/// Download `url` if needed and return the cached file.
///
/// `platform` is optional: known hosts are detected from the URL, and unknown
/// hosts fall back to `generic`. Callers that already know the platform should
/// pass it explicitly, which also selects its cache namespace.
#[tauri::command]
pub(crate) async fn fetch_image(
    app: AppHandle,
    client: State<'_, HttpClient>,
    url: String,
    platform: Option<String>,
) -> Result<CachedImage> {
    let url = cache::normalize_url(&url)?;
    let platform = resolve_platform(platform.as_deref(), &url)?;
    let cache_dir = app
        .path()
        .app_cache_dir()?
        .join(CACHE_SUBDIR)
        .join(platform.id());
    tokio::fs::create_dir_all(&cache_dir).await?;

    let key = cache::cache_key(
        platform.id(),
        platform.cache_version(),
        &platform.cache_key_url(&url),
    );
    if let Some(path) = cache::find_cached(&cache_dir, &key).await? {
        return cached_image(path, platform.id(), true).await;
    }

    let download = http::download_image(&client.get()?, platform, &url).await?;
    let path = cache_dir.join(format!("{key}.{}", download.format.extension()));
    cache::write_atomically(&cache_dir, &path, &key, &download.bytes).await?;

    cached_image(path, platform.id(), false).await
}

async fn cached_image(path: PathBuf, platform: &str, from_cache: bool) -> Result<CachedImage> {
    let metadata = tokio::fs::metadata(&path).await?;
    let content_type = path
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(ImageFormat::from_extension)
        .map_or("application/octet-stream", ImageFormat::content_type);

    Ok(CachedImage {
        path: cache::path_to_string(&path)?,
        content_type: content_type.to_string(),
        byte_length: metadata.len(),
        from_cache,
        platform: platform.to_string(),
    })
}

/// Search a platform by keyword.
#[tauri::command]
pub(crate) async fn search_comics(
    client: State<'_, HttpClient>,
    platform: String,
    query: String,
    page: u32,
) -> Result<SearchResponse> {
    let platform = find_platform(&platform).ok_or_else(|| unknown_platform(&platform))?;

    let query = query.trim();
    if query.is_empty() {
        return Err(AppError::InvalidInput {
            field: "query",
            reason: "must not be empty".into(),
        });
    }
    if page == 0 {
        return Err(AppError::InvalidInput {
            field: "page",
            reason: "must be 1 or greater".into(),
        });
    }

    let spec = platform.search_request(query, page)?;
    let body = http::fetch_document(&client.get()?, platform, &spec).await?;
    platform.parse_search(query, page, &body)
}

/// Fetch exact comic metadata by platform id.
#[tauri::command]
pub(crate) async fn get_comic(
    client: State<'_, HttpClient>,
    platform: String,
    id: String,
) -> Result<ComicDetail> {
    let platform = find_platform(&platform).ok_or_else(|| unknown_platform(&platform))?;

    let id = id.trim();
    if id.is_empty() {
        return Err(AppError::InvalidInput {
            field: "id",
            reason: "must not be empty".into(),
        });
    }

    let spec = platform.detail_request(id)?;
    let body = http::fetch_document(&client.get()?, platform, &spec).await?;
    platform.parse_detail(id, &body)
}

/// Apply or clear the user's HTTP proxy for every subsequent request.
#[tauri::command]
pub(crate) fn set_http_proxy(
    client: State<'_, HttpClient>,
    proxy: Option<HttpProxyConfig>,
) -> Result<()> {
    client.set_proxy(proxy)
}
