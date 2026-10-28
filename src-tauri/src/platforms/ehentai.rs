//! E-Hentai galleries: cursor-paginated search, `gdata` metadata, and
//! image-page resolution.
//!
//! E-Hentai's search has no numeric page numbers; each response carries the
//! next cursor. The cursor is kept in memory so the numeric search UI can walk
//! pages one at a time. Gallery detail and page counts come from the official
//! `gdata` API; the reader lists stable `/s/...` image pages and resolves the
//! short-lived H@H URL only when the image is fetched.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::{LazyLock, Mutex};

use reqwest::RequestBuilder;
use reqwest::header::{ACCEPT_LANGUAGE, REFERER};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::json;
use url::Url;

use crate::error::{AppError, Result};
use crate::image::HttpClient;
use crate::image::http;
use crate::platforms::model::{
    ComicDetail, DetailChapter, ReaderChapter, ReaderPage, SearchItem, SearchResponse,
};

use super::{Catalog, ImageProfile, RequestSpec, absolute_url, collapse_whitespace, host_matches};

const BASE_URL: &str = "https://e-hentai.org";
const API_URL: &str = "https://api.e-hentai.org/api.php";
const REFERER_VALUE: &str = "https://e-hentai.org/";
const ACCEPT_LANGUAGE_VALUE: &str = "en-US,en;q=0.9";
const HOST_SUFFIXES: &[&str] = &["e-hentai.org", "ehgt.org", "hath.network"];
const SEARCH_PAGE_SIZE: u32 = 25;
const GALLERY_PAGE_SIZE: u32 = 20;
const SEARCH_CURSOR_LIMIT: usize = 64;

/// Search cursors are per-process and per-query. The search session itself is
/// memory-only in the webview, so losing the map on restart matches the UI.
static SEARCH_CURSORS: LazyLock<Mutex<HashMap<String, String>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(super) struct Ehentai;

impl Ehentai {
    pub(super) const ID: &'static str = "ehentai";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EhentaiTag {
    namespace: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EhentaiSearchExtra {
    category: String,
    languages: Vec<String>,
    pages: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EhentaiDetailExtra {
    gid: u64,
    token: String,
    title_jpn: Option<String>,
    category: String,
    uploader: Option<String>,
    posted: Option<u64>,
    pages: Option<u32>,
    file_size: Option<u64>,
    rating: Option<f64>,
    torrent_count: Option<u32>,
    expunged: bool,
    languages: Vec<String>,
    tags: Vec<EhentaiTag>,
}

impl ImageProfile for Ehentai {
    fn id(&self) -> &'static str {
        Self::ID
    }

    fn matches(&self, url: &Url) -> bool {
        host_matches(url, HOST_SUFFIXES)
    }

    fn configure(&self, request: RequestBuilder, _url: &Url) -> RequestBuilder {
        request
            .header(REFERER, REFERER_VALUE)
            .header(ACCEPT_LANGUAGE, ACCEPT_LANGUAGE_VALUE)
    }

    fn cache_key_url(&self, url: &Url) -> String {
        // The image page URL is what the reader stores. It is stable across
        // keystamp refreshes, so it is the right cache identity.
        if is_image_page(url) {
            format!("{BASE_URL}{}", url.path())
        } else {
            url.to_string()
        }
    }

    fn resolve_image_url<'a>(
        &'a self,
        client: &'a HttpClient,
        url: &'a Url,
    ) -> Pin<Box<dyn Future<Output = Result<Url>> + Send + 'a>> {
        Box::pin(async move {
            if !is_image_page(url) {
                return Ok(url.clone());
            }

            let spec = RequestSpec::html(url.clone());
            let fetched = http::fetch_document(client, self, &spec).await?;
            parse_image_page(&fetched.body)
        })
    }
}

impl Catalog for Ehentai {
    fn single_chapter(&self) -> bool {
        true
    }

