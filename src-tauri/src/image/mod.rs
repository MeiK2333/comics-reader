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
use crate::logging;

use cache::ImageFormat;
pub(crate) use http::HttpClient;
use http::HttpProxyConfig;
pub(crate) use model::{ChapterRef, ReaderChapter, ReaderPage};
use model::{ComicDetail, SearchResponse};
use platform::{
    ImageDecode, Platform, RequestSpec, descramble, find as find_platform,
    resolve as resolve_platform, scramble_id, segment_count, unknown_platform,
};

/// Bump only when the cache layout itself changes. Per-platform header changes
/// use `Platform::cache_version` instead.
const CACHE_VERSION: &str = "v2";
const CACHE_SUBDIR: &str = "images";
/// Bump when a platform's decoded bytes change without a URL change (for
/// example, a new JM descramble algorithm). The suffix is part of the cache key.
const IMAGE_DECODE_VERSION: u32 = 1;

fn decode_key_suffix() -> String {
    format!("?decoded=v{IMAGE_DECODE_VERSION}")
}

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
    let requested_platform = platform.clone();
    let result = fetch_image_inner(&app, client.inner(), url.clone(), platform).await;

    match &result {
        // Cache hits are intentionally not logged: a single results page would
        // otherwise bury the network failures this log is meant to surface.
        Ok(image) if !image.from_cache => logging::info(
            "image",
            format!(
                "downloaded image: platform={} url={} bytes={}",
                image.platform, url, image.byte_length
            ),
        ),
        Ok(_) => {}
        Err(error) => logging::error(
            "image",
            format!(
                "failed to fetch image: platform={} url={} code={} error={error}",
                requested_platform.as_deref().unwrap_or("auto"),
                url,
                error.code().as_str()
            ),
        ),
    }

    result
}

pub(crate) async fn fetch_image_inner(
    app: &AppHandle,
    client: &HttpClient,
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

    // Decoded bytes differ from the download, so they get a distinct key. The
    // suffix is constant, which also lets a cached page load without asking JM
    // for the scramble id again.
    let decode = platform.image_decode(&url);
    let mut key_url = platform.cache_key_url(&url);
    if decode.is_some() {
        key_url.push_str(&decode_key_suffix());
    }

    let key = cache::cache_key(platform.id(), platform.cache_version(), &key_url);
    if let Some(path) = cache::find_cached(&cache_dir, &key).await? {
        return cached_image(path, platform.id(), true).await;
    }

    let mut last_error = None;
    let mut download = None;

    for candidate in std::iter::once(url.clone()).chain(platform.image_fallbacks(&url)) {
        match http::download_image(client, platform, &candidate).await {
            Ok(result) => {
                download = Some(result);
                break;
            }
            // A blocked CDN can answer with a challenge page instead of an
            // error, so `NotImage` is also worth another host.
            Err(error)
                if http::is_mirror_error(&error) || matches!(error, AppError::NotImage { .. }) =>
            {
                logging::warn(
                    "image",
                    format!("image host {candidate} failed ({error}); trying the next host"),
                );
                last_error = Some(error);
            }
            Err(error) => return Err(error),
        }
    }

    let Some(mut download) = download else {
        return Err(
            last_error.unwrap_or_else(|| AppError::Unexpected("image has no URL to try".into()))
        );
    };

    if let Some(ImageDecode::JmScramble { photo_id, filename }) = decode {
        let scramble_id = scramble_id(client, platform, &photo_id).await?;
        let segments = segment_count(scramble_id, &photo_id, &filename);
        if segments > 0 {
            download.bytes = descramble(&download.bytes, segments)?;
            download.format = ImageFormat::Jpeg;
        }
    }

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
    let result = search_comics_inner(client.inner(), &platform, &query, page).await;

    match &result {
        Ok(response) => logging::info(
            "search",
            format!(
                "search completed: platform={platform} page={page} total={} returned={} query={query:?}",
                response.total,
                response.result.len()
            ),
        ),
        Err(error) => logging::error(
            "search",
            format!(
                "search failed: platform={platform} page={page} code={} error={error} query={query:?}",
                error.code().as_str()
            ),
        ),
    }

    result
}

