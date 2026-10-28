//! Pixiv artworks through the public web AJAX endpoints.
//!
//! Only anonymously accessible works are supported. Pixiv answers the anonymous
//! detail and page endpoints with 404 for R-18 and login-only artworks, so the
//! catalog stays anonymous instead of persisting an account cookie. Search
//! results hide the R-18/masked and ugoira entries the anonymous API can still
//! return; ugoira has no animated representation in the reader, only its first
//! static frame.
//!
//! A multi-page artwork is one implicit chapter. The reader page list mirrors
//! Pixiv's order and uses the `original` image when the page endpoint provides
//! it. `i.pixiv.re` is a fallback host only; the cache key is normalized back to
//! `i.pximg.net` so both hosts share one cached file.

use std::future::Future;
use std::pin::Pin;

use reqwest::RequestBuilder;
use reqwest::header::{ACCEPT_LANGUAGE, REFERER};
use scraper::Html;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::error::{AppError, Result};
use crate::image::HttpClient;
use crate::image::http;
use crate::platforms::model::{
    ComicDetail, DetailChapter, ReaderChapter, ReaderPage, SearchItem, SearchResponse,
};

use super::{Catalog, ImageProfile, RequestSpec, collapse_whitespace, host_matches, numeric_id};

const BASE_URL: &str = "https://www.pixiv.net";
const HOST_SUFFIXES: &[&str] = &["pixiv.net", "pximg.net", "pixiv.re"];
const REFERER_VALUE: &str = "https://www.pixiv.net/";
const ACCEPT_LANGUAGE_VALUE: &str = "zh-CN,zh;q=0.9,en;q=0.8";
const PAGE_SIZE: u32 = 60;
const IMAGE_PROXY_HOST: &str = "i.pixiv.re";
const IMAGE_HOST: &str = "i.pximg.net";

/// Search result metadata. The cover is a square thumbnail from the search
/// endpoint; the detail hero uses the larger aspect-preserving `small` image.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PixivSearchExtra {
    user_id: String,
    user_name: String,
    illust_type: u8,
    x_restrict: u8,
    page_count: u32,
    width: u32,
    height: u32,
    tags: Vec<String>,
    create_date: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PixivDetailExtra {
    user_id: String,
    user_name: String,
    user_account: String,
    illust_type: u8,
    x_restrict: u8,
    page_count: u32,
    width: u32,
    height: u32,
    bookmark_count: Option<u64>,
    like_count: Option<u64>,
    view_count: Option<u64>,
    create_date: String,
    description: Option<String>,
    series_title: Option<String>,
    tags: Vec<PixivTag>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PixivTag {
    name: String,
    translated: Option<String>,
}

pub(super) struct Pixiv;

impl Pixiv {
    pub(super) const ID: &'static str = "pixiv";
}

impl ImageProfile for Pixiv {
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
        if url.host_str() == Some(IMAGE_PROXY_HOST) {
            let mut normalized = url.clone();
            if normalized.set_host(Some(IMAGE_HOST)).is_ok() {
                return normalized.to_string();
            }
        }
        url.to_string()
    }

    fn image_fallbacks(&self, url: &Url) -> Vec<Url> {
        if url.host_str() != Some(IMAGE_HOST) {
            return Vec::new();
        }

        let mut alternate = url.clone();
        if alternate.set_host(Some(IMAGE_PROXY_HOST)).is_ok() {
            vec![alternate]
        } else {
            Vec::new()
        }
    }
}

impl Catalog for Pixiv {
    fn single_chapter(&self) -> bool {
        true
    }

    fn search_request(&self, query: &str, page: u32) -> Result<RequestSpec> {
        let mut url = Url::parse(&format!("{BASE_URL}/ajax/search/artworks"))
            .map_err(|error| AppError::Unexpected(format!("invalid pixiv search URL: {error}")))?;
        url.path_segments_mut()
            .map_err(|_| AppError::Unexpected("pixiv search URL cannot carry a path".into()))?
            .push(query);
        url.query_pairs_mut()
            .append_pair("word", query)
            .append_pair("order", "date_d")
            .append_pair("mode", "all")
            .append_pair("p", &page.to_string())
            .append_pair("s_mode", "s_tag_full")
            .append_pair("type", "all")
            .append_pair("lang", "zh");
        Ok(RequestSpec::json(url))
    }

