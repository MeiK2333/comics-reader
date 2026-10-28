//! 漫蛙 (`manwa.me`) search, detail and reader pages.
//!
//! The site is server-rendered PHP behind Cloudflare: search, detail and the
//! reader are plain HTML documents, so this platform is a parser like
//! manhuagui. Two quirks shape the implementation:
//!
//! - A missing book or chapter answers `302` to the home page instead of
//!   `404`, so "not found" is detected by the absence of the expected
//!   structure, not by the status code.
//! - The detail and reader pages embed the complete chapter list. Reading a
//!   chapter's neighbours off the page itself keeps previous/next correct even
//!   for works with hundreds of chapters.

use std::collections::HashMap;
use std::sync::LazyLock;

use aes::cipher::{BlockDecrypt, KeyInit};
use regex::Regex;
use reqwest::RequestBuilder;
use reqwest::header::{ACCEPT_LANGUAGE, REFERER};
use scraper::{ElementRef, Html, Node, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{AppError, Result};
use crate::platforms::model::{
    ChapterRef, ComicDetail, DetailChapter, ReaderChapter, ReaderPage, SearchItem, SearchResponse,
};

use super::{
    Catalog, ImageProfile, RequestSpec, absolute_url, collapse_whitespace, host_matches, numeric_id,
};

const BASE_URL: &str = "https://manwa.me";
/// Every image host serves the same paths. Logging in is not required, so the
/// page always points at this host; the other mirrors are retry targets.
const IMAGE_HOST: &str = "mwappimgs.cc";
/// The reader page also lists `mwfilmsvfast40.cc`, which does not serve these
/// images, so the retry targets stay on the two verified mirrors.
const IMAGE_HOSTS: &[&str] = &[IMAGE_HOST, "newmwtuyuan25.cc", "newmwtuyuan21.cc"];
const HOST_SUFFIXES: &[&str] = &[
    "manwa.me",
    IMAGE_HOST,
    "newmwtuyuan25.cc",
    "newmwtuyuan21.cc",
];
const REFERER_VALUE: &str = "https://manwa.me/";
const ACCEPT_LANGUAGE_VALUE: &str = "zh-CN,zh;q=0.9,en;q=0.8";
/// The search page renders at most 35 cards per page but never prints a
/// total. The frontend uses `has_next` instead of the cumulative total.
const PAGE_SIZE: u32 = 35;
/// Every page image is stored AES-128-CBC encrypted. The reader script
/// (`/static/js/ch.js`) decrypts it with this key used as its own IV before
/// handing the WebP to `webp-hero`; covers and avatars are stored in the clear.
const IMAGE_CIPHER_KEY: &[u8; 16] = b"my2ecret782ecret";
/// Bump when the page-image cipher changes so cached plaintext is dropped.
const IMAGE_DECODE_VERSION: &str = "v1";
/// Lazy-loading placeholders that must never be treated as real images.
const PLACEHOLDER_IMAGES: &[&str] = &[
    "imagecover_s.png",
    "imagecover3.jpg",
    "imagecover.jpg",
    "cover.jpeg",
];

static DATE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d{4}-\d{2}-\d{2}").expect("valid date regex"));

/// Fields shared by search and detail. Search cards only expose the author,
/// status and description; the detail page fills the rest.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManwaExtra {
    authors: Vec<String>,
    status: Option<String>,
    latest_chapter: Option<String>,
    updated_at: Option<String>,
    region: Option<String>,
    categories: Vec<String>,
    tags: Vec<String>,
    aliases: Vec<String>,
    subscribers: Option<String>,
    description: Option<String>,
}

impl ManwaExtra {
    fn search(authors: Vec<String>, status: Option<String>, description: Option<String>) -> Self {
        Self {
            authors,
            status,
            latest_chapter: None,
            updated_at: None,
            region: None,
            categories: Vec::new(),
            tags: Vec::new(),
            aliases: Vec::new(),
            subscribers: None,
            description,
        }
    }
}

pub(super) struct Manwa;

impl Manwa {
    pub(super) const ID: &'static str = "manwa";
}

