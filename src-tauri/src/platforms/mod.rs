//! Per-platform request profiles and page parsers.
//!
//! Adding an image-only platform means implementing [`ImageProfile`] and
//! registering it in `registry`. Search/detail/reading additionally require
//! [`Catalog`]. The shared fetching pipeline in `image` stays unchanged; the
//! platform only builds request URLs and parses the returned bytes. A platform
//! whose detail needs extra requests overrides [`Catalog::comic_detail`].

mod copymanga;
mod ehentai;
mod generic;
mod jm;
mod jm_api;
mod jm_image;
mod manhuagui;
mod manwa;
pub(crate) mod model;
mod nhentai;
mod pixiv;
mod registry;

pub(crate) use registry::{catalog, find_catalog, is_known_platform, resolve};
#[cfg(test)]
pub(crate) use registry::{detect, find_profile};

use std::future::Future;
use std::pin::Pin;

use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use reqwest::{Method, RequestBuilder};
use url::Url;

use crate::error::{AppError, Result};
use crate::image::HttpClient;
use crate::image::http;

use model::{ComicDetail, ReaderChapter, SearchResponse};

/// Future returned by [`ImageProfile::preprocess`].
pub(crate) type PreprocessFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<Vec<u8>>>> + Send + 'a>>;

const HTML_ACCEPT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";

/// A request URL plus the `Accept` value its parser expects.
pub(crate) struct RequestSpec {
    pub(crate) url: Url,
    pub(crate) accept: &'static str,
    /// Headers that only this request needs, such as JM's APP token. They are
    /// applied after [`ImageProfile::configure`], which keeps shared platform
    /// headers in one place and per-request signing data here.
    pub(crate) headers: HeaderMap,
    /// Mirrors of the same logical request. The fetch layer tries them in
    /// order when the current one fails with a retryable error.
    pub(crate) fallbacks: Vec<Url>,
    pub(crate) method: Method,
    /// JSON bodies are tiny; keeping the bytes lets every fallback attempt
    /// rebuild the same request without re-serializing.
    pub(crate) body: Option<Vec<u8>>,
}

impl RequestSpec {
    pub(crate) fn html(url: Url) -> Self {
        Self {
            url,
            accept: HTML_ACCEPT,
            headers: HeaderMap::new(),
            fallbacks: Vec::new(),
            method: Method::GET,
            body: None,
        }
    }

    pub(crate) fn json(url: Url) -> Self {
        Self {
            url,
            accept: "application/json",
            headers: HeaderMap::new(),
            fallbacks: Vec::new(),
            method: Method::GET,
            body: None,
        }
    }

    /// A JSON API request that must use POST, such as E-Hentai's `gdata`.
    pub(crate) fn json_post(url: Url, body: Vec<u8>) -> Self {
        let mut spec = Self::json(url);
        spec.method = Method::POST;
        spec.body = Some(body);
        spec.headers
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        spec
    }

    pub(crate) fn with_headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    pub(crate) fn with_fallbacks(mut self, fallbacks: Vec<Url>) -> Self {
        self.fallbacks = fallbacks;
        self
    }

    /// The primary URL followed by every mirror.
    pub(crate) fn urls(&self) -> impl Iterator<Item = &Url> {
        std::iter::once(&self.url).chain(self.fallbacks.iter())
    }
}

/// Everything the image pipeline needs to know about one platform.
pub(crate) trait ImageProfile: Send + Sync {
    /// Stable id used by command arguments, cache namespaces and the UI.
    fn id(&self) -> &'static str;

    /// Bump when this platform's request profile changes; only its cache
    /// entries are invalidated.
    fn cache_version(&self) -> u32 {
        1
    }

    /// Best-effort detection from an image URL. `Generic` never matches.
    fn matches(&self, _url: &Url) -> bool {
        false
    }

    /// Apply platform-specific request headers to the shared client request.
    fn configure(&self, request: RequestBuilder, _url: &Url) -> RequestBuilder;

    /// URL used as the cache identity. Platforms whose image URLs carry
    /// disposable query parameters can drop them without changing the request.
    fn cache_key_url(&self, url: &Url) -> String {
        url.to_string()
    }