    fn normalize_work_id(&self, id: &str) -> Result<String> {
        let (gid, token) = parse_work_id(id)?;
        Ok(work_id(&gid, &token))
    }

    fn search_request(&self, query: &str, page: u32) -> Result<RequestSpec> {
        let url = if page <= 1 {
            let mut url = Url::parse(&format!("{BASE_URL}/"))
                .map_err(|error| AppError::Unexpected(format!("invalid E-Hentai URL: {error}")))?;
            url.query_pairs_mut()
                .append_pair("f_search", query)
                .append_pair("inline_set", "dm_l");
            url
        } else {
            cursor_for(query, page)?
        };

        Ok(RequestSpec::html(url))
    }

    fn parse_search(&self, query: &str, page: u32, body: &[u8]) -> Result<SearchResponse> {
        let html = Html::parse_document(&String::from_utf8_lossy(body));
        let next = next_search_url(&html);
        store_cursor(query, page.saturating_add(1), next.as_deref());

        let rows = selector("table.itg tr");
        let result = html
            .select(&rows)
            .filter_map(parse_search_row)
            .collect::<Vec<_>>();

        let total = (page.saturating_sub(1) as u64) * SEARCH_PAGE_SIZE as u64 + result.len() as u64;

        Ok(SearchResponse {
            total,
            page_size: SEARCH_PAGE_SIZE,
            result,
            has_next: Some(next.is_some()),
        })
    }

    fn detail_request(&self, id: &str) -> Result<RequestSpec> {
        gdata_request(id)
    }

    fn parse_detail(&self, id: &str, body: &[u8]) -> Result<ComicDetail> {
        let (gid, token) = parse_work_id(id)?;
        let item = parse_gdata(body)?;
        build_detail(&item, &gid, &token)
    }

    fn reader_chapter<'a>(
        &'a self,
        client: &'a HttpClient,
        id: &'a str,
        _chapter_id: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<ReaderChapter>> + Send + 'a>> {
        Box::pin(async move {
            let (gid, token) = parse_work_id(id)?;
            let spec = gdata_request(id)?;
            let fetched = http::fetch_document(client, self, &spec).await?;
            let item = parse_gdata(&fetched.body)?;
            let title = display_title(&item, &gid);
            let thumb = item.thumb.trim().to_owned();
            let count = parse_u32(&item.filecount).unwrap_or(0);
            let pages = fetch_gallery_pages(client, &gid, &token, count).await?;

            if pages.is_empty() {
                return Err(AppError::NotFound {
                    resource: format!("E-Hentai gallery `{id}` pages"),
                });
            }

            Ok(ReaderChapter {
                title,
                thumb,
                chapter_id: None,
                chapter_title: None,
                pages,
                previous: None,
                next: None,
            })
        })
    }
}

/// Parse the canonical `gid-token`/`gid/token`/gallery URL form into its parts.
fn parse_work_id(raw: &str) -> Result<(String, String)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(invalid_id("must not be empty"));
    }

    if raw.starts_with("http://") || raw.starts_with("https://") {
        let url = Url::parse(raw).map_err(|_| invalid_id("must be a valid gallery URL"))?;
        let segments = url
            .path_segments()
            .map(|segments| segments.collect::<Vec<_>>())
            .unwrap_or_default();
        if segments.len() < 3 || segments[0] != "g" {
            return Err(invalid_id("URL must point to /g/{gid}/{token}/"));
        }
        return validate_work_id(segments[1], segments[2]);
    }

    let parts = raw
        .split_once('/')
        .or_else(|| raw.split_once('-'))
        .ok_or_else(|| invalid_id("expected gid-token, gid/token or a gallery URL"))?;
    validate_work_id(parts.0, parts.1)
}

fn validate_work_id(gid: &str, token: &str) -> Result<(String, String)> {
    let gid = gid.trim();
    let token = token.trim().trim_end_matches('/').to_ascii_lowercase();

    if gid.is_empty() || !gid.chars().all(|character| character.is_ascii_digit()) {
        return Err(invalid_id("gallery id must be numeric"));
    }
    if token.len() != 10 || !token.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(invalid_id(
            "gallery token must be 10 hexadecimal characters",
        ));
    }

    Ok((gid.to_owned(), token))
}

