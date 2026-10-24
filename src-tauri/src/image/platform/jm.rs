use std::sync::LazyLock;

use html_escape::decode_html_entities;
use regex::Regex;
use reqwest::RequestBuilder;
use reqwest::header::{ACCEPT_LANGUAGE, REFERER};
use scraper::{ElementRef, Html, Selector};
use serde::Serialize;
use url::Url;

use crate::error::{AppError, Result};
use crate::image::model::{ComicDetail, JmExtra, SearchExtra, SearchItem, SearchResponse};

use super::{Platform, RequestSpec, absolute_url, collapse_whitespace, host_matches, jm_id};

const BASE_URL: &str = "https://18comic.vip";
const HOST_SUFFIXES: &[&str] = &[
    "18comic.vip",
    "18comic.org",
    "18comic.me",
    "jmcomic.me",
    "jmcomic.org",
    "jmcomic.vip",
    "jmapiproxy.com",
];
const REFERER_VALUE: &str = "https://18comic.vip/";
const ACCEPT_LANGUAGE_VALUE: &str = "zh-CN,zh;q=0.9,en;q=0.8";
const PAGE_SIZE: u32 = 80;

static TOTAL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)class="text-white">(\d+)</span>\s*A漫"#).expect("valid total regex")
});
static TOTAL_FALLBACK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:共|找到|約|约)\s*(\d+)\s*(?:本|個|个|部|筆|笔)")
        .expect("valid total fallback regex")
});
static ITEM_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)<a href="/album/(\d+)/[\s\S]*?title="(.*?)"([\s\S]*?)<div class="title-truncate tags .*?>([\s\S]*?)</div>"#,
    )
    .expect("valid search item regex")
});
static IMAGE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?:data-original|src)="([^"]+)""#).expect("valid image regex"));
static TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<a[^>]*>([^<]*)</a>").expect("valid tag regex"));
static PAGE_COUNT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(\d+)").expect("valid page count regex"));
static PUB_DATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"上架日期\s*[:：]\s*([0-9-]+)").expect("valid publish date regex")
});
static UPDATE_DATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"更新日期\s*[:：]\s*([0-9-]+)").expect("valid update date regex"));
static LIKES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"id="albim_likes_\d+">([^<]*)<"#).expect("valid likes regex"));
static VIEWS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<span>([^<]*)</span>\s*<span>(?:次觀看|观看次数)</span>")
        .expect("valid views regex")
});
static COMMENT_COUNT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"id="total_video_comments">(\d+)<"#).expect("valid comment count regex")
});

pub(super) struct Jm;