/// Prepare the platform before building a request.
///
/// JM signs `tokenparam` with the app version refreshed by `prepare`, so a
/// request built too early would use the built-in version for the first command
/// after startup. Keeping both steps in one helper makes the order explicit.
async fn prepared_spec(
    client: &HttpClient,
    platform: &dyn Platform,
    build: impl FnOnce() -> Result<RequestSpec>,
) -> Result<RequestSpec> {
    platform.prepare(client).await;
    build()
}

/// Record mirror health only after the platform parser accepts the body.
fn parse_document<T>(
    platform: &dyn Platform,
    fetched: &http::FetchedDocument,
    parse: impl FnOnce(&[u8]) -> Result<T>,
) -> Result<T> {
    let result = parse(&fetched.body);
    platform.note_request_outcome(&fetched.url, result.is_ok());
    result
}

async fn search_comics_inner(
    client: &HttpClient,
    platform: &str,
    query: &str,
    page: u32,
) -> Result<SearchResponse> {
    let platform = find_platform(platform).ok_or_else(|| unknown_platform(platform))?;

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

    let spec = prepared_spec(client, platform, || platform.search_request(query, page)).await?;
    let fetched = http::fetch_document(client, platform, &spec).await?;
    parse_document(platform, &fetched, |body| {
        platform.parse_search(query, page, body)
    })
}

/// Fetch exact comic metadata by platform id.
#[tauri::command]
pub(crate) async fn get_comic(
    client: State<'_, HttpClient>,
    platform: String,
    id: String,
) -> Result<ComicDetail> {
    let result = get_comic_inner(client.inner(), &platform, &id).await;

    match &result {
        Ok(detail) => logging::info(
            "detail",
            format!(
                "detail loaded: platform={platform} id={id:?} title={:?}",
                detail.title
            ),
        ),
        Err(error) => logging::error(
            "detail",
            format!(
                "detail failed: platform={platform} id={id:?} code={} error={error}",
                error.code().as_str()
            ),
        ),
    }

    result
}

async fn get_comic_inner(client: &HttpClient, platform: &str, id: &str) -> Result<ComicDetail> {
    let platform = find_platform(platform).ok_or_else(|| unknown_platform(platform))?;

    let id = id.trim();
    if id.is_empty() {
        return Err(AppError::InvalidInput {
            field: "id",
            reason: "must not be empty".into(),
        });
    }

    let spec = prepared_spec(client, platform, || platform.detail_request(id)).await?;
    let fetched = http::fetch_document(client, platform, &spec).await?;
    parse_document(platform, &fetched, |body| platform.parse_detail(id, body))
}

/// Fetch the page list for one chapter. Platforms with a single implicit
/// chapter ignore `chapter_id`; chaptered platforms use it to select the page.
#[tauri::command]
pub(crate) async fn get_reader_chapter(
    client: State<'_, HttpClient>,
    platform: String,
    id: String,
    chapter_id: Option<String>,
) -> Result<ReaderChapter> {
    let result =
        get_reader_chapter_inner(client.inner(), &platform, &id, chapter_id.as_deref()).await;

    match &result {
        Ok(chapter) => logging::info(
            "reader",
            format!(
                "reader chapter loaded: platform={platform} id={id:?} chapter={:?} pages={}",
                chapter_id.as_deref().unwrap_or("-"),
                chapter.pages.len()
            ),
        ),
        Err(error) => logging::error(
            "reader",
            format!(
                "reader chapter failed: platform={platform} id={id:?} chapter={:?} code={} error={error}",
                chapter_id.as_deref().unwrap_or("-"),
                error.code().as_str()
            ),
        ),
    }

    result
}

