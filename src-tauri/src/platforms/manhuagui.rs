use std::sync::LazyLock;

use regex::Regex;
use reqwest::RequestBuilder;
use reqwest::header::{ACCEPT_LANGUAGE, REFERER};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{AppError, Result};
use crate::platforms::model::{
    ChapterRef, ComicDetail, DetailChapter, ReaderChapter, ReaderPage, SearchItem, SearchResponse,
};

use super::{
    Catalog, ImageProfile, RequestSpec, absolute_url, collapse_whitespace, host_matches, numeric_id,
};

const BASE_URL: &str = "https://www.manhuagui.com";
const IMAGE_HOST: &str = "https://i.hamreus.com";
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
static DECRYPTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^.*}\('(.*)',(\d+),(\d+),'([\w|+/=]*)'.*$")
        .expect("valid manhuagui decryption regex")
});
static WORD_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\w+\b").expect("valid word regex"));

/// Fields both search and detail return for manhuagui. Search results leave
/// `chapter_groups` empty; the detail page fills it from the chapter list.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManhuaguiExtra {
    original_title: Option<String>,
    authors: Vec<String>,
    status: Option<String>,
    latest_chapter: Option<String>,
    updated_at: Option<String>,
    year: Option<String>,
    region: Option<String>,
    categories: Vec<String>,
    aliases: Vec<String>,
    description: Option<String>,
    score: Option<f32>,
    rating_count: Option<u64>,
    chapter_groups: Vec<ManhuaguiChapterGroup>,
}

/// A site chapter group such as a volume or one-shot collection. `title` is
/// empty when the page did not label the group; the frontend localizes it.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManhuaguiChapterGroup {
    title: String,
    chapters: Vec<ManhuaguiChapterRef>,
}

/// One chapter of a manhuagui work, in reading order.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManhuaguiChapterRef {
    id: String,
    title: String,
}

pub(super) struct Manhuagui;

impl Manhuagui {
    pub(super) const ID: &'static str = "manhuagui";
}

impl ImageProfile for Manhuagui {
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
        let mut cache_url = url.clone();
        cache_url.set_query(None);
        cache_url.to_string()
    }

    fn image_fallbacks(&self, url: &Url) -> Vec<Url> {
        // The normalized path is valid on either image CDN; retry the other host
        // when the primary one is blocked.
        if url.host_str() != Some("i.hamreus.com") {
            return Vec::new();
        }
        let mut alternate = url.clone();
        if alternate.set_host(Some("cf.hamreus.com")).is_ok() {
            vec![alternate]
        } else {
            Vec::new()
        }
    }
}