impl ImageProfile for Manwa {
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
        let mut key = url.clone();
        if is_image_host(&key) {
            let _ = key.set_host(Some(IMAGE_HOST));
        }
        // The `?v=` query stays: covers use it for invalidation, while page
        // images carry a content hash in the file name and a fixed value.
        key.to_string()
    }

    fn image_fallbacks(&self, url: &Url) -> Vec<Url> {
        let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
            return Vec::new();
        };
        if !IMAGE_HOSTS.contains(&host.as_str()) {
            return Vec::new();
        }

        IMAGE_HOSTS
            .iter()
            .filter(|candidate| **candidate != host.as_str())
            .filter_map(|candidate| {
                let mut alternate = url.clone();
                alternate.set_host(Some(candidate)).ok()?;
                Some(alternate)
            })
            .collect()
    }

    fn decode_key(&self, _url: &Url) -> Option<String> {
        Some(format!("?decoded={IMAGE_DECODE_VERSION}"))
    }

    fn decode_image(&self, _url: &Url, bytes: &[u8]) -> Option<Vec<u8>> {
        decrypt_page_image(bytes)
    }
}

impl Catalog for Manwa {
    fn search_request(&self, query: &str, page: u32) -> Result<RequestSpec> {
        let mut url = Url::parse(&format!("{BASE_URL}/search"))
            .map_err(|error| AppError::Unexpected(format!("invalid manwa base URL: {error}")))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("keyword", query);
            if page > 1 {
                pairs.append_pair("page", &page.to_string());
            }
        }
        Ok(RequestSpec::html(url))
    }

    fn parse_search(&self, _query: &str, page: u32, body: &[u8]) -> Result<SearchResponse> {
        let text = String::from_utf8_lossy(body);
        reject_blocked_page(&text)?;
        if !text.contains("book-list") && !text.contains("搜寻结果") && !text.contains("searchtxt")
        {
            return Err(AppError::Unexpected(
                "manwa search page was not recognised".into(),
            ));
        }

        let html = Html::parse_document(&text);
        let result: Vec<SearchItem> = html
            .select(&selector("ul.book-list > li"))
            .filter_map(parse_search_item)
            .collect();
        // There is no result count on the page, so the total is "seen so far"
        // and `has_next` drives the pager (the same contract as E-Hentai).
        let total = (page.saturating_sub(1) as u64) * PAGE_SIZE as u64 + result.len() as u64;

        Ok(SearchResponse {
            total,
            page_size: PAGE_SIZE,
            result,
            has_next: Some(has_next_page(&html)),
        })
    }

    fn detail_request(&self, id: &str) -> Result<RequestSpec> {
        let id = numeric_id(id, Self::ID)?;
        let url = Url::parse(&format!("{BASE_URL}/book/{id}"))
            .map_err(|error| AppError::Unexpected(format!("invalid manwa detail URL: {error}")))?;
        Ok(RequestSpec::html(url))
    }

    fn parse_detail(&self, id: &str, body: &[u8]) -> Result<ComicDetail> {
        let text = String::from_utf8_lossy(body);
        reject_blocked_page(&text)?;

        let html = Html::parse_document(&text);
        let root = html.root_element();
        let Some(title) = text_of(root, "h1.detail-main-info-title") else {
            // The site redirects unknown ids to the home page, so a body
            // without the detail heading is the only reliable "not found".
            return Err(AppError::NotFound {
                resource: format!("manwa comic `{id}`"),
            });
        };

        let chapters = parse_chapters(root);

        let extra = ManwaExtra {
            authors: labeled_links(root, "作者"),
            status: labeled_value(root, "更新状态"),
            latest_chapter: labeled_value(root, "最新章节"),
            updated_at: text_of(root, ".detail-list-title-3")
                .and_then(|text| DATE_RE.find(&text).map(|found| found.as_str().to_string())),
            region: labeled_value(root, "地区"),
            categories: labeled_value(root, "类别")
                .map(|value| split_list(&value))
                .unwrap_or_default(),
            tags: info_tags(root),
            aliases: labeled_value(root, "别名")
                .map(|value| split_list(&value))
                .unwrap_or_default(),
            subscribers: labeled_value(root, "订阅数"),
            description: description_of(root),
        };

        let detail_chapters = chapters
            .into_iter()
            .map(|chapter| DetailChapter {
                id: Some(chapter.id),
                title: Some(chapter.title),
            })
            .collect();

        ComicDetail::new(
            Self::ID,
            title,
            image_url(root, ".detail-main-cover img").unwrap_or_default(),
            detail_chapters,
            extra,
        )
    }

    fn reader_chapter_request(&self, _id: &str, chapter_id: Option<&str>) -> Result<RequestSpec> {
        let Some(chapter_id) = chapter_id else {
            return Err(AppError::InvalidInput {
                field: "chapterId",
                reason: "manwa pages are addressed by chapter id".into(),
            });
        };
        let chapter_id = numeric_id(chapter_id, Self::ID)?;
        let url = Url::parse(&format!("{BASE_URL}/chapter/{chapter_id}"))
            .map_err(|error| AppError::Unexpected(format!("invalid manwa chapter URL: {error}")))?;
        Ok(RequestSpec::html(url))
    }

    fn parse_reader_chapter(
        &self,
        _id: &str,
        chapter_id: Option<&str>,
        body: &[u8],
    ) -> Result<ReaderChapter> {
        let text = String::from_utf8_lossy(body);
        reject_blocked_page(&text)?;

        let html = Html::parse_document(&text);
        let root = html.root_element();
        let Some(title) = text_of(root, ".view-fix-top-bar-center-right-book-name") else {
            return Err(AppError::NotFound {
                resource: format!("manwa chapter `{}`", chapter_id.unwrap_or_default()),
            });
        };

        let pages = parse_pages(root);
        if pages.is_empty() {
            // Chapters that are still being re-uploaded ("排队补图中") render no
            // image element at all.
            return Err(AppError::NotFound {
                resource: format!("manwa chapter `{}`", chapter_id.unwrap_or_default()),
            });
        }

        let titles: HashMap<String, String> = parse_chapters(root)
            .into_iter()
            .map(|chapter| (chapter.id, chapter.title))
            .collect();

        Ok(ReaderChapter {
            title,
            thumb: String::new(),
            chapter_id: chapter_id.and_then(|id| numeric_id(id, Self::ID).ok()),
            chapter_title: text_of(root, ".view-fix-top-bar-center-right-chapter-name"),
            pages,
            previous: nav_ref(root, "prev", &titles),
            next: nav_ref(root, "next", &titles),
        })
    }
}