    fn parse_search(&self, _query: &str, _page: u32, body: &[u8]) -> Result<SearchResponse> {
        let body = envelope_body(body, "pixiv search results")?;
        let body: ApiSearchBody = serde_json::from_value(body)?;
        let section = body.illust_manga;

        // Pixiv's anonymous search is already safe-mode, but masking the
        // restricted entries here keeps an unexpected payload from producing a
        // card that cannot open. Ugoira would only show its first frame.
        let result = section
            .data
            .into_iter()
            .filter(|work| work.illust_type != 2 && work.x_restrict == 0 && !work.is_masked)
            .map(|work| -> Result<SearchItem> {
                let title = non_empty(&work.title).unwrap_or_else(|| format!("#{}", work.id));
                let extra = PixivSearchExtra {
                    user_id: work.user_id,
                    user_name: work.user_name,
                    illust_type: work.illust_type,
                    x_restrict: work.x_restrict,
                    page_count: work.page_count,
                    width: work.width,
                    height: work.height,
                    tags: work.tags,
                    create_date: work.create_date,
                };
                SearchItem::new(Pixiv::ID, work.id, title, work.url, extra)
            })
            .collect::<Result<Vec<_>>>()?;

        // Anonymous search only exposes the first ten pages (600 works) even
        // though the payload reports the site-wide total. Reporting the
        // accessible count keeps the numeric pagination from requesting pages
        // that Pixiv resolves to the last one.
        let total = section
            .total
            .min(u64::from(section.last_page.max(1)) * u64::from(PAGE_SIZE));

        Ok(SearchResponse {
            total,
            page_size: PAGE_SIZE,
            result,
            has_next: None,
        })
    }

    fn detail_request(&self, id: &str) -> Result<RequestSpec> {
        let id = numeric_id(id, "pixiv")?;
        let mut url = Url::parse(&format!("{BASE_URL}/ajax/illust/{id}"))
            .map_err(|error| AppError::Unexpected(format!("invalid pixiv detail URL: {error}")))?;
        url.query_pairs_mut().append_pair("lang", "zh");
        Ok(RequestSpec::json(url))
    }

    fn parse_detail(&self, id: &str, body: &[u8]) -> Result<ComicDetail> {
        let body = envelope_body(body, &format!("pixiv artwork `{id}`"))?;
        let detail: ApiDetail = serde_json::from_value(body)?;
        build_detail(id, detail)
    }

    fn comic_detail<'a>(
        &'a self,
        client: &'a HttpClient,
        id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<ComicDetail>> + Send + 'a>> {
        Box::pin(async move {
            let id = numeric_id(id, "pixiv")?;
            let spec = self.detail_request(&id)?;
            let fetched = fetch_or_not_found(client, &spec, &id).await?;
            self.parse_detail(&id, &fetched.body)
        })
    }

    fn reader_chapter_request(&self, id: &str, _chapter_id: Option<&str>) -> Result<RequestSpec> {
        let id = numeric_id(id, "pixiv")?;
        let mut url = Url::parse(&format!("{BASE_URL}/ajax/illust/{id}/pages"))
            .map_err(|error| AppError::Unexpected(format!("invalid pixiv pages URL: {error}")))?;
        url.query_pairs_mut().append_pair("lang", "zh");
        Ok(RequestSpec::json(url))
    }

    fn parse_reader_chapter(
        &self,
        id: &str,
        _chapter_id: Option<&str>,
        body: &[u8],
    ) -> Result<ReaderChapter> {
        let pages = parse_pages(body)?;
        if pages.is_empty() {
            return Err(AppError::NotFound {
                resource: format!("pixiv artwork `{id}` pages"),
            });
        }

        // The detail fetch in `reader_chapter` replaces these fallbacks with
        // the work title and cover, which the pages endpoint does not carry.
        Ok(ReaderChapter {
            title: format!("#{id}"),
            thumb: String::new(),
            chapter_id: None,
            chapter_title: None,
            pages,
            previous: None,
            next: None,
        })
    }

    fn reader_chapter<'a>(
        &'a self,
        client: &'a HttpClient,
        id: &'a str,
        _chapter_id: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<ReaderChapter>> + Send + 'a>> {
        Box::pin(async move {
            let id = numeric_id(id, "pixiv")?;
            let pages_spec = self.reader_chapter_request(&id, None)?;
            let detail_spec = self.detail_request(&id)?;

            // Metadata and page URLs are independent requests. The detail
            // endpoint supplies the title and cover used by history; the pages
            // endpoint supplies the images.
            let (pages_fetched, detail_fetched) = tokio::join!(
                fetch_or_not_found(client, &pages_spec, &id),
                fetch_or_not_found(client, &detail_spec, &id),
            );

            let pages_fetched = pages_fetched?;
            let detail_fetched = detail_fetched?;
            let mut chapter = self.parse_reader_chapter(&id, None, &pages_fetched.body)?;
            let detail = self.parse_detail(&id, &detail_fetched.body)?;
            chapter.title = detail.title;
            chapter.thumb = detail.thumb;
            Ok(chapter)
        })
    }
}