impl Catalog for Manhuagui {
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
            has_next: None,
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
            chapter_groups: parse_chapter_groups(root, id),
        };

        let chapters = extra
            .chapter_groups
            .iter()
            .flat_map(|group| group.chapters.iter())
            .map(|chapter| DetailChapter {
                id: Some(chapter.id.clone()),
                title: Some(chapter.title.clone()),
            })
            .collect();

        ComicDetail::new(Manhuagui::ID, title, thumb, chapters, extra)
    }

    fn reader_chapter_request(&self, id: &str, chapter_id: Option<&str>) -> Result<RequestSpec> {
        let comic = numeric_id(id, "manhuagui")?;
        let chapter = chapter_id.ok_or_else(|| AppError::InvalidInput {
            field: "chapterId",
            reason: "a manhuagui chapter id is required".into(),
        })?;
        let chapter = numeric_id(chapter, "manhuagui")?;

        let mut url = Url::parse(&format!("{BASE_URL}/comic")).map_err(|error| {
            AppError::Unexpected(format!("invalid manhuagui base URL: {error}"))
        })?;
        url.path_segments_mut()
            .map_err(|_| AppError::Unexpected("manhuagui chapter URL cannot be a base".into()))?
            .push(&comic)
            .push(&format!("{chapter}.html"));
        Ok(RequestSpec::html(url))
    }

    fn parse_reader_chapter(
        &self,
        id: &str,
        chapter_id: Option<&str>,
        body: &[u8],
    ) -> Result<ReaderChapter> {
        let comic = numeric_id(id, "manhuagui")?;
        let chapter = chapter_id
            .map(|value| numeric_id(value, "manhuagui"))
            .transpose()?
            .ok_or_else(|| AppError::InvalidInput {
                field: "chapterId",
                reason: "a manhuagui chapter id is required".into(),
            })?;

        let text = String::from_utf8_lossy(body);
        let html = Html::parse_document(&text);
        let root = html.root_element();

        let manifest = parse_reader_manifest(&text);
        let pages = manifest.as_ref().map(manifest_pages).unwrap_or_default();
        if pages.is_empty() {
            return Err(AppError::NotFound {
                resource: format!("manhuagui chapter `{chapter}` pages"),
            });
        }

        let (title, chapter_title, thumb) = parse_reader_identity(root, &comic, manifest.as_ref());
        let (previous, next) = parse_reader_navigation(root, &comic, &chapter, manifest.as_ref());

        Ok(ReaderChapter {
            title,
            thumb,
            chapter_id: Some(chapter),
            chapter_title,
            pages,
            previous,
            next,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManhuaguiPageData {
    #[serde(default)]
    path: String,
    #[serde(default)]
    files: Vec<String>,
    #[serde(default)]
    bname: Option<String>,
    #[serde(default)]
    cname: Option<String>,
    #[serde(default)]
    bpic: Option<String>,
    #[serde(default)]
    next_id: i64,
    #[serde(default)]
    prev_id: i64,
}

fn parse_chapter_groups(root: ElementRef<'_>, comic_id: &str) -> Vec<ManhuaguiChapterGroup> {
    // R18/hidden works put the real chapter list in a compressed __VIEWSTATE
    // field; prefer it over the visible placeholder when it decodes.
    if let Some(encoded) = attr_of(root, "#__VIEWSTATE", "value")
        && let Some(decoded) = decode_lz_string(&encoded)
    {
        let fragment = Html::parse_fragment(&decoded);
        let groups = parse_chapter_groups_markup(fragment.root_element(), comic_id);
        if !groups.is_empty() {
            return groups;
        }
    }

    parse_chapter_groups_markup(root, comic_id)
}

fn parse_chapter_groups_markup(root: ElementRef<'_>, comic_id: &str) -> Vec<ManhuaguiChapterGroup> {
    let mut scope = root.select(&selector(".chapter")).next().unwrap_or(root);
    let mut sections: Vec<_> = scope.select(&selector(".chapter-list")).collect();
    if sections.is_empty() {
        sections = scope.select(&selector("[id^=chapter-list-]")).collect();
    }
    if sections.is_empty() {
        // Some layouts wrap only the heading in `.chapter`; retry at the root.
        scope = root;
        sections = root.select(&selector(".chapter-list")).collect();
        if sections.is_empty() {
            return Vec::new();
        }
    }

    let mut titles: Vec<String> = scope
        .select(&selector("h4"))
        .map(|heading| collapse_whitespace(&heading.text().collect::<String>()))
        .collect();
    // The page sometimes emits a volume heading without a matching list (or
    // vice versa); keep the DOM order and use an empty title for the gap.
    while titles.len() < sections.len() {
        titles.push(String::new());
    }
    titles.truncate(sections.len());

    let mut groups = Vec::new();
    for (title, section) in titles.into_iter().zip(sections) {
        let mut chapters = Vec::new();
        for page in section.select(&selector("ul")) {
            let mut links: Vec<_> = page.select(&selector("li a[href]")).collect();
            // manhuagui lists the newest chapter first inside each volume.
            links.reverse();
            for link in links {
                if let Some(chapter) = chapter_from_link(link, comic_id) {
                    chapters.push(chapter);
                }
            }
        }
        if !chapters.is_empty() {
            groups.push(ManhuaguiChapterGroup { title, chapters });
        }
    }

    groups
}

fn chapter_from_link(link: ElementRef<'_>, comic_id: &str) -> Option<ManhuaguiChapterRef> {
    let href = link.value().attr("href")?;
    let id = chapter_id_from_href(href, comic_id)?;
    let title = link
        .value()
        .attr("title")
        .map(collapse_whitespace)
        .filter(|value| !value.is_empty())
        .or_else(|| {
            let text = collapse_whitespace(&link.text().collect::<String>());
            (!text.is_empty()).then_some(text)
        })
        .unwrap_or_else(|| format!("第{id}话"));

    Some(ManhuaguiChapterRef { id, title })
}

fn chapter_id_from_href(href: &str, comic_id: &str) -> Option<String> {
    let base = Url::parse(BASE_URL).ok()?;
    let url = base.join(href.trim()).ok()?;
    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|part| !part.is_empty())
        .collect();
    if segments.len() == 1 {
        let chapter = segments[0].strip_suffix(".html")?;
        return numeric_id(chapter, "manhuagui").ok();
    }
    if segments.len() < 3 || segments[0] != "comic" || segments[1] != comic_id {
        return None;
    }

    let chapter = segments.last()?.strip_suffix(".html")?;
    numeric_id(chapter, "manhuagui").ok()
}

fn decode_lz_string(encoded: &str) -> Option<String> {
    let data = lz_str::decompress_from_base64(encoded.trim())?;
    String::from_utf16(&data)
        .ok()
        .filter(|value| !value.is_empty())
}

// The current manhuagui chapter page packs the manifest the same way as
// lanyeeee/manhuagui-downloader's `decrypt.rs`: unpack the payload against an
// LZString-compressed dictionary, then parse the resulting JSON object.
fn parse_reader_manifest(html: &str) -> Option<ManhuaguiPageData> {
    let (payload, base, compressed) = extract_decryption_data(html)?;
    let dictionary = decode_lz_string(&compressed)?;
    let unpacked = unpack_payload(&payload, base, &dictionary)?;
    let object = extract_object(&unpacked)?;
    serde_json::from_str(object).ok()
}

fn extract_decryption_data(html: &str) -> Option<(String, u32, String)> {
    let captures = DECRYPTION_RE.captures(html)?;
    let payload = captures.get(1)?.as_str().to_owned();
    let base = captures.get(2)?.as_str().parse::<u32>().ok()?;
    let _count = captures.get(3)?.as_str().parse::<usize>().ok()?;
    let compressed = captures.get(4)?.as_str().to_owned();
    Some((payload, base, compressed))
}

fn unpack_payload(payload: &str, base: u32, dictionary: &str) -> Option<String> {
    let words: Vec<&str> = dictionary.split('|').collect();
    let replaced = WORD_RE.replace_all(payload, |captures: &regex::Captures<'_>| {
        let token = &captures[0];
        if let Some(index) = parse_base(token, base)
            && let Some(word) = words.get(index)
            && !word.is_empty()
        {
            return (*word).to_owned();
        }
        token.to_owned()
    });
    Some(replaced.into_owned())
}

fn parse_base(token: &str, base: u32) -> Option<usize> {
    if !(2..=62).contains(&base) {
        return None;
    }

    let mut value = 0usize;
    for character in token.chars() {
        let digit = match character {
            '0'..='9' => character as u32 - '0' as u32,
            'a'..='z' => character as u32 - 'a' as u32 + 10,
            'A'..='Z' => character as u32 - 'A' as u32 + 36,
            _ => return None,
        };
        if digit >= base {
            return None;
        }
        value = value
            .checked_mul(base as usize)?
            .checked_add(digit as usize)?;
    }
    Some(value)
}

fn extract_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;

    for (offset, character) in text[start..].char_indices() {
        if let Some(active) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == active {
                quote = None;
            }
            continue;
        }

        match character {
            '"' | '\'' => quote = Some(character),
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(&text[start..start + offset + character.len_utf8()]);
                }
            }
            _ => {}
        }
    }
    None
}