/// Whether a page link ("下一页") exists. A last page renders the window
/// without it.
fn has_next_page(html: &Html) -> bool {
    html.select(&selector(".pagination2 a"))
        .any(|link| collapse_whitespace(&link.text().collect::<String>()) == "下一页")
}

/// Some networks (notably mainland China, including direct ECH connections)
/// get a JavaScript domain-rotation page instead of content. It is an HTTP
/// 200, so without this guard a search would silently parse to zero results.
fn reject_blocked_page(body: &str) -> Result<()> {
    if body.contains("网址跳转") && body.contains("JSON.parse(atob(") {
        return Err(AppError::Network {
            url: BASE_URL.to_string(),
            reason: "manwa redirected this network to a backup domain; enable the proxy (ECH cannot follow the rotation)".into(),
        });
    }
    if body.contains("Just a moment")
        || body.contains("challenge-platform")
        || body.contains("cf-chl-")
    {
        return Err(AppError::Network {
            url: BASE_URL.to_string(),
            reason: "manwa answered with a bot challenge; retry or enable the proxy".into(),
        });
    }
    Ok(())
}

fn parse_search_item(item: ElementRef<'_>) -> Option<SearchItem> {
    let link = item.select(&selector("a[href^=\"/book/\"]")).next()?;
    let id = book_id_from_href(link.attr("href")?)?;
    let title = text_of(item, "p.book-list-info-title")
        .or_else(|| link.attr("title").map(collapse_whitespace))
        .filter(|title| !title.is_empty())?;
    let thumb = image_url(item, "img.book-list-cover-img").unwrap_or_default();
    let status = text_of(item, "span.book-list-info-bottom-right-font");
    let author = text_of(item, "span.book-list-info-bottom-item")
        .map(|text| strip_author_prefix(&text))
        .filter(|text| !text.is_empty())
        .into_iter()
        .collect();
    let description = text_of(item, "p.book-list-info-desc").filter(|text| text != "...");

    SearchItem::new(
        Manwa::ID,
        id,
        title,
        thumb,
        ManwaExtra::search(author, status, description),
    )
    .ok()
}