fn invalid_id(reason: impl Into<String>) -> AppError {
    AppError::InvalidInput {
        field: "id",
        reason: reason.into(),
    }
}

fn work_id(gid: &str, token: &str) -> String {
    format!("{gid}-{token}")
}

fn cursor_key(query: &str, page: u32) -> String {
    format!("{query}\u{0}{page}")
}

fn store_cursor(query: &str, page: u32, next: Option<&str>) {
    let Ok(mut cursors) = SEARCH_CURSORS.lock() else {
        return;
    };
    let key = cursor_key(query, page);
    match next {
        Some(next) => {
            if cursors.len() >= SEARCH_CURSOR_LIMIT {
                cursors.clear();
            }
            cursors.insert(key, next.to_owned());
        }
        None => {
            cursors.remove(&key);
        }
    }
}

fn cursor_for(query: &str, page: u32) -> Result<Url> {
    let next = SEARCH_CURSORS
        .lock()
        .map_err(|_| AppError::Unexpected("E-Hentai search cursor lock was poisoned".into()))?
        .get(&cursor_key(query, page))
        .cloned();

    let next = next.ok_or_else(|| AppError::InvalidInput {
        field: "page",
        reason: "E-Hentai search pages must be requested in order from page 1".into(),
    })?;

    Url::parse(&next)
        .map_err(|error| AppError::Unexpected(format!("invalid E-Hentai cursor: {error}")))
}

fn next_search_url(html: &Html) -> Option<String> {
    let selector = selector("a#dnext");
    html.select(&selector)
        .next()
        .and_then(|anchor| anchor.value().attr("href"))
        .map(str::trim)
        .filter(|href| !href.is_empty())
        .map(|href| absolute_url(BASE_URL, href))
}

fn parse_search_row(row: ElementRef<'_>) -> Option<SearchItem> {
    let link = row.select(&selector("td.gl3c a")).next()?;
    let href = link.value().attr("href")?;
    let (gid, token) = parse_gallery_href(href)?;

    let title = row
        .select(&selector("div.glink"))
        .next()
        .map(|element| collapse_whitespace(&element.text().collect::<String>()))
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| format!("#{gid}"));

    let thumb = row
        .select(&selector("td.gl2c img"))
        .next()
        .and_then(|image| {
            image
                .value()
                .attr("data-src")
                .or_else(|| image.value().attr("src"))
        })
        .map(str::trim)
        .filter(|src| !src.is_empty() && !src.starts_with("data:"))
        .map(|src| absolute_url(BASE_URL, src))
        .unwrap_or_default();

    let category = row
        .select(&selector("td.gl1c .cn"))
        .next()
        .map(|element| collapse_whitespace(&element.text().collect::<String>()))
        .unwrap_or_default();

    let tags = row
        .select(&selector(".gt"))
        .filter_map(|element| element.value().attr("title"))
        .filter_map(parse_tag)
        .collect::<Vec<_>>();

    let pages = row
        .select(&selector("td.gl4c div"))
        .filter_map(|element| parse_page_count(&element.text().collect::<String>()))
        .last();

    let extra = EhentaiSearchExtra {
        category,
        languages: languages(&tags),
        pages,
    };

    SearchItem::new(Ehentai::ID, work_id(&gid, &token), title, thumb, extra).ok()
}

fn parse_gallery_href(raw: &str) -> Option<(String, String)> {
    let url = Url::parse(&absolute_url(BASE_URL, raw)).ok()?;
    let segments = url.path_segments()?.collect::<Vec<_>>();
    if segments.len() < 3 || segments[0] != "g" {
        return None;
    }
    validate_work_id(segments[1], segments[2]).ok()
}