fn manifest_pages(data: &ManhuaguiPageData) -> Vec<ReaderPage> {
    data.files
        .iter()
        .filter_map(|file| {
            let file = file.trim_end_matches(".webp");
            if file.is_empty() {
                return None;
            }
            let path = data.path.trim();
            let raw = if file.starts_with("http://") || file.starts_with("https://") {
                file.to_owned()
            } else {
                let mut root = if path.starts_with("http://") || path.starts_with("https://") {
                    path.to_owned()
                } else if path.is_empty() {
                    IMAGE_HOST.to_owned()
                } else if path.starts_with('/') {
                    format!("{IMAGE_HOST}{path}")
                } else {
                    format!("{IMAGE_HOST}/{path}")
                };
                if !root.ends_with('/') {
                    root.push('/');
                }
                format!("{root}{file}")
            };
            Url::parse(&raw).ok().map(|url| ReaderPage {
                url: url.to_string(),
                width: None,
                height: None,
                local_path: None,
            })
        })
        .collect()
}

fn cover_from_bpic(bpic: &str) -> Option<String> {
    let bpic = bpic.trim().trim_start_matches('/');
    if bpic.is_empty() {
        return None;
    }
    let raw = if bpic.starts_with("http://") || bpic.starts_with("https://") {
        bpic.to_owned()
    } else if bpic.starts_with("h/") {
        format!("https://cf.mhgui.com/cpic/{bpic}")
    } else {
        format!("https://cf.mhgui.com/cpic/h/{bpic}")
    };
    Url::parse(&raw).ok().map(|url| url.to_string())
}
fn parse_reader_identity(
    root: ElementRef<'_>,
    comic_id: &str,
    manifest: Option<&ManhuaguiPageData>,
) -> (String, Option<String>, String) {
    let page_title = text_of(root, "title").unwrap_or_default();
    let selected_title = text_of(root, "div.book-title h1")
        .or_else(|| attr_of(root, "meta[property=\"og:title\"]", "content"))
        .or_else(|| text_of(root, "h1"))
        .filter(|title| !looks_like_chapter(title));
    let (html_title, html_chapter) = reader_titles(&page_title, selected_title, comic_id);

    let title = manifest
        .and_then(|data| data.bname.as_deref())
        .map(collapse_whitespace)
        .filter(|value| !value.is_empty())
        .unwrap_or(html_title);
    let chapter_title = manifest
        .and_then(|data| data.cname.as_deref())
        .map(collapse_whitespace)
        .filter(|value| !value.is_empty())
        .or(html_chapter)
        .filter(|value| value != &title && !value.is_empty());

    let html_thumb = attr_of(root, "meta[property=\"og:image\"]", "content")
        .or_else(|| attr_of(root, "p.hcover img", "src"))
        .or_else(|| attr_of(root, "div.book-cover img", "src"))
        .map(|raw| absolute_url(BASE_URL, &raw));
    let thumb = manifest
        .and_then(|data| data.bpic.as_deref())
        .and_then(cover_from_bpic)
        .or(html_thumb)
        .unwrap_or_default();

    (title, chapter_title, thumb)
}