#[derive(Deserialize)]
struct ApiEnvelope {
    error: bool,
    body: Value,
}

fn envelope_body(body: &[u8], resource: &str) -> Result<Value> {
    let envelope: ApiEnvelope = serde_json::from_slice(body)?;
    if envelope.error {
        return Err(AppError::NotFound {
            resource: resource.into(),
        });
    }
    Ok(envelope.body)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiSearchBody {
    illust_manga: ApiSearchSection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiSearchSection {
    data: Vec<ApiSearchWork>,
    total: u64,
    last_page: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiSearchWork {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    illust_type: u8,
    #[serde(default)]
    x_restrict: u8,
    #[serde(default)]
    url: String,
    #[serde(default)]
    user_id: String,
    #[serde(default)]
    user_name: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    page_count: u32,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    create_date: String,
    #[serde(default)]
    is_masked: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiDetail {
    #[serde(default)]
    illust_title: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    illust_type: u8,
    #[serde(default)]
    x_restrict: u8,
    #[serde(default)]
    urls: ApiImageUrls,
    #[serde(default)]
    tags: ApiTagList,
    #[serde(default)]
    user_id: String,
    #[serde(default)]
    user_name: String,
    #[serde(default)]
    user_account: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    page_count: u32,
    #[serde(default)]
    bookmark_count: Option<u64>,
    #[serde(default)]
    like_count: Option<u64>,
    #[serde(default)]
    view_count: Option<u64>,
    #[serde(default)]
    create_date: String,
    #[serde(default)]
    series_nav_data: Option<ApiSeriesNavData>,
}

#[derive(Default, Deserialize)]
struct ApiImageUrls {
    #[serde(default)]
    thumb: String,
    #[serde(default)]
    small: String,
    #[serde(default)]
    regular: String,
    #[serde(default)]
    original: String,
}

#[derive(Default, Deserialize)]
struct ApiTagList {
    #[serde(default)]
    tags: Vec<ApiTag>,
}

#[derive(Deserialize)]
struct ApiTag {
    #[serde(default)]
    tag: String,
    #[serde(default)]
    translation: Option<ApiTagTranslation>,
}

#[derive(Default, Deserialize)]
struct ApiTagTranslation {
    #[serde(default)]
    en: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiSeriesNavData {
    #[serde(default)]
    series_title: Option<String>,
}

#[derive(Deserialize)]
struct ApiPage {
    #[serde(default)]
    urls: ApiPageUrls,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
}

#[derive(Default, Deserialize)]
struct ApiPageUrls {
    #[serde(default)]
    small: String,
    #[serde(default)]
    regular: String,
    #[serde(default)]
    original: String,
}

fn build_detail(id: &str, detail: ApiDetail) -> Result<ComicDetail> {
    let title = non_empty(&detail.illust_title)
        .or_else(|| non_empty(&detail.title))
        .unwrap_or_else(|| format!("#{id}"));
    let thumb = first_non_empty([
        &detail.urls.small,
        &detail.urls.thumb,
        &detail.urls.regular,
        &detail.urls.original,
    ])
    .unwrap_or_default();
    let description = non_empty(&plain_text(&detail.description));
    let series_title = detail
        .series_nav_data
        .and_then(|series| series.series_title)
        .and_then(|title| non_empty(&title));
    let tags = detail
        .tags
        .tags
        .into_iter()
        .filter_map(|tag| {
            let name = non_empty(&tag.tag)?;
            Some(PixivTag {
                name,
                translated: tag
                    .translation
                    .and_then(|translation| translation.en)
                    .and_then(|value| non_empty(&value)),
            })
        })
        .collect();

    let extra = PixivDetailExtra {
        user_id: detail.user_id,
        user_name: detail.user_name,
        user_account: detail.user_account,
        illust_type: detail.illust_type,
        x_restrict: detail.x_restrict,
        page_count: detail.page_count,
        width: detail.width,
        height: detail.height,
        bookmark_count: detail.bookmark_count,
        like_count: detail.like_count,
        view_count: detail.view_count,
        create_date: detail.create_date,
        description,
        series_title,
        tags,
    };

    ComicDetail::new(
        Pixiv::ID,
        title,
        thumb,
        vec![DetailChapter {
            id: None,
            title: None,
        }],
        extra,
    )
}

fn parse_pages(body: &[u8]) -> Result<Vec<ReaderPage>> {
    let body = envelope_body(body, "pixiv artwork pages")?;
    let pages: Vec<ApiPage> = serde_json::from_value(body)?;

    Ok(pages
        .into_iter()
        .filter_map(|page| {
            let url = first_non_empty([&page.urls.original, &page.urls.regular, &page.urls.small])?;
            Some(ReaderPage {
                url,
                width: page.width,
                height: page.height,
                local_path: None,
            })
        })
        .collect())
}

async fn fetch_or_not_found(
    client: &HttpClient,
    spec: &RequestSpec,
    id: &str,
) -> Result<http::FetchedDocument> {
    match http::fetch_document(client, &Pixiv, spec).await {
        Ok(fetched) => Ok(fetched),
        // Anonymous Pixiv answers R-18, login-only and deleted works with 404.
        // Treat that as a missing artwork instead of a generic HTTP failure.
        Err(AppError::HttpStatus { status: 404, .. }) => Err(AppError::NotFound {
            resource: format!("pixiv artwork `{id}`"),
        }),
        Err(error) => Err(error),
    }
}

fn first_non_empty<'a>(values: impl IntoIterator<Item = &'a String>) -> Option<String> {
    values
        .into_iter()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
        .map(str::to_owned)
}

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// Pixiv descriptions are HTML fragments. The UI renders plain text only, so
/// tags are dropped and entities decoded before the value reaches the wire.
fn plain_text(raw: &str) -> String {
    let fragment = Html::parse_fragment(raw);
    let text = fragment.root_element().text().collect::<String>();
    collapse_whitespace(&html_escape::decode_html_entities(&text))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(raw: &str) -> Url {
        Url::parse(raw).unwrap()
    }

    #[test]
    fn builds_search_detail_and_page_urls() {
        let search = Pixiv.search_request("初音 ミク", 2).unwrap();
        let url = search.url.as_str();
        assert!(url.starts_with("https://www.pixiv.net/ajax/search/artworks/"));
        assert!(url.contains("%E5%88%9D%E9%9F%B3%20%E3%83%9F%E3%82%AF"));
        assert!(url.contains("word=%E5%88%9D%E9%9F%B3+%E3%83%9F%E3%82%AF"));
        assert!(url.contains("order=date_d"));
        assert!(url.contains("mode=all"));
        assert!(url.contains("p=2"));
        assert!(url.contains("s_mode=s_tag_full"));
        assert!(url.contains("type=all"));
        assert!(url.contains("lang=zh"));
        assert_eq!(search.accept, "application/json");

        let detail = Pixiv.detail_request("150699872").unwrap();
        assert_eq!(
            detail.url.as_str(),
            "https://www.pixiv.net/ajax/illust/150699872?lang=zh"
        );
        assert!(Pixiv.detail_request("../150699872").is_err());

        let pages = Pixiv.reader_chapter_request("150699872", None).unwrap();
        assert_eq!(
            pages.url.as_str(),
            "https://www.pixiv.net/ajax/illust/150699872/pages?lang=zh"
        );
        assert!(Pixiv.reader_chapter_request("abc", None).is_err());
    }

    #[test]
    fn parses_search_and_filters_ugoira() {
        let response = Pixiv
            .parse_search(
                "test",
                1,
                include_str!("pixiv/fixtures/search.json").as_bytes(),
            )
            .unwrap();

        assert_eq!(response.total, 600);
        assert_eq!(response.page_size, 60);
        assert_eq!(response.has_next, None);
        assert_eq!(response.result.len(), 2);
        assert_eq!(
            response
                .result
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            ["150699872", "150507655"]
        );

        let first = &response.result[0];
        assert_eq!(first.platform, "pixiv");
        assert_eq!(first.title, "オーズワン");
        assert_eq!(
            first.thumb,
            "https://i.pximg.net/c/250x250_80_a2/img-master/img/2026/10/11/00/52/56/150699872_p0_square1200.jpg"
        );

        let extra: PixivSearchExtra = serde_json::from_value(first.extra.clone()).unwrap();
        assert_eq!(extra.user_id, "83601061");
        assert_eq!(extra.user_name, "Ray");
        assert_eq!(extra.illust_type, 0);
        assert_eq!(extra.page_count, 15);
        assert_eq!(extra.width, 1254);
        assert_eq!(extra.height, 1254);
        assert_eq!(
            extra.tags,
            ["女子", "少女", "VOCALOID", "ミク", "初音", "テト"]
        );
        assert_eq!(extra.create_date, "2026-10-11T00:52:56+09:00");
    }

    #[test]
    fn search_drops_restricted_works() {
        let body = br#"{
          "error": false,
          "body": {
            "illustManga": {
              "data": [
                { "id": "1", "title": "safe", "illustType": 0, "xRestrict": 0, "isMasked": false },
                { "id": "2", "title": "r18", "illustType": 0, "xRestrict": 1, "isMasked": false },
                { "id": "3", "title": "masked", "illustType": 0, "xRestrict": 0, "isMasked": true }
              ],
              "total": 3,
              "lastPage": 1
            }
          }
        }"#;

        let response = Pixiv.parse_search("test", 1, body).unwrap();
        assert_eq!(response.result.len(), 1);
        assert_eq!(response.result[0].id, "1");
        assert_eq!(response.total, 3);
    }

    #[test]
    fn parses_detail_into_typed_extra() {
        let detail = Pixiv
            .parse_detail(
                "150699872",
                include_str!("pixiv/fixtures/detail.json").as_bytes(),
            )
            .unwrap();

        assert_eq!(detail.platform, "pixiv");
        assert_eq!(detail.title, "オーズワン");
        assert_eq!(
            detail.thumb,
            "https://i.pximg.net/c/540x540_70/img-master/img/2026/10/11/00/52/56/150699872_p0_master1200.jpg"
        );
        assert_eq!(
            detail.chapters,
            vec![DetailChapter {
                id: None,
                title: None,
            }]
        );

        let extra: PixivDetailExtra = serde_json::from_value(detail.extra).unwrap();
        assert_eq!(extra.user_id, "83601061");
        assert_eq!(extra.user_name, "Ray");
        assert_eq!(extra.user_account, "user_vnaf3524");
        assert_eq!(extra.page_count, 15);
        assert_eq!(extra.width, 1254);
        assert_eq!(extra.height, 1254);
        assert_eq!(extra.bookmark_count, Some(3));
        assert_eq!(extra.like_count, Some(3));
        assert_eq!(extra.view_count, Some(92));
        assert_eq!(extra.create_date, "2026-10-10T15:52:00+00:00");
        assert_eq!(
            extra.description.as_deref(),
            Some("https://youtube.com/shorts/lFZDW8evpMs?feature=share")
        );
        assert_eq!(extra.series_title, None);
        assert_eq!(extra.tags.len(), 6);
        assert_eq!(extra.tags[0].name, "女子");
        assert_eq!(extra.tags[0].translated.as_deref(), Some("woman"));
        assert_eq!(extra.tags[2].name, "VOCALOID");
        assert_eq!(extra.tags[2].translated, None);
    }

    #[test]
    fn parses_pages_with_original_urls() {
        let chapter = Pixiv
            .parse_reader_chapter(
                "150699872",
                None,
                include_str!("pixiv/fixtures/pages.json").as_bytes(),
            )
            .unwrap();

        assert_eq!(chapter.pages.len(), 2);
        assert_eq!(
            chapter.pages[0].url,
            "https://i.pximg.net/img-original/img/2026/10/11/00/52/56/150699872_p0.png"
        );
        assert_eq!(chapter.pages[0].width, Some(1254));
        assert_eq!(chapter.pages[0].height, Some(1254));
        assert_eq!(
            chapter.pages[1].url,
            "https://i.pximg.net/img-original/img/2026/10/11/00/52/56/150699872_p1.png"
        );
    }

    #[test]
    fn maps_error_envelopes_and_empty_pages_to_not_found() {
        let error = Pixiv
            .parse_detail("42", br#"{"error":true,"message":"","body":[]}"#)
            .unwrap_err();
        assert_eq!(error.code(), crate::error::ErrorCode::NotFound);

        let error = Pixiv
            .parse_reader_chapter("42", None, br#"{"error":true,"message":"","body":[]}"#)
            .unwrap_err();
        assert_eq!(error.code(), crate::error::ErrorCode::NotFound);

        let error = Pixiv
            .parse_reader_chapter("42", None, br#"{"error":false,"body":[]}"#)
            .unwrap_err();
        assert_eq!(error.code(), crate::error::ErrorCode::NotFound);
    }

    #[test]
    fn uses_the_pixiv_image_proxy_only_as_a_fallback() {
        let primary =
            parse("https://i.pximg.net/img-original/img/2026/10/11/00/52/56/150699872_p0.png");
        let proxy =
            parse("https://i.pixiv.re/img-original/img/2026/10/11/00/52/56/150699872_p0.png");

        assert_eq!(Pixiv.image_fallbacks(&primary), vec![proxy.clone()]);
        assert!(Pixiv.image_fallbacks(&proxy).is_empty());
        assert_eq!(Pixiv.cache_key_url(&proxy), Pixiv.cache_key_url(&primary));
        assert_eq!(Pixiv.cache_key_url(&proxy), primary.to_string());
    }

    #[test]
    fn matches_pixiv_image_hosts() {
        assert!(Pixiv.matches(&parse("https://i.pximg.net/a.jpg")));
        assert!(Pixiv.matches(&parse("https://www.pixiv.net/artworks/1")));
        assert!(Pixiv.matches(&parse("https://i.pixiv.re/a.jpg")));
        assert!(!Pixiv.matches(&parse("https://notpixiv.net/a.jpg")));
        assert!(!Pixiv.matches(&parse("https://pixiv.example.com/a.jpg")));
    }

    #[test]
    fn matches_the_platform_wire_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../src/platforms/pixiv/fixtures/wire.json"
        ))
        .unwrap();

        let response = Pixiv
            .parse_search(
                "test",
                1,
                include_str!("pixiv/fixtures/search.json").as_bytes(),
            )
            .unwrap();
        assert_eq!(
            serde_json::to_value(&response.result[0]).unwrap(),
            fixture["search"]
        );

        let detail = Pixiv
            .parse_detail(
                "150699872",
                include_str!("pixiv/fixtures/detail.json").as_bytes(),
            )
            .unwrap();
        assert_eq!(serde_json::to_value(detail).unwrap(), fixture["detail"]);
    }
}