/// Chapters are listed in reading order on both the detail and reader pages.
fn parse_chapters(root: ElementRef<'_>) -> Vec<ChapterRef> {
    let mut seen = std::collections::HashSet::new();
    let mut chapters = Vec::new();

    for link in root.select(&selector("#detail-list-select a.chapteritem")) {
        let Some(id) = link.attr("href").and_then(chapter_id_from_href) else {
            continue;
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        let title = link
            .attr("title")
            .map(collapse_whitespace)
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| collapse_whitespace(&link.text().collect::<String>()));
        if title.is_empty() {
            continue;
        }
        chapters.push(ChapterRef { id, title });
    }

    chapters
}

/// Page images keep their DOM order in `data-sort`; sorting defends against a
/// future shuffle without trusting the document order.
fn parse_pages(root: ElementRef<'_>) -> Vec<ReaderPage> {
    let mut ordered: Vec<(u32, usize, ReaderPage)> = root
        .select(&selector("#cp_img img.content-img"))
        .enumerate()
        .filter_map(|(index, image)| {
            let raw = image_source(image)?;
            let sort = image
                .attr("data-sort")
                .and_then(|value| value.trim().parse::<u32>().ok())
                .unwrap_or(u32::MAX);
            Some((
                sort,
                index,
                ReaderPage {
                    url: absolute_url(BASE_URL, &raw),
                    width: None,
                    height: None,
                    local_path: None,
                },
            ))
        })
        .collect();

    ordered.sort_by_key(|(sort, index, _)| (*sort, *index));
    ordered.into_iter().map(|(_, _, page)| page).collect()
}

fn nav_ref(
    root: ElementRef<'_>,
    direction: &str,
    titles: &HashMap<String, String>,
) -> Option<ChapterRef> {
    let href = attr_of(
        root,
        &format!("a.view-fix-bottom-bar-item-menu-{direction}"),
        "href",
    )?;
    let id = chapter_id_from_href(&href)?;
    let title = titles.get(&id).cloned().unwrap_or_default();
    Some(ChapterRef { id, title })
}

fn image_source(image: ElementRef<'_>) -> Option<String> {
    ["data-r-src", "data-original", "src"]
        .iter()
        .filter_map(|attribute| image.attr(attribute))
        .map(str::trim)
        .find(|value| !value.is_empty() && !is_placeholder(value))
        .map(str::to_string)
}

fn is_placeholder(raw: &str) -> bool {
    PLACEHOLDER_IMAGES.iter().any(|name| raw.contains(name))
}

fn is_image_host(url: &Url) -> bool {
    url.host_str()
        .map(str::to_ascii_lowercase)
        .is_some_and(|host| IMAGE_HOSTS.contains(&host.as_str()))
}

fn book_id_from_href(href: &str) -> Option<String> {
    let rest = href.split("/book/").nth(1)?;
    numeric_id(rest.split(['/', '?', '#']).next()?, Manwa::ID).ok()
}

fn chapter_id_from_href(href: &str) -> Option<String> {
    let rest = href.split("/chapter/").nth(1)?;
    numeric_id(rest.split(['/', '?', '#']).next()?, Manwa::ID).ok()
}

fn strip_author_prefix(raw: &str) -> String {
    let text = raw
        .strip_prefix("作者：")
        .or_else(|| raw.strip_prefix("作者:"))
        .unwrap_or(raw);
    text.trim()
        .trim_end_matches("...")
        .trim_end_matches('…')
        .trim()
        .to_string()
}

/// The detail metadata is a list of label/value rows; labels vary per work
/// (`别名`, `订阅数`, …), so every field is looked up by its label text.
fn info_row<'a>(root: ElementRef<'a>, label: &str) -> Option<ElementRef<'a>> {
    root.select(&selector("p.detail-main-info-author"))
        .find(|row| {
            row.select(&selector(".detail-main-info-author-field"))
                .next()
                .map(|field| field_label(&field.text().collect::<String>()))
                .is_some_and(|field| field == label)
        })
}