fn reader_titles(
    page_title: &str,
    selected_title: Option<String>,
    comic_id: &str,
) -> (String, Option<String>) {
    let normalized = page_title.replace(['_', '|', '｜'], " - ");
    let parts: Vec<String> = normalized
        .split(" - ")
        .map(collapse_whitespace)
        .filter(|part| {
            !part.is_empty()
                && !matches!(part.as_str(), "看漫画" | "漫画柜" | "在线阅读" | "在线漫画")
        })
        .collect();

    let chapter = parts.iter().find(|part| looks_like_chapter(part)).cloned();
    let work = selected_title
        .map(|title| collapse_whitespace(&title))
        .filter(|title| !title.is_empty())
        .or_else(|| {
            parts
                .iter()
                .find(|part| Some(*part) != chapter.as_ref())
                .cloned()
        })
        .or_else(|| parts.first().cloned())
        .unwrap_or_else(|| format!("manhuagui {comic_id}"));

    (work, chapter)
}

fn looks_like_chapter(value: &str) -> bool {
    value.contains('第')
        && ['话', '話', '回', '章', '集', '卷']
            .iter()
            .any(|marker| value.contains(*marker))
}

fn parse_reader_navigation(
    root: ElementRef<'_>,
    comic_id: &str,
    chapter_id: &str,
    manifest: Option<&ManhuaguiPageData>,
) -> (Option<ChapterRef>, Option<ChapterRef>) {
    let (html_previous, html_next) = parse_reader_links(root, comic_id, chapter_id);
    let Some(data) = manifest else {
        return (html_previous, html_next);
    };

    let previous = if data.prev_id > 0 {
        let id = data.prev_id.to_string();
        Some(ChapterRef {
            title: html_previous
                .as_ref()
                .filter(|item| item.id == id)
                .map_or_else(|| "上一章".to_owned(), |item| item.title.clone()),
            id,
        })
    } else {
        None
    };
    let next = if data.next_id > 0 {
        let id = data.next_id.to_string();
        Some(ChapterRef {
            title: html_next
                .as_ref()
                .filter(|item| item.id == id)
                .map_or_else(|| "下一章".to_owned(), |item| item.title.clone()),
            id,
        })
    } else {
        None
    };

    (previous, next)
}