fn parse_tag(raw: &str) -> Option<EhentaiTag> {
    let (namespace, name) = raw.split_once(':')?;
    let namespace = namespace.trim().to_ascii_lowercase();
    let name = collapse_whitespace(name);
    if namespace.is_empty() || name.is_empty() {
        return None;
    }
    Some(EhentaiTag { namespace, name })
}

fn languages(tags: &[EhentaiTag]) -> Vec<String> {
    let mut languages = Vec::new();
    for tag in tags {
        if tag.namespace == "language" && !languages.contains(&tag.name) {
            languages.push(tag.name.clone());
        }
    }
    languages
}

fn parse_page_count(text: &str) -> Option<u32> {
    let lower = text.to_ascii_lowercase();
    let index = lower.find("page")?;
    let before = lower[..index].trim_end();
    let digits = before
        .chars()
        .rev()
        .take_while(|character| character.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
}

fn gdata_request(id: &str) -> Result<RequestSpec> {
    let (gid, token) = parse_work_id(id)?;
    let gid = gid
        .parse::<u64>()
        .map_err(|_| invalid_id("gallery id is out of range"))?;
    let body = serde_json::to_vec(&json!({
        "method": "gdata",
        "gidlist": [[gid, token]],
        "namespace": 1,
    }))?;
    let url = Url::parse(API_URL)
        .map_err(|error| AppError::Unexpected(format!("invalid E-Hentai API URL: {error}")))?;
    Ok(RequestSpec::json_post(url, body))
}

#[derive(Debug, Deserialize)]
struct GdataResponse {
    #[serde(default)]
    gmetadata: Vec<GdataItem>,
}

#[derive(Debug, Deserialize)]
struct GdataItem {
    gid: u64,
    token: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    title_jpn: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    thumb: String,
    #[serde(default)]
    uploader: String,
    #[serde(default)]
    posted: String,
    #[serde(default)]
    filecount: String,
    #[serde(default)]
    filesize: u64,
    #[serde(default)]
    rating: String,
    #[serde(default)]
    torrentcount: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    expunged: bool,
}

fn parse_gdata(body: &[u8]) -> Result<GdataItem> {
    let response: GdataResponse = serde_json::from_slice(body)?;
    response
        .gmetadata
        .into_iter()
        .next()
        .ok_or_else(|| AppError::NotFound {
            resource: "E-Hentai gallery".into(),
        })
}

fn build_detail(item: &GdataItem, gid: &str, token: &str) -> Result<ComicDetail> {
    let tags = item
        .tags
        .iter()
        .filter_map(|tag| parse_tag(tag))
        .collect::<Vec<_>>();
    let extra = EhentaiDetailExtra {
        gid: item.gid,
        token: non_empty(&item.token).unwrap_or_else(|| token.to_owned()),
        title_jpn: non_empty(&item.title_jpn),
        category: item.category.trim().to_owned(),
        uploader: non_empty(&item.uploader),
        posted: parse_u64(&item.posted).filter(|value| *value > 0),
        pages: parse_u32(&item.filecount).filter(|value| *value > 0),
        file_size: (item.filesize > 0).then_some(item.filesize),
        rating: item
            .rating
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| *value > 0.0),
        torrent_count: parse_u32(&item.torrentcount).filter(|value| *value > 0),
        expunged: item.expunged,
        languages: languages(&tags),
        tags,
    };

    ComicDetail::new(
        Ehentai::ID,
        display_title(item, gid),
        item.thumb.trim().to_owned(),
        vec![DetailChapter {
            id: None,
            title: None,
        }],
        extra,
    )
}