    /// Alternative hosts for the same image. The image pipeline tries them in
    /// order when the current host fails with a retryable error.
    fn image_fallbacks(&self, _url: &Url) -> Vec<Url> {
        Vec::new()
    }

    /// Cache-key suffix for platforms whose cached bytes differ from the
    /// downloaded bytes. Returning `None` keeps the plain URL key.
    fn decode_key(&self, _url: &Url) -> Option<String> {
        None
    }

    /// Transform downloaded bytes before they are cached. `None` keeps the
    /// downloaded bytes; `Some(bytes)` replaces them.
    fn preprocess<'a>(
        &'a self,
        _client: &'a HttpClient,
        _url: &'a Url,
        _bytes: &'a [u8],
    ) -> PreprocessFuture<'a> {
        Box::pin(async move { Ok(None) })
    }

    /// Last-chance transform for image hosts that encrypt bytes at rest.
    ///
    /// Called only when [`download_image`](crate::image::http) cannot detect
    /// an image format from the downloaded bytes. Returning bytes replaces the
    /// payload and the pipeline re-runs format detection; `None` keeps the
    /// `notImage` error. Platforms that decode here must also return a suffix
    /// from [`Self::decode_key`] so decoded cache entries never collide with
    /// raw downloads.
    fn decode_image(&self, _url: &Url, _bytes: &[u8]) -> Option<Vec<u8>> {
        None
    }

    /// Resolve a stable page URL into the URL that actually serves image bytes.
    ///
    /// E-Hentai exposes `/s/...` image pages whose real image link carries a
    /// short-lived `keystamp`; resolving it immediately before the download
    /// keeps the cache key stable while the image URL stays fresh. Platforms
    /// that serve images directly keep the identity implementation.
    fn resolve_image_url<'a>(
        &'a self,
        _client: &'a HttpClient,
        url: &'a Url,
    ) -> Pin<Box<dyn Future<Output = Result<Url>> + Send + 'a>> {
        Box::pin(async move { Ok(url.clone()) })
    }
}

/// Search/detail/reading capabilities layered on top of an image profile.
/// Platforms without a catalog stay image-only and do not need to implement
/// empty parser methods.
pub(crate) trait Catalog: ImageProfile {
    /// Whether the work has one implicit chapter (nhentai, E-Hentai, pixiv).
    /// Downloads and the reader ignore chapter ids for such a platform.
    fn single_chapter(&self) -> bool {
        false
    }

    /// Normalize a work id before it becomes a persisted key. Most platforms
    /// only accept digits; JM also accepts its `JM` prefix.
    fn normalize_work_id(&self, id: &str) -> Result<String> {
        numeric_id(id, self.id())
    }

    /// Normalize a chapter id before it becomes a download key or file name.
    /// Most platforms use numeric ids; copymanga uses UUIDs.
    fn normalize_chapter_id(&self, id: &str) -> Result<String> {
        numeric_id(id, "chapterId")
    }

    /// Called after the platform parses a document from one mirror, or after
    /// the fetch fails. A platform with interchangeable origins can remember
    /// which one works; single-origin platforms keep the default no-op. HTTP
    /// success alone is not enough: an unparseable body is not a healthy
    /// mirror.
    fn note_request_outcome(&self, _url: &Url, _success: bool) {}

