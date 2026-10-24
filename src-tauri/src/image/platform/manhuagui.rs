use std::sync::LazyLock;

use regex::Regex;
use reqwest::RequestBuilder;
use reqwest::header::{ACCEPT_LANGUAGE, REFERER};
use scraper::{ElementRef, Html, Selector};
use url::Url;

use crate::error::{AppError, Result};
use crate::image::model::{ComicDetail, ManhuaguiExtra, SearchExtra, SearchItem, SearchResponse};

use super::{Platform, RequestSpec, absolute_url, collapse_whitespace, host_matches, numeric_id};

const BASE_URL: &str = "https://www.manhuagui.com";
const HOST_SUFFIXES: &[&str] = &["manhuagui.com", "mhgui.com", "hamreus.com"];
const REFERER_VALUE: &str = "https://www.manhuagui.com/";
const ACCEPT_LANGUAGE_VALUE: &str = "zh-CN,zh;q=0.9,en;q=0.8";
const PAGE_SIZE: u32 = 10;

static TOTAL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"共查找到\s*(\d+)\s*部漫画").expect("valid total regex"));
static STATUS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(连载中|已完结)").expect("valid status regex"));
static DATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d{4}-\d{2}-\d{2}(?: \d{2}:\d{2})?").expect("valid date regex"));

pub(super) struct Manhuagui;

impl Platform for Manhuagui {
    fn id(&self) -> &'static str {
        "manhuagui"
    }

    /// Query strings on manhuagui image URLs are signed/expiring variants of
    /// the same asset, so they must not create separate cache entries.
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
        let filename = if page == 1 {
            format!("{query}.html")
        } else {
            format!("{query}_p{page}.html")
        };
        let mut url = Url::parse(&format!("{BASE_URL}/s")).map_err(|error| {
            AppError::Unexpected(format!("invalid manhuagui base URL: {error}"))
        })?;
        url.path_segments_mut()
            .map_err(|_| AppError::Unexpected("manhuagui search URL cannot be a base".into()))?
            .push(&filename);
        Ok(RequestSpec::html(url))
    }

    fn parse_search(&self, _query: &str, _page: u32, body: &[u8]) -> Result<SearchResponse> {
        let html = Html::parse_document(&String::from_utf8_lossy(body));
        let total = html
            .select(&selector(".result-count"))
            .next()
            .and_then(|element| {
                let text = element.text().collect::<String>();
                TOTAL_RE
                    .captures(&text)
                    .and_then(|captures| captures[1].parse::<u64>().ok())
            })
            .unwrap_or(0);

        let result: Vec<SearchItem> = html
            .select(&selector("div.book-result li"))
            .filter_map(parse_search_item)
            .collect();

        Ok(SearchResponse {
            total: total.max(result.len() as u64),
            page_size: PAGE_SIZE,
            result,
        })
    }

    fn detail_request(&self, id: &str) -> Result<RequestSpec> {
        let id = numeric_id(id, "manhuagui")?;
        let mut url = Url::parse(&format!("{BASE_URL}/comic")).map_err(|error| {
            AppError::Unexpected(format!("invalid manhuagui base URL: {error}"))
        })?;
        url.path_segments_mut()
            .map_err(|_| AppError::Unexpected("manhuagui comic URL cannot be a base".into()))?
            .push(&id)
            .push("");
        Ok(RequestSpec::html(url))
    }

    fn parse_detail(&self, id: &str, body: &[u8]) -> Result<ComicDetail> {
        let html = Html::parse_document(&String::from_utf8_lossy(body));
        let root = html.root_element();
        let Some(title) = text_of(root, ".book-title h1") else {
            let page_text = collapse_whitespace(&root.text().collect::<String>());
            return Err(if page_text.contains("页面不存在") {
                AppError::NotFound {
                    resource: format!("manhuagui comic `{id}`"),
                }
            } else {
                AppError::Unexpected(format!("failed to parse manhuagui comic `{id}`"))
            });
        };

        let thumb = attr_of(root, ".hcover img", "src")
            .or_else(|| attr_of(root, "meta[property=\"og:image\"]", "content"))
            .map(|raw| absolute_url(BASE_URL, &raw))
            .unwrap_or_default();

        let status_element = root.select(&selector("li.status")).next();
        let (status, mut latest_chapter, updated_at) = parse_status(status_element);
        latest_chapter = latest_chapter.or_else(|| {
            text_of(root, ".hcover .text").map(|text| {
                text.trim_start_matches("更新至：")
                    .trim_start_matches("更新至:")
                    .to_string()
            })
        });

        let extra = ManhuaguiExtra {
            original_title: text_of(root, ".book-title h2"),
            authors: labeled_links(root, "漫画作者："),
            status,
            latest_chapter,
            updated_at,
            year: labeled_links(root, "出品年代：").into_iter().next(),
            region: labeled_links(root, "漫画地区：").into_iter().next(),
            categories: labeled_links(root, "漫画剧情："),
            aliases: labeled_links(root, "漫画别名："),
            description: text_of(root, "#intro-cut")
                .or_else(|| text_of(root, "#intro-all"))
                .map(|text| clean_description(&text)),
            score: None,
            rating_count: None,
        };

        Ok(ComicDetail {
            title,
            thumb,
            extra: serde_json::to_value(extra)?,
        })
    }
}

