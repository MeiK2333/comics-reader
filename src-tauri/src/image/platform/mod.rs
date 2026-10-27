//! Per-platform request profiles and page parsers.
//!
//! Adding an image-only platform means implementing [`ImageProfile`] and
//! registering it in [`PROFILES`]. Search/detail/reading additionally require
//! [`Catalog`], registered in [`CATALOGS`]. The shared fetching pipeline in the
//! parent module stays unchanged; the platform only builds request URLs and
//! parses the returned bytes.

mod generic;
mod jm;
mod jm_api;
mod jm_image;
mod manhuagui;
mod nhentai;

pub(crate) use jm::scramble_id;
pub(crate) use jm_image::{descramble, segment_count};

use std::future::Future;
use std::pin::Pin;

use reqwest::RequestBuilder;
use reqwest::header::HeaderMap;
use url::Url;

use crate::error::{AppError, Result};
use crate::image::HttpClient;
use crate::image::model::{ComicDetail, ReaderChapter, SearchResponse};

use generic::Generic;
use jm::Jm;
use manhuagui::Manhuagui;
use nhentai::Nhentai;

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
}

impl RequestSpec {
    pub(crate) fn html(url: Url) -> Self {
        Self {
            url,
            accept: HTML_ACCEPT,
            headers: HeaderMap::new(),
            fallbacks: Vec::new(),
        }
    }

    pub(crate) fn json(url: Url) -> Self {
        Self {
            url,
            accept: "application/json",
            headers: HeaderMap::new(),
            fallbacks: Vec::new(),
        }
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

/// Post-download work a platform needs before the bytes are cached.
#[derive(Debug, Clone)]
pub(crate) enum ImageDecode {
    /// JM permutes the horizontal bands of every page image. The band count
    /// depends on the photo id, the file name and the album's scramble id.
    JmScramble { photo_id: String, filename: String },
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

    /// Whether the downloaded bytes need decoding before they are cached.
    fn image_decode(&self, _url: &Url) -> Option<ImageDecode> {
        None
    }
}

/// Search/detail/reading capabilities layered on top of an image profile.
/// Platforms without a catalog stay image-only and do not need to implement
/// empty parser methods.
pub(crate) trait Catalog: ImageProfile {
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

    /// Error used by platforms that only implement image fetching.
    fn unsupported(&self, operation: &str) -> AppError {
        AppError::InvalidInput {
            field: "platform",
            reason: format!("platform `{}` does not support {operation}", self.id()),
        }
    }
}

static GENERIC: Generic = Generic;
static MANHUAGUI: Manhuagui = Manhuagui;
static JM: Jm = Jm::new();
static NHENTAI: Nhentai = Nhentai;

static PROFILES: [&dyn ImageProfile; 4] = [&GENERIC, &MANHUAGUI, &JM, &NHENTAI];
static CATALOGS: [&dyn Catalog; 3] = [&MANHUAGUI, &JM, &NHENTAI];

/// Resolve an explicit image profile id, or detect one from the image URL.
pub(crate) fn resolve(explicit: Option<&str>, url: &Url) -> Result<&'static dyn ImageProfile> {
    match explicit {
        Some(id) => find_profile(id).ok_or_else(|| unknown_platform(id)),
        None => Ok(detect(url)),
    }
}

/// Look up an image profile by its stable command id.
pub(crate) fn find_profile(id: &str) -> Option<&'static dyn ImageProfile> {
    PROFILES
        .iter()
        .copied()
        .find(|platform| platform.id() == id)
}

/// Look up a catalog by its stable command id.
pub(crate) fn find_catalog(id: &str) -> Option<&'static dyn Catalog> {
    CATALOGS
        .iter()
        .copied()
        .find(|platform| platform.id() == id)
}

/// Resolve a catalog, preserving the old "platform does not support ..."
/// error for image-only platforms while unknown ids stay unknown.
pub(crate) fn catalog(id: &str, operation: &str) -> Result<&'static dyn Catalog> {
    find_catalog(id).ok_or_else(|| {
        if find_profile(id).is_some() {
            AppError::InvalidInput {
                field: "platform",
                reason: format!("platform `{id}` does not support {operation}"),
            }
        } else {
            unknown_platform(id)
        }
    })
}

pub(crate) fn unknown_platform(id: &str) -> AppError {
    AppError::InvalidInput {
        field: "platform",
        reason: format!("unknown platform `{id}`"),
    }
}

fn detect(url: &Url) -> &'static dyn ImageProfile {
    PROFILES
        .iter()
        .copied()
        .find(|platform| platform.matches(url))
        .unwrap_or(&GENERIC)
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
    }
}