fn field_label(raw: &str) -> String {
    raw.trim().trim_end_matches(['：', ':']).trim().to_string()
}

fn labeled_value(root: ElementRef<'_>, label: &str) -> Option<String> {
    let row = info_row(root, label)?;
    let value = row.select(&selector(".detail-main-info-value")).next()?;
    let text = collapse_whitespace(&value.text().collect::<String>());
    (!text.is_empty()).then_some(text)
}

fn labeled_links(root: ElementRef<'_>, label: &str) -> Vec<String> {
    let Some(row) = info_row(root, label) else {
        return Vec::new();
    };
    let Some(value) = row.select(&selector(".detail-main-info-value")).next() else {
        return Vec::new();
    };

    let links: Vec<String> = value
        .select(&selector("a"))
        .map(|link| collapse_whitespace(&link.text().collect::<String>()))
        .filter(|text| !text.is_empty())
        .collect();
    if links.is_empty() {
        let text = collapse_whitespace(&value.text().collect::<String>());
        return if text.is_empty() {
            Vec::new()
        } else {
            vec![text]
        };
    }
    links
}

fn info_tags(root: ElementRef<'_>) -> Vec<String> {
    root.select(&selector("a.info-tag"))
        .map(|tag| collapse_whitespace(&tag.text().collect::<String>()))
        .filter(|tag| !tag.is_empty())
        .collect()
}

/// The synopsis is HTML with `<br>` line breaks; collect the text nodes and
/// turn the breaks into newlines.
fn description_of(root: ElementRef<'_>) -> Option<String> {
    let element = root.select(&selector("p.detail-desc")).next()?;
    let mut raw = String::new();
    for node in element.descendants() {
        match node.value() {
            Node::Text(text) => raw.push_str(&text.text),
            Node::Element(inner) if inner.name() == "br" => raw.push('\n'),
            _ => {}
        }
    }

    let text = raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (!text.is_empty()).then_some(text)
}

/// `别名：a、b` and `类别：BL` use ideographic commas without spaces.
fn split_list(raw: &str) -> Vec<String> {
    raw.split(['、', '，', ','])
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

/// AES-128-CBC decryption with PKCS#7 padding, mirroring CryptoJS
/// `AES.decrypt(ciphertext, Utf8.parse(key), { iv: Utf8.parse(key) })`.
// The project supports Rust 1.85, where `usize::is_multiple_of` does not exist.
#[allow(clippy::manual_is_multiple_of)]
fn decrypt_page_image(ciphertext: &[u8]) -> Option<Vec<u8>> {
    if ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
        return None;
    }

    let cipher = aes::Aes128::new_from_slice(IMAGE_CIPHER_KEY).ok()?;
    let mut previous = *IMAGE_CIPHER_KEY;
    let mut plain = Vec::with_capacity(ciphertext.len());

    for chunk in ciphertext.chunks(16) {
        let mut block = aes::cipher::Block::<aes::Aes128>::clone_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        for (byte, value) in block.iter_mut().zip(previous.iter()) {
            *byte ^= *value;
        }
        plain.extend_from_slice(&block);
        previous.copy_from_slice(chunk);
    }

    unpad_pkcs7(plain)
}

fn unpad_pkcs7(mut data: Vec<u8>) -> Option<Vec<u8>> {
    let padding = *data.last()? as usize;
    if padding == 0 || padding > 16 || padding > data.len() {
        return None;
    }
    if !data[data.len() - padding..]
        .iter()
        .all(|byte| *byte as usize == padding)
    {
        return None;
    }
    data.truncate(data.len() - padding);
    Some(data)
}

fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("static CSS selector")
}

fn text_of(scope: ElementRef<'_>, css: &str) -> Option<String> {
    let text = scope
        .select(&selector(css))
        .next()?
        .text()
        .collect::<String>();
    let text = collapse_whitespace(&text);
    (!text.is_empty()).then_some(text)
}

fn attr_of(scope: ElementRef<'_>, css: &str, attribute: &str) -> Option<String> {
    scope
        .select(&selector(css))
        .next()?
        .attr(attribute)
        .map(str::to_string)
}