fn parse_search_item(item: ElementRef<'_>) -> Option<SearchItem> {
    let href = attr_of(item, "a.bcover", "href")?;
    let id = numeric_id(
        href.trim_start_matches("/comic/").trim_end_matches('/'),
        "manhuagui",
    )
    .ok()?;
    let title = attr_of(item, "a.bcover", "title").or_else(|| text_of(item, "dt a"))?;
    let thumb = attr_of(item, "img", "src")
        .map(|raw| absolute_url(BASE_URL, &raw))
        .unwrap_or_default();

    let status_element = item.select(&selector("dd.tags.status")).next();
    let (status, latest_chapter, updated_at) = parse_status(status_element);

    let extra = ManhuaguiExtra {
        original_title: None,
        authors: labeled_links(item, "作者："),
        status,
        latest_chapter,
        updated_at,
        year: labeled_links(item, "年份：").into_iter().next(),
        region: labeled_links(item, "地区：").into_iter().next(),
        categories: labeled_links(item, "类型："),
        aliases: labeled_links(item, "别名："),
        description: text_of(item, "dd.intro span").map(|text| clean_description(&text)),
        score: text_of(item, ".book-score .score-avg strong").and_then(|text| text.parse().ok()),
        rating_count: text_of(item, ".book-score .score-num strong")
            .and_then(|text| text.replace(',', "").parse().ok()),
    };

    Some(SearchItem::new(
        id,
        title,
        thumb,
        SearchExtra::Manhuagui(extra),
    ))
}

fn parse_status(
    element: Option<ElementRef<'_>>,
) -> (Option<String>, Option<String>, Option<String>) {
    let Some(element) = element else {
        return (None, None, None);
    };
    let text = collapse_whitespace(&element.text().collect::<String>());
    let status = STATUS_RE
        .captures(&text)
        .map(|captures| captures[1].to_string());
    let updated_at = DATE_RE.find(&text).map(|found| found.as_str().to_string());
    let latest_chapter = element
        .select(&selector("a.blue"))
        .next()
        .map(|link| collapse_whitespace(&link.text().collect::<String>()))
        .filter(|text| !text.is_empty());

    (status, latest_chapter, updated_at)
}