    /// Optional async work before a command serves its request. JM uses it to
    /// refresh the app version it signs with; other platforms have nothing to
    /// prepare and resolve immediately.
    fn prepare<'a>(
        &'a self,
        _client: &'a HttpClient,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(std::future::ready(()))
    }

    /// Build the keyword search request for a 1-based page.
    fn search_request(&self, _query: &str, _page: u32) -> Result<RequestSpec> {
        Err(self.unsupported("search"))
    }

    /// Parse a search response body.
    fn parse_search(&self, _query: &str, _page: u32, _body: &[u8]) -> Result<SearchResponse> {
        Err(self.unsupported("search"))
    }

    /// Build the exact-id comic detail request.
    fn detail_request(&self, _id: &str) -> Result<RequestSpec> {
        Err(self.unsupported("comic detail"))
    }

    /// Parse a comic detail response body.
    fn parse_detail(&self, _id: &str, _body: &[u8]) -> Result<ComicDetail> {
        Err(self.unsupported("comic detail"))
    }

    /// Fetch and parse a comic's complete detail.
    ///
    /// The default implementation is the one-request path most platforms use.
    /// Platforms whose chapter list lives behind another endpoint (copymanga's
    /// per-group chapter requests) override this instead of splitting the
    /// payload across `detail_request` and `parse_detail`.
    fn comic_detail<'a>(
        &'a self,
        client: &'a HttpClient,
        id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<ComicDetail>> + Send + 'a>> {
        Box::pin(async move {
            let spec = self.detail_request(id)?;
            let fetched = http::fetch_document(client, self, &spec).await?;
            let result = self.parse_detail(id, &fetched.body);
            self.note_request_outcome(&fetched.url, result.is_ok());
            result
        })
    }

    /// Build the request that returns a chapter's page list. `chapter_id` is
    /// `None` for galleries that have a single implicit chapter.
    fn reader_chapter_request(&self, _id: &str, _chapter_id: Option<&str>) -> Result<RequestSpec> {
        Err(self.unsupported("reading"))
    }

    /// Parse a chapter page list from the response body.
    fn parse_reader_chapter(
        &self,
        _id: &str,
        _chapter_id: Option<&str>,
        _body: &[u8],
    ) -> Result<ReaderChapter> {
        Err(self.unsupported("reading"))
    }

    /// Fetch and parse a chapter's page list.
    ///
    /// The default implementation is the one-request path. Platforms whose
    /// page list spans several documents (E-Hentai's paginated gallery pages)
    /// override this instead of forcing every page into one response.
    fn reader_chapter<'a>(
        &'a self,
        client: &'a HttpClient,
        id: &'a str,
        chapter_id: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<ReaderChapter>> + Send + 'a>> {
        Box::pin(async move {
            let spec = self.reader_chapter_request(id, chapter_id)?;
            let fetched = http::fetch_document(client, self, &spec).await?;
            let result = self.parse_reader_chapter(id, chapter_id, &fetched.body);
            self.note_request_outcome(&fetched.url, result.is_ok());
            result
        })
    }

    /// Error used by platforms that only implement image fetching.
    fn unsupported(&self, operation: &str) -> AppError {
        AppError::InvalidInput {
            field: "platform",
            reason: format!("platform `{}` does not support {operation}", self.id()),
        }
    }
}

/// Match an exact host or one of its subdomains without treating
/// `notmanhuagui.com` as `manhuagui.com`.
pub(super) fn host_matches(url: &Url, suffixes: &[&str]) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();

    suffixes.iter().any(|suffix| {
        host == *suffix
            || host
                .strip_suffix(suffix)
                .is_some_and(|prefix| prefix.ends_with('.'))
    })
}

/// Turn protocol-relative or root-relative asset URLs into absolute ones.
pub(super) fn absolute_url(base: &str, raw: &str) -> String {
    let raw = raw.trim();
    if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.to_string()
    } else if let Some(rest) = raw.strip_prefix("//") {
        format!("https://{rest}")
    } else if raw.starts_with('/') {
        format!("{base}{raw}")
    } else {
        format!("{base}/{raw}")
    }
}

/// Search and detail ids are injected into URL paths, so only accept digits.
pub(super) fn numeric_id(id: &str, platform: &str) -> Result<String> {
    let trimmed = id.trim();
    if trimmed.is_empty() || !trimmed.chars().all(|character| character.is_ascii_digit()) {
        return Err(AppError::InvalidInput {
            field: "id",
            reason: format!("`{platform}` ids must be numeric"),
        });
    }
    Ok(trimmed.to_string())
}

/// JM also accepts ids like `JM123456`; normalize them to the numeric form.
pub(super) fn jm_id(id: &str) -> Result<String> {
    let trimmed = id.trim();
    let digits = trimmed
        .strip_prefix("JM")
        .or_else(|| trimmed.strip_prefix("jm"))
        .unwrap_or(trimmed);
    numeric_id(digits, "jm")
}