fn image_url(scope: ElementRef<'_>, css: &str) -> Option<String> {
    image_source(scope.select(&selector(css)).next()?).map(|raw| absolute_url(BASE_URL, &raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEARCH_HTML: &str = include_str!("manwa/fixtures/search.html");
    const BOOK_HTML: &str = include_str!("manwa/fixtures/book.html");
    const CHAPTER_HTML: &str = include_str!("manwa/fixtures/chapter.html");
    const ROTATION_HTML: &str = include_str!("manwa/fixtures/rotation.html");

    fn extra_of(value: &serde_json::Value) -> ManwaExtra {
        serde_json::from_value(value.clone()).expect("extra should deserialize")
    }

    #[test]
    fn builds_search_detail_and_reader_urls() {
        assert_eq!(
            Manwa.search_request("海贼", 1).unwrap().url.as_str(),
            "https://manwa.me/search?keyword=%E6%B5%B7%E8%B4%BC"
        );
        assert_eq!(
            Manwa.search_request("海贼", 2).unwrap().url.as_str(),
            "https://manwa.me/search?keyword=%E6%B5%B7%E8%B4%BC&page=2"
        );
        assert_eq!(
            Manwa.detail_request("17648").unwrap().url.as_str(),
            "https://manwa.me/book/17648"
        );
        assert!(Manwa.detail_request("../17648").is_err());
        assert_eq!(
            Manwa
                .reader_chapter_request("17648", Some("30470944"))
                .unwrap()
                .url
                .as_str(),
            "https://manwa.me/chapter/30470944"
        );
        assert!(Manwa.reader_chapter_request("17648", None).is_err());
        assert!(
            Manwa
                .reader_chapter_request("17648", Some("30/../1"))
                .is_err()
        );
    }

    #[test]
    fn parses_search_cards() {
        let response = Manwa
            .parse_search("海贼", 1, SEARCH_HTML.as_bytes())
            .unwrap();

        assert_eq!(response.page_size, 35);
        assert_eq!(response.result.len(), 5);
        assert_eq!(response.total, 5);
        assert_eq!(response.has_next, Some(true));

        let first = &response.result[0];
        assert_eq!(first.id, "362094");
        assert_eq!(first.platform, "manwa");
        assert_eq!(first.title, "海贼巨乳5");
        assert_eq!(
            first.thumb,
            "https://mwappimgs.cc/static/upload2/book/content/20250330/c_228a8dd3d2449a3189cd1fe64f5026824.png"
        );

        let extra = extra_of(&first.extra);
        assert_eq!(extra.authors, vec!["BRAVE HEART petit (KOJIROU!)"]);
        assert_eq!(extra.status.as_deref(), Some("连载中"));
        assert_eq!(extra.description.as_deref(), Some("海贼王本..."));

        // The site truncates long synopses to "..." on the card only.
        let empty_synopsis = extra_of(&response.result[1].extra);
        assert_eq!(empty_synopsis.description, None);
    }

    #[test]
    fn the_second_page_reports_a_cumulative_total() {
        let response = Manwa
            .parse_search("海贼", 2, SEARCH_HTML.as_bytes())
            .unwrap();
        assert_eq!(response.total, 35 + 5);
    }

    #[test]
    fn parsed_detail_exposes_the_metadata_rows() {
        let detail = Manwa.parse_detail("17648", BOOK_HTML.as_bytes()).unwrap();

        assert_eq!(detail.title, "水边之夜 (完整版)");
        assert_eq!(
            detail.thumb,
            "https://mwappimgs.cc/static/upload3/book/id/17648/cover_1.png?v=1781461214"
        );
        assert_eq!(detail.chapters.len(), 6);
        assert_eq!(detail.chapters[0].id.as_deref(), Some("497238"));
        assert_eq!(detail.chapters[0].title.as_deref(), Some("第01话"));

        let extra = extra_of(&detail.extra);
        assert_eq!(extra.authors, vec!["으자"]);
        assert_eq!(extra.status.as_deref(), Some("已完结"));
        assert_eq!(extra.latest_chapter.as_deref(), Some("海月小剧场"));
        assert_eq!(extra.updated_at.as_deref(), Some("2026-08-28"));
        assert_eq!(extra.region.as_deref(), Some("韩国"));
        assert_eq!(extra.categories, vec!["BL"]);
        assert_eq!(extra.subscribers.as_deref(), Some("3523.75K"));
        assert_eq!(extra.aliases.len(), 5);
        assert!(extra.tags.contains(&"韩漫".to_string()));

        let description = extra.description.unwrap();
        assert_eq!(description.lines().count(), 3);
        assert!(description.ends_with("要身体…"));
    }

    #[test]
    fn parsed_reader_keeps_page_order_and_neighbours() {
        let chapter = Manwa
            .parse_reader_chapter("17648", Some("30470944"), CHAPTER_HTML.as_bytes())
            .unwrap();

        assert_eq!(chapter.title, "水边之夜 (完整版)");
        assert_eq!(chapter.chapter_title.as_deref(), Some("第74话"));
        assert_eq!(chapter.chapter_id.as_deref(), Some("30470944"));
        assert!(chapter.thumb.is_empty());
        assert_eq!(chapter.pages.len(), 6);
        assert!(
            chapter.pages[0]
                .url
                .starts_with("https://mwappimgs.cc/static/upload2/book/id/17648/30470944/")
        );

        let previous = chapter.previous.expect("chapter 74 has a previous");
        assert_eq!(previous.id, "30464620");
        assert_eq!(previous.title, "小可爱生日快乐");
        let next = chapter.next.expect("chapter 74 has a next");
        assert_eq!(next.id, "30481268");
        assert_eq!(next.title, "第75话");
    }

    #[test]
    fn redirected_pages_are_not_found() {
        let home = b"<html><body><a href=\"/book/1\">home</a></body></html>";
        assert!(matches!(
            Manwa.parse_detail("1", home),
            Err(AppError::NotFound { .. })
        ));
        assert!(matches!(
            Manwa.parse_reader_chapter("1", Some("2"), home),
            Err(AppError::NotFound { .. })
        ));
    }

    #[test]
    fn a_domain_rotation_page_is_a_network_error() {
        let errors = [
            Manwa
                .parse_search("海贼", 1, ROTATION_HTML.as_bytes())
                .err(),
            Manwa.parse_detail("17648", ROTATION_HTML.as_bytes()).err(),
            Manwa
                .parse_reader_chapter("17648", Some("30470944"), ROTATION_HTML.as_bytes())
                .err(),
        ];

        for error in errors {
            match error {
                Some(AppError::Network { reason, .. }) => {
                    assert!(reason.contains("backup domain"), "{reason}");
                }
                other => panic!("expected a network error, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_chapter_without_pages_is_not_found() {
        let html = "<html><body><div class=\"view-fix-top-bar-center-right-book-name\">Book</div></body></html>";
        assert!(matches!(
            Manwa.parse_reader_chapter("1", Some("2"), html.as_bytes()),
            Err(AppError::NotFound { .. })
        ));
    }

    #[test]
    fn the_last_search_page_has_no_next_link() {
        let last = r#"<html><body><ul class="pagination2"><li class="active"><a>20</a></li><li><a data-page="19">上一页</a></li></ul></body></html>"#;
        assert!(!has_next_page(&Html::parse_document(last)));

        let middle = r#"<html><body><ul class="pagination2"><li><a data-page="2">下一页</a></li></ul></body></html>"#;
        assert!(has_next_page(&Html::parse_document(middle)));
    }

    #[test]
    fn image_fallbacks_switch_between_hosts() {
        let url =
            Url::parse("https://mwappimgs.cc/static/upload3/book/id/1/2/page_zb.webp?v=20220724")
                .unwrap();
        let fallbacks = Manwa.image_fallbacks(&url);

        assert_eq!(fallbacks.len(), 2);
        assert_eq!(fallbacks[0].host_str(), Some("newmwtuyuan25.cc"));
        assert_eq!(fallbacks[0].path(), url.path());
        assert_eq!(fallbacks[0].query(), url.query());
        assert!(
            Manwa
                .image_fallbacks(&Url::parse("https://example.com/1.png").unwrap())
                .is_empty()
        );
    }

    #[test]
    fn cache_key_normalizes_the_image_host() {
        let primary =
            Url::parse("https://mwappimgs.cc/static/upload3/book/id/1/2/page_zb.webp?v=20220724")
                .unwrap();
        let mirror = Url::parse(
            "https://newmwtuyuan21.cc/static/upload3/book/id/1/2/page_zb.webp?v=20220724",
        )
        .unwrap();

        assert_eq!(Manwa.cache_key_url(&primary), Manwa.cache_key_url(&mirror));
        assert_eq!(
            Manwa.cache_key_url(&mirror),
            "https://mwappimgs.cc/static/upload3/book/id/1/2/page_zb.webp?v=20220724"
        );
    }

    fn encrypt_cbc(plain: &[u8]) -> Vec<u8> {
        use aes::cipher::BlockEncrypt;

        let cipher = aes::Aes128::new_from_slice(IMAGE_CIPHER_KEY).unwrap();
        let padding = 16 - plain.len() % 16;
        let mut padded = plain.to_vec();
        padded.extend(std::iter::repeat_n(padding as u8, padding));

        let mut previous = *IMAGE_CIPHER_KEY;
        let mut encrypted = Vec::with_capacity(padded.len());
        for chunk in padded.chunks(16) {
            let mut block = aes::cipher::Block::<aes::Aes128>::clone_from_slice(chunk);
            for (byte, value) in block.iter_mut().zip(previous.iter()) {
                *byte ^= *value;
            }
            cipher.encrypt_block(&mut block);
            encrypted.extend_from_slice(&block);
            previous.copy_from_slice(&block);
        }
        encrypted
    }

    #[test]
    fn decrypts_page_images_encrypted_at_rest() {
        let payload = b"RIFF\x24\x00\x00\x00WEBPVP8 \x18\x00\x00\x00 example payload";
        let encrypted = encrypt_cbc(payload);

        assert_ne!(encrypted.as_slice(), payload);
        assert_eq!(
            decrypt_page_image(&encrypted).as_deref(),
            Some(&payload[..])
        );
        assert!(decrypt_page_image(payload).is_none());
        assert!(decrypt_page_image(&encrypted[..15]).is_none());
        assert!(decrypt_page_image(&[]).is_none());
    }

    #[test]
    fn decoded_images_use_their_own_cache_namespace() {
        let url =
            Url::parse("https://mwappimgs.cc/static/upload3/book/id/1/2/page_zb.webp").unwrap();
        assert_eq!(Manwa.decode_key(&url).as_deref(), Some("?decoded=v1"));
    }

    fn contract_search_extra() -> ManwaExtra {
        ManwaExtra::search(
            vec!["BRAVE HEART petit (KOJIROU!)".into()],
            Some("连载中".into()),
            Some("海贼王本...".into()),
        )
    }

    fn contract_detail_extra() -> ManwaExtra {
        ManwaExtra {
            authors: vec!["으자".into()],
            status: Some("已完结".into()),
            latest_chapter: Some("海月小剧场".into()),
            updated_at: Some("2026-08-28".into()),
            region: Some("韩国".into()),
            categories: vec!["BL".into()],
            tags: vec!["韩漫".into()],
            aliases: vec!["海边之夜".into()],
            subscribers: Some("3523.75K".into()),
            description: Some("第一行\n第二行".into()),
        }
    }

    #[test]
    fn matches_the_platform_wire_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../src/platforms/manwa/fixtures/wire.json"
        ))
        .unwrap();

        let search = SearchItem::new(
            Manwa::ID,
            "362094",
            "海贼巨乳5",
            "https://mwappimgs.cc/static/upload2/book/content/20250330/c_228a8dd3d2449a3189cd1fe64f5026824.png",
            contract_search_extra(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(search).unwrap(), fixture["search"]);

        let detail = ComicDetail::new(
            Manwa::ID,
            "水边之夜 (完整版)",
            "https://mwappimgs.cc/static/upload3/book/id/17648/cover_1.png?v=1781461214",
            vec![DetailChapter {
                id: Some("497238".into()),
                title: Some("第01话".into()),
            }],
            contract_detail_extra(),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(detail).unwrap(), fixture["detail"]);
    }
}