fn parse_reader_links(
    root: ElementRef<'_>,
    comic_id: &str,
    chapter_id: &str,
) -> (Option<ChapterRef>, Option<ChapterRef>) {
    let mut previous = None;
    let mut next = None;

    for link in root.select(&selector("a[href]")) {
        let Some(href) = link.value().attr("href") else {
            continue;
        };
        let Some(id) = chapter_id_from_href(href, comic_id) else {
            continue;
        };
        if id == chapter_id {
            continue;
        }

        let text = collapse_whitespace(&link.text().collect::<String>());
        let title = link
            .value()
            .attr("title")
            .map(collapse_whitespace)
            .unwrap_or_default();
        let classes = link.value().attr("class").unwrap_or_default();
        let element_id = link.value().attr("id").unwrap_or_default();
        let rel = link.value().attr("rel").unwrap_or_default();
        let label = format!("{text} {title}");

        let is_previous = label.contains("上一章")
            || label.contains("上一话")
            || label.contains("上一話")
            || label.contains("上一回")
            || label.contains("前一章")
            || label.contains("前一話")
            || label.contains("前一回")
            || has_token(classes, "prev")
            || has_token(element_id, "prev")
            || has_token(rel, "prev");
        let is_next = label.contains("下一章")
            || label.contains("下一话")
            || label.contains("下一話")
            || label.contains("下一回")
            || label.contains("后一章")
            || label.contains("後一章")
            || label.contains("后一話")
            || has_token(classes, "next")
            || has_token(element_id, "next")
            || has_token(rel, "next");

        let reference = ChapterRef {
            id,
            title: if !title.is_empty() && !is_generic_navigation(&title) {
                title
            } else if !text.is_empty() && !is_generic_navigation(&text) {
                text
            } else {
                "chapter".to_owned()
            },
        };

        if is_previous && previous.is_none() {
            previous = Some(reference);
        } else if is_next && next.is_none() {
            next = Some(reference);
        }
    }

    (previous, next)
}

fn has_token(value: &str, token: &str) -> bool {
    value
        .split_whitespace()
        .any(|part| part.eq_ignore_ascii_case(token))
}

fn is_generic_navigation(value: &str) -> bool {
    matches!(
        value,
        "上一章"
            | "上一话"
            | "上一話"
            | "上一回"
            | "前一章"
            | "前一話"
            | "前一回"
            | "下一章"
            | "下一话"
            | "下一話"
            | "下一回"
            | "后一章"
            | "後一章"
            | "后一話"
    )
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
        chapter_groups: Vec::new(),
    };

    SearchItem::new(Manhuagui::ID, id, title, thumb, extra).ok()
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
<div class="chapter">
  <h4>单话</h4>
  <div class="chapter-list cf mt10" id="chapter-list-0"><ul style="display:block">
    <li><a class="status0" href="/comic/1/102.html" title="第2话"><span>第2话<i>20p</i></span></a></li>
    <li><a class="status0" href="/comic/1/101.html" title="第1话"><span>第1话<i>18p</i></span></a></li>
  </ul></div>
  <h4>单行本</h4>
  <div class="chapter-list cf mt10" id="chapter-list-1"><ul style="display:block">
    <li><a class="status0" href="/comic/1/202.html" title="第2卷"><span>第2卷<i>40p</i></span></a></li>
    <li><a class="status0" href="/comic/1/201.html" title="第1卷"><span>第1卷<i>38p</i></span></a></li>
  </ul></div>
