//! Per-platform request profiles and page parsers.
//!
//! Adding a platform means implementing [`Platform`], registering it in
//! [`PLATFORMS`] and adding its host suffixes. The shared fetching pipeline in
//! the parent module stays unchanged; the platform only builds request URLs and
//! parses the returned bytes.

mod generic;
mod jm;
mod manhuagui;
mod nhentai;

use reqwest::RequestBuilder;
use url::Url;

use crate::error::{AppError, Result};
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
}

impl RequestSpec {
    pub(crate) fn html(url: Url) -> Self {
        Self {
            url,
            accept: HTML_ACCEPT,
        }
    }

    pub(crate) fn json(url: Url) -> Self {
        Self {
            url,
            accept: "application/json",
        }
    }
}

/// Everything the request layer needs to know about one platform.
pub(crate) trait Platform: Send + Sync {
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
static JM: Jm = Jm;
static NHENTAI: Nhentai = Nhentai;

static PLATFORMS: [&dyn Platform; 4] = [&GENERIC, &MANHUAGUI, &JM, &NHENTAI];

/// Resolve an explicit platform id, or detect one from the image URL.
pub(crate) fn resolve(explicit: Option<&str>, url: &Url) -> Result<&'static dyn Platform> {
    match explicit {
        Some(id) => find(id).ok_or_else(|| unknown_platform(id)),
        None => Ok(detect(url)),
    }
}

/// Look up a platform by its stable command id.
pub(crate) fn find(id: &str) -> Option<&'static dyn Platform> {
    PLATFORMS
        .iter()
        .copied()
        .find(|platform| platform.id() == id)
}

pub(crate) fn unknown_platform(id: &str) -> AppError {
    AppError::InvalidInput {
        field: "platform",
        reason: format!("unknown platform `{id}`"),
    }
}

fn detect(url: &Url) -> &'static dyn Platform {
    PLATFORMS
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
    use reqwest::header::{ACCEPT_LANGUAGE, HeaderMap, REFERER};

    fn parse(raw: &str) -> Url {
        Url::parse(raw).unwrap()
    }

    fn headers_for(id: &str, raw: &str) -> HeaderMap {
        let url = parse(raw);
        let platform = find(id).expect("platform should exist");
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

        let jm = headers_for("jm", "https://cdn-msp.jmapiproxy.com/a.jpg");
        assert_eq!(jm[REFERER].to_str().unwrap(), "https://18comic.vip/");
    }

    #[test]
    fn generic_has_no_referer() {
        let headers = headers_for("generic", "https://example.com/a.png");
        assert!(!headers.contains_key(REFERER));
    }

    #[test]
    fn generic_rejects_search_and_detail() {
        let generic = find("generic").unwrap();
        assert!(generic.search_request("test", 1).is_err());
        assert!(generic.detail_request("1").is_err());
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
            find("manhuagui").unwrap().cache_key_url(&url),
            "https://example.com/a.jpg"
        );
        assert_eq!(
            find("jm").unwrap().cache_key_url(&url),
            "https://example.com/a.jpg"
        );
        assert_eq!(find("nhentai").unwrap().cache_key_url(&url), url.as_str());
    }
}