pub(crate) async fn get_reader_chapter_inner(
    client: &HttpClient,
    platform: &str,
    id: &str,
    chapter_id: Option<&str>,
) -> Result<ReaderChapter> {
    let platform = find_platform(platform).ok_or_else(|| unknown_platform(platform))?;

    let id = id.trim();
    if id.is_empty() {
        return Err(AppError::InvalidInput {
            field: "id",
            reason: "must not be empty".into(),
        });
    }
    let chapter_id = chapter_id.map(str::trim).filter(|value| !value.is_empty());

    let spec = prepared_spec(client, platform, || {
        platform.reader_chapter_request(id, chapter_id)
    })
    .await?;
    let fetched = http::fetch_document(client, platform, &spec).await?;
    parse_document(platform, &fetched, |body| {
        platform.parse_reader_chapter(id, chapter_id, body)
    })
}

/// Apply or clear the user's HTTP proxy for every subsequent request.
#[tauri::command]
pub(crate) fn set_http_proxy(
    client: State<'_, HttpClient>,
    proxy: Option<HttpProxyConfig>,
) -> Result<()> {
    let enabled = proxy.is_some();
    let result = client.set_proxy(proxy);

    match &result {
        Ok(()) if enabled => logging::info("http", "HTTP proxy enabled"),
        Ok(()) => logging::info("http", "HTTP proxy cleared"),
        Err(error) => logging::error(
            "http",
            format!(
                "failed to update the HTTP proxy: code={} error={error}",
                error.code().as_str()
            ),
        ),
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    use reqwest::RequestBuilder;
    use url::Url;

    struct FakePlatform {
        prepared: AtomicBool,
    }

    impl Platform for FakePlatform {
        fn id(&self) -> &'static str {
            "fake"
        }

        fn configure(&self, request: RequestBuilder, _url: &Url) -> RequestBuilder {
            request
        }

        fn prepare<'a>(
            &'a self,
            _client: &'a HttpClient,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
            self.prepared.store(true, Ordering::SeqCst);
            Box::pin(std::future::ready(()))
        }

        fn search_request(&self, _query: &str, _page: u32) -> Result<RequestSpec> {
            assert!(
                self.prepared.load(Ordering::SeqCst),
                "prepare must run before the request is built"
            );
            Ok(RequestSpec::json(
                Url::parse("https://example.com/").unwrap(),
            ))
        }
    }

    #[tokio::test]
    async fn prepares_before_building_a_request() {
        let client = HttpClient::default();
        let platform = FakePlatform {
            prepared: AtomicBool::new(false),
        };

        let spec = prepared_spec(&client, &platform, || platform.search_request("example", 1))
            .await
            .unwrap();

        assert_eq!(spec.url.as_str(), "https://example.com/");
    }

    struct HealthPlatform {
        healthy: AtomicBool,
    }

    impl Platform for HealthPlatform {
        fn id(&self) -> &'static str {
            "health"
        }

        fn configure(&self, request: RequestBuilder, _url: &Url) -> RequestBuilder {
            request
        }

        fn note_request_outcome(&self, _url: &Url, success: bool) {
            self.healthy.store(success, Ordering::SeqCst);
        }
    }

    #[test]
    fn mirror_health_waits_for_a_successful_parse() {
        let platform = HealthPlatform {
            healthy: AtomicBool::new(false),
        };
        let fetched = http::FetchedDocument {
            url: Url::parse("https://example.com/").unwrap(),
            body: b"ok".to_vec(),
        };

        parse_document(&platform, &fetched, |body| {
            assert_eq!(body, b"ok");
            Ok(())
        })
        .unwrap();
        assert!(platform.healthy.load(Ordering::SeqCst));

        let error = parse_document::<()>(&platform, &fetched, |_| {
            Err(AppError::Unexpected("bad body".into()))
        })
        .unwrap_err();
        assert_eq!(error.code().as_str(), "unexpected");
        assert!(!platform.healthy.load(Ordering::SeqCst));
    }

    #[test]
    fn decoded_cache_keys_carry_the_decode_version() {
        assert_eq!(decode_key_suffix(), "?decoded=v1");
    }
}