fn clean_description(raw: &str) -> String {
    raw.replace("[详情]", "")
        .replace("[展開詳情]", "")
        .trim_start_matches("简介：")
        .trim_start_matches("簡介：")
        .trim()
        .to_string()
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

fn labeled_links(scope: ElementRef<'_>, label: &str) -> Vec<String> {
    for strong in scope.select(&selector("strong")) {
        if collapse_whitespace(&strong.text().collect::<String>()) != label {
            continue;
        }

        let links = match strong.parent().and_then(ElementRef::wrap) {
            Some(parent) => parent.select(&selector("a")).collect::<Vec<_>>(),
            None => strong.select(&selector("a")).collect::<Vec<_>>(),
        };

        return links
            .into_iter()
            .map(|link| collapse_whitespace(&link.text().collect::<String>()))
            .filter(|text| !text.is_empty())
            .collect();
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::model::SearchPlatform;

    const SEARCH_HTML: &str = r#"
<div class="result-count">搜索“<strong>火影</strong>”共查找到 <strong>2</strong> 部漫画</div>
<div class="book-result"><ul>
<li>
  <div class="book-cover"><a class="bcover" href="/comic/1/" title="示例漫画"><img src="//cf.mhgui.com/cpic/b/1.jpg"></a></div>
  <div class="book-detail"><dl>
    <dt><a>示例漫画</a></dt>
    <dd class="tags status"><span><strong>状态：</strong><span class="red">连载中</span>。最近于 [ <span class="red">2026-01-02 03:04</span> ] 更新至 [ <a class="blue">第12话</a> ]。</span></dd>
    <dd class="tags"><span><strong>年份：</strong><a>2020年</a></span><span><strong>地区：</strong><a>日本漫画</a></span><span><strong>类型：</strong><a>冒险</a><a>奇幻</a></span></dd>
    <dd class="tags"><span><strong>作者：</strong><a>作者A</a></span></dd>
    <dd class="tags"><span><strong>别名：</strong><a>别名A</a></span></dd>
    <dd class="intro"><span><strong>简介：</strong>一个示例故事。[<a>详情</a>]</span></dd>
  </dl></div>
  <div class="book-score"><p class="score-avg">评分 <strong>8.5</strong></p><p class="score-num">总计 <strong>123</strong> 人评分</p></div>
</li>
<li>
  <div class="book-cover"><a class="bcover" href="/comic/2/" title="第二个"><img src="//cf.mhgui.com/cpic/b/2.jpg"></a></div>
  <div class="book-detail"><dl>
    <dt><a>第二个</a></dt>
    <dd class="tags status"><span><strong>状态：</strong><span class="red">已完结</span>。</span></dd>
  </dl></div>
</li>
</ul></div>
"#;

    const DETAIL_HTML: &str = r#"
<div class="book-title"><h1>示例漫画</h1><h2>Original Title</h2></div>
<p class="hcover"><img src="//cf.mhgui.com/cpic/h/1.jpg"><span class="text">更新至：第12话</span></p>
<ul class="detail-list">
  <li><span><strong>出品年代：</strong><a>2020年</a></span><span><strong>漫画地区：</strong><a>日本漫画</a></span></li>
  <li><span><strong>漫画剧情：</strong><a>冒险</a><a>奇幻</a></span><span><strong>漫画作者：</strong><a>作者A</a></span></li>
  <li><span><strong>漫画别名：</strong>暂无</span></li>
  <li class="status"><span><strong>漫画状态：</strong><span class="dgreen">已完结</span>。最近于 [<span class="red">2026-01-02</span>] 更新至 [ <a class="blue">第12话</a> ]。</span></li>
</ul>
<div class="book-intro"><div id="intro-cut">简介文本</div></div>
"#;

    #[test]
    fn builds_search_and_detail_urls() {
        let search = Manhuagui.search_request("火影", 2).unwrap();
        assert_eq!(
            search.url.as_str(),
            "https://www.manhuagui.com/s/%E7%81%AB%E5%BD%B1_p2.html"
        );

        let detail = Manhuagui.detail_request("28004").unwrap();
        assert_eq!(
            detail.url.as_str(),
            "https://www.manhuagui.com/comic/28004/"
        );
        assert!(Manhuagui.detail_request("28/004").is_err());
    }

    #[test]
    fn parses_search_results() {
        let response = Manhuagui
            .parse_search("火影", 1, SEARCH_HTML.as_bytes())
            .unwrap();
        assert_eq!(response.total, 2);
        assert_eq!(response.page_size, 10);
        assert_eq!(response.result.len(), 2);

        let first = &response.result[0];
        assert_eq!(first.platform, SearchPlatform::Manhuagui);
        assert_eq!(first.id, "1");
        assert_eq!(first.title, "示例漫画");
        assert_eq!(first.thumb, "https://cf.mhgui.com/cpic/b/1.jpg");

        let SearchExtra::Manhuagui(extra) = &first.extra else {
            panic!("expected manhuagui search extra");
        };
        assert_eq!(extra.authors, ["作者A"]);
        assert_eq!(extra.status.as_deref(), Some("连载中"));
        assert_eq!(extra.latest_chapter.as_deref(), Some("第12话"));
        assert_eq!(extra.updated_at.as_deref(), Some("2026-01-02 03:04"));
        assert_eq!(extra.categories, ["冒险", "奇幻"]);
        assert_eq!(extra.description.as_deref(), Some("一个示例故事。"));
        assert_eq!(extra.score, Some(8.5));
        assert_eq!(extra.rating_count, Some(123));
    }

    #[test]
    fn parses_detail() {
        let detail = Manhuagui.parse_detail("1", DETAIL_HTML.as_bytes()).unwrap();
        assert_eq!(detail.title, "示例漫画");
        assert_eq!(detail.thumb, "https://cf.mhgui.com/cpic/h/1.jpg");
        assert_eq!(detail.extra["originalTitle"], "Original Title");
        assert_eq!(detail.extra["authors"], serde_json::json!(["作者A"]));
        assert_eq!(detail.extra["status"], "已完结");
        assert_eq!(detail.extra["latestChapter"], "第12话");
        assert_eq!(detail.extra["updatedAt"], "2026-01-02");
        assert_eq!(
            detail.extra["categories"],
            serde_json::json!(["冒险", "奇幻"])
        );
        assert_eq!(detail.extra["description"], "简介文本");
    }

    #[test]
    fn cache_key_ignores_signed_query() {
        let signed = Url::parse("https://i.hamreus.com/ps1/1.jpg?e=1700000000&m=abc").unwrap();
        assert_eq!(
            Manhuagui.cache_key_url(&signed),
            "https://i.hamreus.com/ps1/1.jpg"
        );

        let plain = Url::parse("https://cf.mhgui.com/cpic/h/1.jpg").unwrap();
        assert_eq!(Manhuagui.cache_key_url(&plain), plain.as_str());
    }
}