pub(super) fn collapse_whitespace(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::Client;
    use reqwest::header::{ACCEPT_LANGUAGE, HeaderMap, REFERER, USER_AGENT};

    fn parse(raw: &str) -> Url {
        Url::parse(raw).unwrap()
    }

    fn headers_for(id: &str, raw: &str) -> HeaderMap {
        let url = parse(raw);
        let platform = find_profile(id).expect("platform should exist");
        platform
            .configure(Client::new().get(url.clone()), &url)
            .build()
            .unwrap()
            .headers()
            .clone()
    }

    #[test]
    fn detects_known_hosts() {
        assert_eq!(
            detect(&parse("https://cf.mhgui.com/cpic/h/1.jpg")).id(),
            "manhuagui"
        );
        assert_eq!(
            detect(&parse("https://i.hamreus.com/ps1/1.jpg")).id(),
            "manhuagui"
        );
        assert_eq!(
            detect(&parse("https://cdn-msp.jmapiproxy.com/media/1.jpg")).id(),
            "jm"
        );
        assert_eq!(
            detect(&parse("https://i1.nhentai.net/galleries/1/1.jpg")).id(),
            "nhentai"
        );
        assert_eq!(
            detect(&parse("https://sl.mangafunb.fun/l/1/1.jpg.c1500x.jpg")).id(),
            "copymanga"
        );
        assert_eq!(
            detect(&parse("https://e-hentai.org/s/abc/1-1")).id(),
            "ehentai"
        );
        assert_eq!(
            detect(&parse("https://x.hath.network:1167/h/a/b.webp")).id(),
            "ehentai"
        );
        assert_eq!(
            detect(&parse("https://ehgt.org/w/1/thumb.webp")).id(),
            "ehentai"
        );
        assert_eq!(
            detect(&parse("https://i.pximg.net/img-original/img/1_p0.png")).id(),
            "pixiv"
        );
        assert_eq!(
            detect(&parse("https://i.pixiv.re/img-original/img/1_p0.png")).id(),
            "pixiv"
        );
        assert_eq!(
            detect(&parse(
                "https://mwappimgs.cc/static/upload3/book/id/1/2_zb.webp?v=1"
            ))
            .id(),
            "manwa"
        );
        assert_eq!(
            detect(&parse(
                "https://newmwtuyuan25.cc/static/upload3/book/id/1/2_zb.webp"
            ))
            .id(),
            "manwa"
        );
        assert_eq!(detect(&parse("https://example.com/a.png")).id(), "generic");
    }

    #[test]
    fn explicit_platform_wins_and_unknown_is_rejected() {
        let url = parse("https://example.com/a.png");
        assert_eq!(resolve(Some("manhuagui"), &url).unwrap().id(), "manhuagui");
        assert_eq!(resolve(Some("generic"), &url).unwrap().id(), "generic");
        assert!(resolve(Some("unknown"), &url).is_err());
    }

    #[test]
    fn platform_headers_are_applied() {
        let manhuagui = headers_for("manhuagui", "https://cf.mhgui.com/a.jpg");
        assert_eq!(
            manhuagui[REFERER].to_str().unwrap(),
            "https://www.manhuagui.com/"
        );
        assert_eq!(
            manhuagui[ACCEPT_LANGUAGE].to_str().unwrap(),
            "zh-CN,zh;q=0.9,en;q=0.8"
        );

        let nhentai = headers_for("nhentai", "https://i.nhentai.net/a.jpg");
        assert_eq!(nhentai[REFERER].to_str().unwrap(), "https://nhentai.net/");

        let jm = headers_for("jm", "https://cdn-msp.jmapiproxy1.cc/a.jpg");
        assert_eq!(jm[REFERER].to_str().unwrap(), "https://www.cdnhjk.net");
        assert!(
            jm[USER_AGENT].to_str().unwrap().contains("Android"),
            "jm must send the APP user agent"
        );

        let copymanga = headers_for("copymanga", "https://sl.mangafunb.fun/l/1.jpg");
        assert_eq!(
            copymanga[REFERER].to_str().unwrap(),
            "https://www.mangacopy.com/"
        );

        let ehentai = headers_for("ehentai", "https://e-hentai.org/s/abc/1-1");
        assert_eq!(ehentai[REFERER].to_str().unwrap(), "https://e-hentai.org/");
        assert_eq!(ehentai[ACCEPT_LANGUAGE].to_str().unwrap(), "en-US,en;q=0.9");

        let pixiv = headers_for("pixiv", "https://i.pximg.net/img-original/img/1_p0.png");
        assert_eq!(pixiv[REFERER].to_str().unwrap(), "https://www.pixiv.net/");
        assert_eq!(
            pixiv[ACCEPT_LANGUAGE].to_str().unwrap(),
            "zh-CN,zh;q=0.9,en;q=0.8"
        );

        let manwa = headers_for("manwa", "https://mwappimgs.cc/static/upload3/1.webp");
        assert_eq!(manwa[REFERER].to_str().unwrap(), "https://manwa.me/");
        assert_eq!(
            manwa[ACCEPT_LANGUAGE].to_str().unwrap(),
            "zh-CN,zh;q=0.9,en;q=0.8"
        );
    }

    #[test]
    fn generic_has_no_referer() {
        let headers = headers_for("generic", "https://example.com/a.png");
        assert!(!headers.contains_key(REFERER));
    }

    #[test]
    fn generic_rejects_search_and_detail() {
        assert!(find_catalog("generic").is_none());
        assert!(catalog("generic", "search").is_err());
    }

    #[test]
    fn generic_is_an_image_profile_without_a_catalog() {
        assert!(find_profile("generic").is_some());
        assert!(find_catalog("generic").is_none());
        let error = match catalog("generic", "search") {
            Ok(_) => panic!("generic must not be a catalog"),
            Err(error) => error,
        };
        assert_eq!(error.code().as_str(), "invalidInput");
    }

    #[test]
    fn numeric_ids_reject_path_traversal() {
        assert!(numeric_id("123", "test").is_ok());
        assert!(numeric_id("../123", "test").is_err());
        assert!(numeric_id("12/3", "test").is_err());
        assert!(jm_id("JM123").is_ok());
        assert_eq!(jm_id("JM123").unwrap(), "123");
    }

    #[test]
    fn json_post_keeps_method_body_and_content_type() {
        let spec = RequestSpec::json_post(parse("https://api.example.test/"), b"{}".to_vec());

        assert_eq!(spec.method, reqwest::Method::POST);
        assert_eq!(spec.body.as_deref(), Some(&b"{}"[..]));
        assert_eq!(
            spec.headers[reqwest::header::CONTENT_TYPE],
            "application/json"
        );
    }

    #[tokio::test]
    async fn image_resolution_defaults_to_identity() {
        let url = parse("https://example.com/a.jpg");
        let resolved = find_profile("generic")
            .unwrap()
            .resolve_image_url(&HttpClient::default(), &url)
            .await
            .unwrap();

        assert_eq!(resolved, url);
    }

    #[test]
    fn manhuagui_and_jm_drop_query_for_cache() {
        let url = parse("https://example.com/a.jpg?v=1");
        assert_eq!(
            find_profile("manhuagui").unwrap().cache_key_url(&url),
            "https://example.com/a.jpg"
        );
        assert_eq!(
            find_profile("jm").unwrap().cache_key_url(&url),
            "https://example.com/a.jpg"
        );
        assert_eq!(
            find_profile("nhentai").unwrap().cache_key_url(&url),
            url.as_str()
        );

        let alternate = parse("https://sl.mangafuna.xyz/l/1.jpg?v=1");
        assert_eq!(
            find_profile("copymanga").unwrap().cache_key_url(&alternate),
            "https://sl.mangafunb.fun/l/1.jpg"
        );

        let image_page = parse("https://e-hentai.org/s/abc/1-1?nl=1");
        assert_eq!(
            find_profile("ehentai").unwrap().cache_key_url(&image_page),
            "https://e-hentai.org/s/abc/1-1"
        );

        let pixiv_proxy = parse("https://i.pixiv.re/img-original/img/1_p0.png");
        assert_eq!(
            find_profile("pixiv").unwrap().cache_key_url(&pixiv_proxy),
            "https://i.pximg.net/img-original/img/1_p0.png"
        );
    }
}