fn display_title(item: &GdataItem, gid: &str) -> String {
    let title = collapse_whitespace(&item.title);
    if title.is_empty() {
        format!("#{gid}")
    } else {
        title
    }
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn parse_u32(value: &str) -> Option<u32> {
    value.trim().parse().ok()
}

fn parse_u64(value: &str) -> Option<u64> {
    value.trim().parse().ok()
}

async fn fetch_gallery_pages(
    client: &HttpClient,
    gid: &str,
    token: &str,
    count: u32,
) -> Result<Vec<ReaderPage>> {
    if count == 0 {
        return Ok(Vec::new());
    }

    let page_count = count.div_ceil(GALLERY_PAGE_SIZE);
    let mut pages = Vec::with_capacity(count as usize);
    let mut seen = HashSet::new();

    for index in 0..page_count {
        let mut url = Url::parse(&format!("{BASE_URL}/g/{gid}/{token}/")).map_err(|error| {
            AppError::Unexpected(format!("invalid E-Hentai gallery URL: {error}"))
        })?;
        if index > 0 {
            url.query_pairs_mut().append_pair("p", &index.to_string());
        }
        let spec = RequestSpec::html(url);
        let fetched = http::fetch_document(client, &Ehentai, &spec).await?;
        for (number, page_url) in parse_gallery_page(&fetched.body, gid)? {
            if seen.insert(number) {
                pages.push(ReaderPage {
                    url: page_url.to_string(),
                    width: None,
                    height: None,
                    local_path: None,
                });
            }
        }
    }

    pages.sort_by_key(|page| {
        Url::parse(&page.url)
            .ok()
            .and_then(|url| page_number(&url))
            .unwrap_or(u32::MAX)
    });

    Ok(pages)
}

fn parse_gallery_page(body: &[u8], gid: &str) -> Result<Vec<(u32, Url)>> {
    let html = Html::parse_document(&String::from_utf8_lossy(body));
    let selector = selector("#gdt a");
    let mut links = Vec::new();

    for anchor in html.select(&selector) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        let Ok(url) = Url::parse(&absolute_url(BASE_URL, href)) else {
            continue;
        };
        let Some((number, page_gid)) = parse_page_path(&url) else {
            continue;
        };
        if page_gid == gid {
            links.push((number, url));
        }
    }

    Ok(links)
}

fn parse_page_path(url: &Url) -> Option<(u32, String)> {
    let segments = url.path_segments()?.collect::<Vec<_>>();
    if segments.len() < 3 || segments[0] != "s" {
        return None;
    }
    let (page_gid, number) = segments[2].rsplit_once('-')?;
    Some((number.parse().ok()?, page_gid.to_owned()))
}

fn page_number(url: &Url) -> Option<u32> {
    parse_page_path(url).map(|(number, _)| number)
}

fn parse_image_page(body: &[u8]) -> Result<Url> {
    let html = Html::parse_document(&String::from_utf8_lossy(body));
    let selector = selector("img#img");
    let src = html
        .select(&selector)
        .next()
        .and_then(|image| image.value().attr("src"))
        .map(str::trim)
        .filter(|src| !src.is_empty() && !src.starts_with("data:"))
        .ok_or_else(|| AppError::NotFound {
            resource: "E-Hentai image page".into(),
        })?;

    Url::parse(&absolute_url(BASE_URL, src))
        .map_err(|error| AppError::Unexpected(format!("invalid E-Hentai image URL: {error}")))
}

fn is_image_page(url: &Url) -> bool {
    matches!(
        url.host_str(),
        Some(host)
            if host.eq_ignore_ascii_case("e-hentai.org")
                || host.eq_ignore_ascii_case("exhentai.org")
    ) && url.path().starts_with("/s/")
}