</div>
"#;

    fn packed_reader_html() -> String {
        let dictionary = [
            "x",
            "\"path\"",
            "\"/ps1/\"",
            "\"files\"",
            "[\"001.jpg.webp\"]",
            "\"bname\"",
            "\"示例漫画\"",
            "\"cname\"",
            "\"第2话\"",
            "\"bpic\"",
            "\"43847.jpg\"",
            "\"nextId\"",
            "103",
            "\"prevId\"",
            "101",
        ]
        .join("|");
        let compressed = lz_str::compress_to_base64(&dictionary);
        let payload = "0({1:2,3:4,5:6,7:8,9:a,b:c,d:e});";
        let script = format!(
            "window[\"x\"]((function(p,a,c,k,e,d){{return p;}}('{payload}',62,15,'{compressed}'.split('|'),0,{{}})))"
        );
        format!(
            r#"<html><head><title>示例漫画 - 第2话 - 看漫画</title></head><body>{script}</body></html>"#
        )
    }
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
        assert_eq!(first.platform, "manhuagui");
        assert_eq!(first.id, "1");
        assert_eq!(first.title, "示例漫画");
        assert_eq!(first.thumb, "https://cf.mhgui.com/cpic/b/1.jpg");

        let extra: ManhuaguiExtra = serde_json::from_value(first.extra.clone()).unwrap();
        assert_eq!(extra.authors, ["作者A"]);
        assert_eq!(extra.status.as_deref(), Some("连载中"));
        assert_eq!(extra.latest_chapter.as_deref(), Some("第12话"));
        assert_eq!(extra.updated_at.as_deref(), Some("2026-01-02 03:04"));
        assert_eq!(extra.categories, ["冒险", "奇幻"]);
        assert_eq!(extra.description.as_deref(), Some("一个示例故事。"));
        assert_eq!(extra.score, Some(8.5));
        assert_eq!(extra.rating_count, Some(123));
        assert!(extra.chapter_groups.is_empty());
    }

    #[test]
    fn parses_detail() {
        let detail = Manhuagui.parse_detail("1", DETAIL_HTML.as_bytes()).unwrap();
        assert_eq!(detail.platform, "manhuagui");
        assert_eq!(detail.title, "示例漫画");
        assert_eq!(detail.thumb, "https://cf.mhgui.com/cpic/h/1.jpg");

        let extra: ManhuaguiExtra = serde_json::from_value(detail.extra.clone()).unwrap();
        assert_eq!(
            detail.chapters,
            vec![
                DetailChapter {
                    id: Some("101".into()),
                    title: Some("第1话".into()),
                },
                DetailChapter {
                    id: Some("102".into()),
                    title: Some("第2话".into()),
                },
                DetailChapter {
                    id: Some("201".into()),
                    title: Some("第1卷".into()),
                },
                DetailChapter {
                    id: Some("202".into()),
                    title: Some("第2卷".into()),
                },
            ]
        );
        assert_eq!(extra.original_title.as_deref(), Some("Original Title"));
        assert_eq!(extra.authors, ["作者A"]);
        assert_eq!(extra.status.as_deref(), Some("已完结"));
        assert_eq!(extra.latest_chapter.as_deref(), Some("第12话"));
        assert_eq!(extra.updated_at.as_deref(), Some("2026-01-02"));
        assert_eq!(extra.categories, ["冒险", "奇幻"]);
        assert_eq!(extra.description.as_deref(), Some("简介文本"));
        assert_eq!(extra.chapter_groups.len(), 2);
        assert_eq!(extra.chapter_groups[0].title, "单话");
        assert_eq!(extra.chapter_groups[0].chapters.len(), 2);
        assert_eq!(extra.chapter_groups[0].chapters[0].id, "101");
        assert_eq!(extra.chapter_groups[0].chapters[0].title, "第1话");
        assert_eq!(extra.chapter_groups[0].chapters[1].id, "102");
        assert_eq!(extra.chapter_groups[1].title, "单行本");
        assert_eq!(extra.chapter_groups[1].chapters[0].id, "201");
        assert_eq!(extra.chapter_groups[1].chapters[1].id, "202");
    }

    #[test]
    fn builds_reader_urls() {
        let request = Manhuagui.reader_chapter_request("1", Some("102")).unwrap();
        assert_eq!(
            request.url.as_str(),
            "https://www.manhuagui.com/comic/1/102.html"
        );

        assert!(Manhuagui.reader_chapter_request("1", None).is_err());
        assert!(Manhuagui.reader_chapter_request("1", Some("../2")).is_err());
        assert!(
            Manhuagui
                .reader_chapter_request("1/2", Some("102"))
                .is_err()
        );
    }

    #[test]
    fn a_chapter_without_pages_is_not_found() {
        let error = Manhuagui
            .parse_reader_chapter("1", Some("102"), b"<html></html>")
            .unwrap_err();

        assert_eq!(error.code(), crate::error::ErrorCode::NotFound);
    }

    #[test]
    fn parses_reader_pages_from_the_decrypted_manifest() {
        let html = packed_reader_html();
        let chapter = Manhuagui
            .parse_reader_chapter("1", Some("102"), html.as_bytes())
            .unwrap();

        assert_eq!(chapter.title, "示例漫画");
        assert_eq!(chapter.chapter_title.as_deref(), Some("第2话"));
        assert_eq!(chapter.thumb, "https://cf.mhgui.com/cpic/h/43847.jpg");
        assert_eq!(chapter.chapter_id.as_deref(), Some("102"));
        assert_eq!(chapter.pages.len(), 1);
        assert_eq!(chapter.pages[0].url, "https://i.hamreus.com/ps1/001.jpg");
        assert_eq!(
            chapter.previous.as_ref().map(|item| item.id.as_str()),
            Some("101")
        );
        assert_eq!(
            chapter.next.as_ref().map(|item| item.id.as_str()),
            Some("103")
        );
    }

    #[test]
    fn decodes_lz_string_dictionary_round_trip() {
        let compressed = lz_str::compress_to_base64("a|b|c");
        assert_eq!(decode_lz_string(&compressed).as_deref(), Some("a|b|c"));
    }

    #[test]
    fn parses_hidden_chapter_list() {
        let hidden = r#"
<div class="chapter-list"><ul>
  <li><a href="/comic/1/102.html" title="第2话">第2话</a></li>
  <li><a href="/comic/1/101.html" title="第1话">第1话</a></li>
</ul></div>
"#;
        let encoded = lz_str::compress_to_base64(hidden);
        let html =
            format!(r#"<html><body><input id="__VIEWSTATE" value="{encoded}"></body></html>"#);
        let document = Html::parse_document(&html);
        let groups = parse_chapter_groups(document.root_element(), "1");

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].title, "");
        assert_eq!(groups[0].chapters.len(), 2);
        assert_eq!(groups[0].chapters[0].id, "101");
        assert_eq!(groups[0].chapters[1].id, "102");
    }

    #[test]
    fn image_fallbacks_move_to_the_secondary_cdn() {
        let url = Url::parse("https://i.hamreus.com/ps1/1/001.jpg").unwrap();
        let fallbacks = Manhuagui.image_fallbacks(&url);

        assert_eq!(fallbacks.len(), 1);
        assert_eq!(fallbacks[0].host_str(), Some("cf.hamreus.com"));
        assert_eq!(fallbacks[0].path(), "/ps1/1/001.jpg");
        assert_eq!(fallbacks[0].query(), None);
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
    fn contract_extra(chapter_groups: Vec<ManhuaguiChapterGroup>) -> ManhuaguiExtra {
        ManhuaguiExtra {
            original_title: Some("Original".into()),
            authors: vec!["Author A".into()],
            status: Some("连载中".into()),
            latest_chapter: Some("第3话".into()),
            updated_at: Some("2024-01-01".into()),
            year: Some("2023".into()),
            region: Some("日本".into()),
            categories: vec!["喜剧".into()],
            aliases: vec!["Alias".into()],
            description: Some("Desc".into()),
            score: Some(9.5),
            rating_count: Some(42),
            chapter_groups,
        }
    }

    #[test]
    fn matches_the_platform_wire_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../src/platforms/manhuagui/fixtures/wire.json"
        ))
        .unwrap();

        let search = SearchItem::new(
            Manhuagui::ID,
            "100",
            "Manhua Work",
            "https://cf.mhgui.com/cpic/h/100.jpg",
            contract_extra(Vec::new()),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(search).unwrap(), fixture["search"]);

        let detail = ComicDetail::new(
            Manhuagui::ID,
            "Manhua Work",
            "https://cf.mhgui.com/cpic/h/100.jpg",
            vec![DetailChapter {
                id: Some("ch-1".into()),
                title: Some("第1话".into()),
            }],
            contract_extra(vec![ManhuaguiChapterGroup {
                title: String::new(),
                chapters: vec![ManhuaguiChapterRef {
                    id: "ch-1".into(),
                    title: "第1话".into(),
                }],
            }]),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(detail).unwrap(), fixture["detail"]);
    }
}