impl Platform for Jm {
    fn id(&self) -> &'static str {
        "jm"
    }

    /// JM image URLs carry version/signature query parameters; they are
    /// variants of the same asset and should share one cache entry.
    fn cache_version(&self) -> u32 {
        2
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
        let mut cache_url = url.clone();
        cache_url.set_query(None);
        cache_url.to_string()
    }

    fn search_request(&self, query: &str, page: u32) -> Result<RequestSpec> {
        let mut url = Url::parse(&format!("{BASE_URL}/search/photos"))
            .map_err(|error| AppError::Unexpected(format!("invalid jm base URL: {error}")))?;
        url.query_pairs_mut()
            .append_pair("main_tag", "0")
            .append_pair("search_query", query)
            .append_pair("page", &page.to_string())
            .append_pair("o", "mr")
            .append_pair("t", "a");
        Ok(RequestSpec::html(url))
    }

    fn parse_search(&self, _query: &str, _page: u32, body: &[u8]) -> Result<SearchResponse> {
        let text = String::from_utf8_lossy(body);
        if text.contains("Just a moment") || text.contains("cf-challenge") {
            return Err(AppError::Unexpected(
                "jm returned a Cloudflare challenge instead of search results".into(),
            ));
        }
        if text.contains("關鍵字過短") || text.contains("关键字过短") {
            return Err(AppError::InvalidInput {
                field: "query",
                reason: "keyword is too short for jm".into(),
            });
        }

        let total = TOTAL_RE
            .captures(&text)
            .or_else(|| TOTAL_FALLBACK_RE.captures(&text))
            .and_then(|captures| captures[1].parse::<u64>().ok())
            .unwrap_or(0);

        let mut result = Vec::new();
        for captures in ITEM_RE.captures_iter(&text) {
            let id = captures[1].to_string();
            let title = decode_text(&captures[2]);
            // The cover can appear before the title attribute inside the same
            // result block, so scan the whole match, not just the captured tail.
            let thumb = extract_thumb(&captures[0]).unwrap_or_default();
            let tags = TAG_RE
                .captures_iter(&captures[4])
                .map(|tag| decode_text(&tag[1]))
                .filter(|tag| !tag.is_empty())
                .collect::<Vec<_>>();

            result.push(SearchItem::new(
                id,
                title,
                thumb,
                SearchExtra::Jm(JmExtra { tags }),
            ));
        }

        Ok(SearchResponse {
            total: total.max(result.len() as u64),
            page_size: PAGE_SIZE,
            result,
        })
    }

    fn detail_request(&self, id: &str) -> Result<RequestSpec> {
        let id = jm_id(id)?;
        let mut url = Url::parse(&format!("{BASE_URL}/album"))
            .map_err(|error| AppError::Unexpected(format!("invalid jm base URL: {error}")))?;
        url.path_segments_mut()
            .map_err(|_| AppError::Unexpected("jm album URL cannot be a base".into()))?
            .push(&id)
            .push("");
        Ok(RequestSpec::html(url))
    }

    fn parse_detail(&self, id: &str, body: &[u8]) -> Result<ComicDetail> {
        let raw = String::from_utf8_lossy(body);
        let html = Html::parse_document(&raw);
        let root = html.root_element();

        let Some(title) = text_of(root, "h1.book-name")
            .or_else(|| attr_of(root, "meta[property=\"og:title\"]", "content"))
        else {
            let page_text = collapse_whitespace(&root.text().collect::<String>());
            return Err(
                if page_text.contains("album_missing") || page_text.contains("頁面不存在") {
                    AppError::NotFound {
                        resource: format!("jm album `{id}`"),
                    }
                } else {
                    AppError::Unexpected(format!("failed to parse jm album `{id}`"))
                },
            );
        };

        let thumb = attr_of(root, "img[itemprop=\"image\"]", "src")
            .or_else(|| attr_of(root, ".album-cover img", "src"))
            .or_else(|| attr_of(root, ".book-cover img", "src"))
            .or_else(|| attr_of(root, "img#album_cover", "src"))
            .or_else(|| {
                attr_of(root, "meta[property=\"og:image\"]", "content")
                    .filter(|url| url.contains("/media/albums/"))
            })
            .map(|raw| absolute_url(BASE_URL, &raw))
            .unwrap_or_default();

        let page_text = collapse_whitespace(&root.text().collect::<String>());
        let page_count = text_of(root, ".pagecount").and_then(|text| {
            PAGE_COUNT_RE
                .find(&text)
                .and_then(|found| found.as_str().parse::<u32>().ok())
        });

        let extra = JmDetailExtra {
            authors: itemprop_links(root, "author", "author"),
            tags: itemprop_links(root, "genre", "tags"),
            works: itemprop_links(root, "author", "works"),
            actors: itemprop_links(root, "author", "actor"),
            page_count,
            pub_date: PUB_DATE_RE
                .captures(&page_text)
                .map(|captures| captures[1].to_string()),
            update_date: UPDATE_DATE_RE
                .captures(&page_text)
                .map(|captures| captures[1].to_string()),
            likes: LIKES_RE
                .captures(&raw)
                .map(|captures| captures[1].trim().to_string()),
            views: VIEWS_RE
                .captures(&raw)
                .map(|captures| captures[1].trim().to_string()),
            comment_count: COMMENT_COUNT_RE
                .captures(&raw)
                .and_then(|captures| captures[1].parse::<u64>().ok()),
        };

        Ok(ComicDetail {
            title,
            thumb,
            extra: serde_json::to_value(extra)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JmDetailExtra {
    authors: Vec<String>,
    tags: Vec<String>,
    works: Vec<String>,
    actors: Vec<String>,
    page_count: Option<u32>,
    pub_date: Option<String>,
    update_date: Option<String>,
    likes: Option<String>,
    views: Option<String>,
    comment_count: Option<u64>,
}

fn extract_thumb(block: &str) -> Option<String> {
    let mut candidate = None;

    for captures in IMAGE_RE.captures_iter(block) {
        let raw = decode_html_entities(&captures[1]).trim().to_string();
        if raw.is_empty()
            || raw.starts_with("data:")
            || raw.contains("/media/albums/blank")
            || raw.contains("blank.gif")
        {
            continue;
        }

        let absolute = absolute_url(BASE_URL, &raw);
        if captures[0].starts_with("data-original") {
            return Some(absolute);
        }
        candidate.get_or_insert(absolute);
    }

    candidate
}

fn decode_text(raw: &str) -> String {
    collapse_whitespace(&decode_html_entities(raw))
}

fn itemprop_links(scope: ElementRef<'_>, itemprop: &str, data_type: &str) -> Vec<String> {
    let css = format!("span[itemprop=\"{itemprop}\"][data-type=\"{data_type}\"] a");
    scope
        .select(&selector(&css))
        .map(|link| collapse_whitespace(&link.text().collect::<String>()))
        .filter(|text| !text.is_empty())
        .collect()
}

fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("static CSS selector")
}

fn text_of(scope: ElementRef<'_>, css: &str) -> Option<String> {
    scope
        .select(&selector(css))
        .next()
        .map(|element| collapse_whitespace(&element.text().collect::<String>()))
        .filter(|text| !text.is_empty())
}

fn attr_of(scope: ElementRef<'_>, css: &str, attr: &str) -> Option<String> {
    scope
        .select(&selector(css))
        .next()
        .and_then(|element| element.value().attr(attr))
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::model::SearchPlatform;

    const SEARCH_HTML: &str = r#"
<span class="text-white">2</span> A漫.
<a href="/album/123/" title="示例 &amp; 漫画"><img data-original="//cdn.example/1.jpg"></a>
<div class="title-truncate tags foo"><a>标签A</a><a>标签B</a></div>
<a href="/album/456/" title="第二个"><img src="https://cdn.example/2.jpg"></a>
<div class="title-truncate tags foo"><a>标签C</a></div>
"#;

    const DETAIL_HTML: &str = r#"
<meta property="og:image" content="https://cdn.example/logo.png">
<h1 class="book-name" id="book-name">示例漫画</h1>
<img itemprop="image" src="https://cdn.example/cover.jpg">
<span itemprop="author" data-type="author"><a>作者A</a></span>
<span itemprop="genre" data-type="tags"><a>标签A</a></span>
<span itemprop="author" data-type="works"><a>作品A</a></span>
<span itemprop="author" data-type="actor"><a>角色A</a></span>
<span class="pagecount">頁數:42</span>
<span>上架日期 : 2020-01-02</span>
<span>更新日期 : 2020-02-03</span>
<span id="albim_likes_123">1K</span>
<span>12K</span><span>次觀看</span>
<div class="badge" id="total_video_comments">7</div>
"#;

    #[test]
    fn builds_search_and_detail_urls() {
        let search = Jm.search_request("test 1", 2).unwrap();
        let url = search.url.as_str();
        assert!(url.starts_with("https://18comic.vip/search/photos?"));
        assert!(url.contains("search_query=test+1"));
        assert!(url.contains("page=2"));

        let detail = Jm.detail_request("JM123").unwrap();
        assert_eq!(detail.url.as_str(), "https://18comic.vip/album/123/");
        assert!(Jm.detail_request("123/456").is_err());
    }

    #[test]
    fn parses_search_results() {
        let response = Jm.parse_search("test", 1, SEARCH_HTML.as_bytes()).unwrap();
        assert_eq!(response.total, 2);
        assert_eq!(response.page_size, 80);
        assert_eq!(response.result.len(), 2);

        let first = &response.result[0];
        assert_eq!(first.platform, SearchPlatform::Jm);
        assert_eq!(first.id, "123");
        assert_eq!(first.title, "示例 & 漫画");
        assert_eq!(first.thumb, "https://cdn.example/1.jpg");

        let SearchExtra::Jm(extra) = &first.extra else {
            panic!("expected jm search extra");
        };
        assert_eq!(extra.tags, ["标签A", "标签B"]);
    }

    #[test]
    fn finds_cover_before_the_title_attribute() {
        let block = r#"<a href="/album/1/"><img data-original="//cdn.example/3.jpg"><div title="第三个"></div>"#;
        assert_eq!(
            extract_thumb(block).as_deref(),
            Some("https://cdn.example/3.jpg")
        );
    }

    #[test]
    fn parses_detail() {
        let detail = Jm.parse_detail("123", DETAIL_HTML.as_bytes()).unwrap();
        assert_eq!(detail.title, "示例漫画");
        assert_eq!(detail.thumb, "https://cdn.example/cover.jpg");
        assert_eq!(detail.extra["authors"], serde_json::json!(["作者A"]));
        assert_eq!(detail.extra["tags"], serde_json::json!(["标签A"]));
        assert_eq!(detail.extra["works"], serde_json::json!(["作品A"]));
        assert_eq!(detail.extra["actors"], serde_json::json!(["角色A"]));
        assert_eq!(detail.extra["pageCount"], 42);
        assert_eq!(detail.extra["pubDate"], "2020-01-02");
        assert_eq!(detail.extra["updateDate"], "2020-02-03");
        assert_eq!(detail.extra["likes"], "1K");
        assert_eq!(detail.extra["views"], "12K");
        assert_eq!(detail.extra["commentCount"], 7);
    }

    #[test]
    fn cache_key_ignores_image_query() {
        let signed =
            Url::parse("https://cdn-msp3.18comic.vip/media/albums/1480638_3x4.jpg?v=1").unwrap();
        assert_eq!(
            Jm.cache_key_url(&signed),
            "https://cdn-msp3.18comic.vip/media/albums/1480638_3x4.jpg"
        );

        let plain =
            Url::parse("https://cdn-msp3.18comic.vip/media/albums/1480638_3x4.jpg").unwrap();
        assert_eq!(Jm.cache_key_url(&plain), plain.as_str());
    }
}