fn selector(raw: &str) -> Selector {
    Selector::parse(raw).unwrap_or_else(|error| panic!("invalid selector `{raw}`: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> Url {
        Url::parse(raw).unwrap()
    }

    #[test]
    fn normalizes_hyphen_slash_and_url_work_ids() {
        for raw in [
            "4286553-7752f150d6",
            "4286553/7752f150d6",
            "https://e-hentai.org/g/4286553/7752f150d6/",
            "https://e-hentai.org/g/4286553/7752f150d6/?p=3",
        ] {
            assert_eq!(
                Ehentai.normalize_work_id(raw).unwrap(),
                "4286553-7752f150d6"
            );
        }

        for raw in [
            "",
            "4286553",
            "4286553-xyz",
            "4286553-7752f150d",
            "../4286553-7752f150d6",
            "https://e-hentai.org/s/7752f150d6/4286553-1",
        ] {
            assert!(Ehentai.normalize_work_id(raw).is_err(), "accepted {raw:?}");
        }
    }

    #[test]
    fn builds_first_page_and_requires_cursor_for_later_pages() {
        let first = Ehentai.search_request("hello world", 1).unwrap();
        assert_eq!(
            first.url.as_str(),
            "https://e-hentai.org/?f_search=hello+world&inline_set=dm_l"
        );

        assert!(Ehentai.search_request("hello world", 2).is_err());

        store_cursor(
            "hello world",
            2,
            Some("https://e-hentai.org/?f_search=hello+world&next=42"),
        );
        let second = Ehentai.search_request("hello world", 2).unwrap();
        assert_eq!(
            second.url.as_str(),
            "https://e-hentai.org/?f_search=hello+world&next=42"
        );
    }

    #[test]
    fn parses_search_rows_next_cursor_and_cumulative_total() {
        let body = r#"
        <table class="itg gltc"><tr><th>Title</th></tr>
        <tr>
          <td class="gl1c glcat"><div class="cn ct2">Doujinshi</div></td>
          <td class="gl2c"><div class="glthumb"><div><img src="data:image/gif;base64,AAAA" data-src="https://ehgt.org/w/1/thumb.webp" /></div></div></td>
          <td class="gl3c glname">
            <a href="https://e-hentai.org/g/4286553/7752f150d6/">
              <div class="glink">Example Gallery</div>
              <div class="gt" title="language:english"></div>
              <div class="gt" title="language:translated"></div>
              <div class="gt" title="artist:tooop"></div>
            </a>
          </td>
          <td class="gl4c glhide"><div><a href="https://e-hentai.org/uploader/x">x</a></div><div>266 pages</div></td>
        </tr>
        </table>
        <div class="searchnav"><a id="dnext" href="https://e-hentai.org/?f_search=x&amp;next=42">Next</a></div>
        "#;

        let response = Ehentai.parse_search("x", 1, body.as_bytes()).unwrap();

        assert_eq!(response.page_size, 25);
        assert_eq!(response.total, 1);
        assert_eq!(response.has_next, Some(true));
        assert_eq!(response.result.len(), 1);
        assert_eq!(response.result[0].id, "4286553-7752f150d6");
        assert_eq!(response.result[0].title, "Example Gallery");
        assert_eq!(response.result[0].thumb, "https://ehgt.org/w/1/thumb.webp");

        let extra: EhentaiSearchExtra =
            serde_json::from_value(response.result[0].extra.clone()).unwrap();
        assert_eq!(extra.category, "Doujinshi");
        assert_eq!(extra.pages, Some(266));
        assert_eq!(extra.languages, ["english", "translated"]);
    }

    #[test]
    fn gdata_request_is_post_with_the_expected_body() {
        let spec = Ehentai.detail_request("4286553-7752f150d6").unwrap();

        assert_eq!(spec.method, reqwest::Method::POST);
        assert_eq!(spec.url.as_str(), API_URL);
        let body: serde_json::Value =
            serde_json::from_slice(spec.body.as_deref().unwrap()).unwrap();
        assert_eq!(body["method"], "gdata");
        assert_eq!(body["gidlist"][0][0], 4286553);
        assert_eq!(body["gidlist"][0][1], "7752f150d6");
        assert_eq!(body["namespace"], 1);
    }

    #[test]
    fn parses_gdata_detail_into_typed_extra() {
        let body = r#"
        {"gmetadata":[{
          "gid":4286553,
          "token":"7752f150d6",
          "title":"Example Gallery",
          "title_jpn":"サンプル",
          "category":"Doujinshi",
          "thumb":"https://ehgt.org/w/1/cover.webp",
          "uploader":"uploader",
          "posted":"1793153501",
          "filecount":"266",
          "filesize":599167632,
          "rating":"4.23",
          "torrentcount":"1",
          "tags":["language:english","language:translated","artist:tooop"],
          "expunged":false
        }]}
        "#;

        let detail = Ehentai
            .parse_detail("4286553-7752f150d6", body.as_bytes())
            .unwrap();
        assert_eq!(detail.platform, "ehentai");
        assert_eq!(detail.title, "Example Gallery");
        assert_eq!(detail.thumb, "https://ehgt.org/w/1/cover.webp");
        assert_eq!(
            detail.chapters,
            vec![DetailChapter {
                id: None,
                title: None,
            }]
        );

        let extra: EhentaiDetailExtra = serde_json::from_value(detail.extra).unwrap();
        assert_eq!(extra.gid, 4286553);
        assert_eq!(extra.token, "7752f150d6");
        assert_eq!(extra.title_jpn.as_deref(), Some("サンプル"));
        assert_eq!(extra.category, "Doujinshi");
        assert_eq!(extra.pages, Some(266));
        assert_eq!(extra.file_size, Some(599167632));
        assert_eq!(extra.rating, Some(4.23));
        assert_eq!(extra.torrent_count, Some(1));
        assert_eq!(extra.languages, ["english", "translated"]);
        assert_eq!(extra.tags.len(), 3);
        assert!(!extra.expunged);
    }

    #[test]
    fn missing_gdata_is_not_found() {
        let error = Ehentai
            .parse_detail("4286553-7752f150d6", br#"{"gmetadata":[]}"#)
            .unwrap_err();
        assert_eq!(error.code(), crate::error::ErrorCode::NotFound);
    }

    #[test]
    fn parses_gallery_page_links_in_page_order() {
        let body = r#"
        <div id="gdt">
          <a href="https://e-hentai.org/s/aaaa/4286553-2"><div title="Page 2"></div></a>
          <a href="https://e-hentai.org/s/bbbb/4286553-1"><div title="Page 1"></div></a>
          <a href="https://e-hentai.org/s/cccc/9999-1"><div title="Other gallery"></div></a>
        </div>
        "#;

        let links = parse_gallery_page(body.as_bytes(), "4286553").unwrap();
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].0, 2);
        assert_eq!(links[1].0, 1);
        assert_eq!(links[0].1.as_str(), "https://e-hentai.org/s/aaaa/4286553-2");
    }

    #[test]
    fn parses_the_image_source_from_a_show_page() {
        let body = br#"
        <div id="i3"><a href="https://e-hentai.org/s/next/1-2">
          <img id="img" src="https://example.hath.network:1167/h/abc/keystamp=1/file.webp" />
        </a></div>
        "#;

        let url = parse_image_page(body).unwrap();
        assert_eq!(
            url.as_str(),
            "https://example.hath.network:1167/h/abc/keystamp=1/file.webp"
        );
    }

    #[test]
    fn missing_image_source_is_not_found() {
        let error = parse_image_page(b"<html><body>This gallery is unavailable.</body></html>")
            .unwrap_err();
        assert_eq!(error.code(), crate::error::ErrorCode::NotFound);
    }

    #[test]
    fn matches_hosts_and_caches_image_pages_without_query() {
        assert!(Ehentai.matches(&parse("https://e-hentai.org/s/abc/1-1")));
        assert!(Ehentai.matches(&parse("https://ehgt.org/w/1/thumb.webp")));
        assert!(Ehentai.matches(&parse("https://x.hath.network:1167/h/a/b.webp")));
        assert!(!Ehentai.matches(&parse("https://example.com/a.jpg")));

        assert_eq!(
            Ehentai.cache_key_url(&parse("https://e-hentai.org/s/abc/1-1?nl=1")),
            "https://e-hentai.org/s/abc/1-1"
        );
        assert_eq!(
            Ehentai.cache_key_url(&parse("https://ehgt.org/w/1/thumb.webp")),
            "https://ehgt.org/w/1/thumb.webp"
        );
    }

    #[tokio::test]
    async fn resolve_image_url_leaves_non_page_urls_untouched() {
        let url = parse("https://ehgt.org/w/1/thumb.webp");
        let resolved = Ehentai
            .resolve_image_url(&HttpClient::default(), &url)
            .await
            .unwrap();
        assert_eq!(resolved, url);
    }

    #[test]
    fn parses_a_real_search_page_fixture() {
        let response = Ehentai
            .parse_search(
                "test",
                1,
                include_str!("ehentai/fixtures/search.html").as_bytes(),
            )
            .unwrap();

        assert_eq!(response.result.len(), 25);
        assert_eq!(response.has_next, Some(true));
        assert_eq!(response.result[0].id, "4286553-7752f150d6");
        assert_eq!(response.result[0].title, "[Artist] Tooop (TooopScoop)");
        assert_eq!(
            response.result[0].thumb,
            "https://ehgt.org/w/02/751/66865-p955r5as.webp"
        );

        let extra: EhentaiSearchExtra =
            serde_json::from_value(response.result[0].extra.clone()).unwrap();
        assert_eq!(extra.category, "Western");
        assert_eq!(extra.pages, Some(266));
        assert_eq!(extra.languages, ["textless narrative"]);
    }

    #[test]
    fn parses_a_real_gallery_page_fixture() {
        let links = parse_gallery_page(
            include_str!("ehentai/fixtures/gallery.html").as_bytes(),
            "4286553",
        )
        .unwrap();

        assert_eq!(links.len(), 20);
        assert_eq!(links[0].0, 1);
        assert_eq!(
            links[0].1.as_str(),
            "https://e-hentai.org/s/4f92b2083e/4286553-1"
        );
        assert_eq!(links[19].0, 20);
    }

    #[test]
    fn parses_a_real_image_page_fixture() {
        let url =
            parse_image_page(include_str!("ehentai/fixtures/image-page.html").as_bytes()).unwrap();

        assert_eq!(url.host_str(), Some("zmjokgb.stclcqukkdxu.hath.network"));
        assert_eq!(url.port(), Some(1167));
        assert!(url.path().contains("keystamp="));
    }

    #[test]
    fn matches_the_platform_wire_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../src/platforms/ehentai/fixtures/wire.json"
        ))
        .unwrap();

        let search = SearchItem::new(
            Ehentai::ID,
            "4286553-7752f150d6",
            "E-Hentai Work",
            "https://ehgt.org/w/1/thumb.webp",
            EhentaiSearchExtra {
                category: "Doujinshi".into(),
                languages: vec!["english".into(), "translated".into()],
                pages: Some(24),
            },
        )
        .unwrap();
        assert_eq!(serde_json::to_value(search).unwrap(), fixture["search"]);

        let detail = ComicDetail::new(
            Ehentai::ID,
            "E-Hentai Work",
            "https://ehgt.org/w/1/cover.webp",
            vec![DetailChapter {
                id: None,
                title: None,
            }],
            EhentaiDetailExtra {
                gid: 4_286_553,
                token: "7752f150d6".into(),
                title_jpn: Some("サンプル".into()),
                category: "Doujinshi".into(),
                uploader: Some("uploader".into()),
                posted: Some(1_793_153_501),
                pages: Some(24),
                file_size: Some(123_456),
                rating: Some(4.23),
                torrent_count: Some(1),
                expunged: false,
                languages: vec!["english".into(), "translated".into()],
                tags: vec![EhentaiTag {
                    namespace: "artist".into(),
                    name: "tooop".into(),
                }],
            },
        )
        .unwrap();
        assert_eq!(serde_json::to_value(detail).unwrap(), fixture["detail"]);
    }
}
