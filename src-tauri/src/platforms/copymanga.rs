//! Copymanga (拷贝漫画 / mangacopy.com).
//!
//! Metadata comes from the official APP API (`api.2024manga.com`): search and
//! the comic detail payload are plain JSON, while the chapter list is one
//! request per site group. Reading pages come from the web chapter page, whose
//! image manifest is encrypted with AES-128-CBC using the `cct` and
//! `contentKey` variables embedded in that page. Both halves are anonymous;
//! member-only chapters simply return no pages.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::LazyLock;

use aes::cipher::{BlockDecrypt, KeyInit};
use regex::Regex;
use reqwest::RequestBuilder;
use reqwest::header::{ACCEPT_LANGUAGE, HeaderMap, HeaderValue, REFERER};
use scraper::{ElementRef, Html, Selector};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::{AppError, Result};
use crate::image::HttpClient;
use crate::image::http;
use crate::logging;
use crate::platforms::model::{
    ChapterRef, ComicDetail, DetailChapter, ReaderChapter, ReaderPage, SearchItem, SearchResponse,
};

use super::{Catalog, ImageProfile, RequestSpec, absolute_url, collapse_whitespace, host_matches};

const WEB_BASE: &str = "https://www.mangacopy.com";
/// The APP API host that currently serves detail and chapter payloads. Search
/// also answers on `api.mangacopy.com`, but detail there returns a null
/// `results`, so one host is used for every API request.
const API_BASE: &str = "https://api.2024manga.com";
const API_VERSION: &str = "3.0.0";
const API_PLATFORM: &str = "3";
const API_SOURCE: &str = "com.manga2020.app";
const REFERER_VALUE: &str = "https://www.mangacopy.com/";
const ACCEPT_LANGUAGE_VALUE: &str = "zh-CN,zh;q=0.9,en;q=0.8";
const PAGE_SIZE: u32 = 21;
const CHAPTER_PAGE_SIZE: u32 = 100;
const HOST_SUFFIXES: &[&str] = &[
    "mangacopy.com",
    "2024manga.com",
    "mangafunb.fun",
    "mangafuna.xyz",
    "copymanga.tv",
    "copy-manga.com",
    "copy20.com",
    "copy2000.site",
    "2025copy.com",
    "2026copy.com",
    "copy3000.com",
    "copy4000.com",
    "copy5000.com",
    "copy6000.com",
];

static CCT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)(?:var|let|const)\s+cct\s*=\s*['"]([^'"]*)['"]"#)
        .expect("valid copymanga cct regex")
});
static CONTENT_KEY_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)(?:var|let|const)\s+contentKey\s*=\s*['"]([^'"]*)['"]"#)
        .expect("valid copymanga content key regex")
});

/// Fields both search and detail return. Search results leave the detail-only
/// fields empty; the detail payload fills them in.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CopymangaExtra {
    original_title: Option<String>,
    authors: Vec<String>,
    status: Option<String>,
    latest_chapter: Option<String>,
    updated_at: Option<String>,
    region: Option<String>,
    categories: Vec<String>,
    aliases: Vec<String>,
    description: Option<String>,
    popularity: Option<u64>,
    chapter_groups: Vec<CopymangaChapterGroup>,
}

/// A site chapter group such as the default serialization or a volume set.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CopymangaChapterGroup {
    /// Empty when the site did not label the group.
    title: String,
    chapters: Vec<CopymangaChapterRef>,
}

/// One chapter of a copymanga work, in reading order.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CopymangaChapterRef {
    /// Chapter UUID used by the web reader route.
    id: String,
    title: String,
}

pub(super) struct Copymanga;

impl Copymanga {
    pub(super) const ID: &'static str = "copymanga";
}

impl ImageProfile for Copymanga {
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

    /// Image CDNs all serve the same asset. Use a canonical host and drop any
    /// disposable query so a mirror retry does not create a second cache file.
    fn cache_key_url(&self, url: &Url) -> String {
        let mut cache_url = url.clone();
        cache_url.set_query(None);
        if let Some(host) = cache_url.host_str().map(str::to_owned)
            && let Some(alternate) = alternate_cdn_host(&host, "mangafuna.xyz", "mangafunb.fun")
        {
            // A failed host rewrite leaves the original URL cacheable.
            let _ = cache_url.set_host(Some(&alternate));
        }
        cache_url.to_string()
    }

    /// Page and cover images are mirrored between `mangafunb.fun` and
    /// `mangafuna.xyz` with identical paths.
    fn image_fallbacks(&self, url: &Url) -> Vec<Url> {
        let Some(host) = url.host_str() else {
            return Vec::new();
        };
        let alternate = alternate_cdn_host(host, "mangafunb.fun", "mangafuna.xyz")
            .or_else(|| alternate_cdn_host(host, "mangafuna.xyz", "mangafunb.fun"));
        let Some(alternate) = alternate else {
            return Vec::new();
        };

        let mut alternate_url = url.clone();
        if alternate_url.set_host(Some(&alternate)).is_ok() {
            vec![alternate_url]
        } else {
            Vec::new()
        }
    }
}

impl Catalog for Copymanga {
    fn normalize_work_id(&self, id: &str) -> Result<String> {
        work_id(id)
    }

    fn normalize_chapter_id(&self, id: &str) -> Result<String> {
        chapter_id_value(id)
    }

    /// The APP detail payload only lists group summaries, so the complete
    /// chapter list needs one request per group. This is why copymanga
    /// overrides the shared one-request detail path.
    fn comic_detail<'a>(
        &'a self,
        client: &'a HttpClient,
        id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<ComicDetail>> + Send + 'a>> {
        Box::pin(async move {
            let id = work_id(id)?;
            let spec = detail_spec(&id)?;
            let fetched = http::fetch_document(client, self, &spec).await?;
            let parsed = parse_detail_parts(&id, &fetched.body);
            self.note_request_outcome(&fetched.url, parsed.is_ok());

            let DetailParts {
                title,
                thumb,
                extra,
                groups,
            } = parsed?;
            let groups = fetch_chapter_groups(client, self, &id, groups).await?;
            build_detail(title, thumb, extra, groups)
        })
    }

    fn search_request(&self, query: &str, page: u32) -> Result<RequestSpec> {
        let offset = page.saturating_sub(1).saturating_mul(PAGE_SIZE);
        let mut url = join_url(API_BASE, &["api", "v3", "search", "comic"])?;
        url.query_pairs_mut()
            .append_pair("limit", &PAGE_SIZE.to_string())
            .append_pair("offset", &offset.to_string())
            .append_pair("q", query)
            .append_pair("q_type", "")
            .append_pair("platform", API_PLATFORM);
        Ok(api_spec(url))
    }

    fn parse_search(&self, _query: &str, _page: u32, body: &[u8]) -> Result<SearchResponse> {
        let Some(results) = api_results::<SearchResults>(body)? else {
            return Ok(SearchResponse {
                total: 0,
                page_size: PAGE_SIZE,
                result: Vec::new(),
            });
        };

        let mut result = Vec::with_capacity(results.list.len());
        for item in results.list {
            let Ok(id) = work_id(&item.path_word) else {
                continue;
            };
            let title = clean_text(Some(item.name)).unwrap_or_else(|| id.clone());
            let thumb = clean_text(item.cover).unwrap_or_default();
            let aliases = split_aliases(item.alias.as_deref());
            let original_title = aliases
                .iter()
                .find(|alias| alias.as_str() != title.as_str())
                .cloned();
            let extra = CopymangaExtra {
                original_title,
                authors: author_names(item.author),
                status: None,
                latest_chapter: None,
                updated_at: None,
                region: None,
                categories: theme_names(item.theme),
                aliases,
                description: None,
                popularity: item.popular,
                chapter_groups: Vec::new(),
            };

            if let Ok(search_item) = SearchItem::new(Self::ID, id, title, thumb, extra) {
                result.push(search_item);
            }
        }

        Ok(SearchResponse {
            total: results.total.max(result.len() as u64),
            page_size: if results.limit == 0 {
                PAGE_SIZE
            } else {
                results.limit
            },
            result,
        })
    }

    fn reader_chapter_request(&self, id: &str, chapter_id: Option<&str>) -> Result<RequestSpec> {
        let comic = work_id(id)?;
        let chapter = chapter_id.ok_or_else(|| AppError::InvalidInput {
            field: "chapterId",
            reason: "a copymanga chapter id is required".into(),
        })?;
        let chapter = chapter_id_value(chapter)?;

        let mut url = join_url(WEB_BASE, &["comic", &comic, "chapter", &chapter])?;
        // The site reads the chapter UUID from the path and the encrypted
        // manifest from the HTML; no API headers are needed.
        url.set_query(None);
        Ok(RequestSpec::html(url))
    }

    fn parse_reader_chapter(
        &self,
        id: &str,
        chapter_id: Option<&str>,
        body: &[u8],
    ) -> Result<ReaderChapter> {
        let comic = work_id(id)?;
        let chapter = chapter_id
            .map(chapter_id_value)
            .transpose()?
            .ok_or_else(|| AppError::InvalidInput {
                field: "chapterId",
                reason: "a copymanga chapter id is required".into(),
            })?;

        let text = String::from_utf8_lossy(body);
        let pages = parse_reader_pages(&text).map_err(|error| chapter_error(&chapter, error))?;
        if pages.is_empty() {
            return Err(AppError::NotFound {
                resource: format!("copymanga chapter `{chapter}` pages"),
            });
        }

        let html = Html::parse_document(&text);
        let root = html.root_element();
        let (title, chapter_title) = parse_reader_titles(root, &comic);
        let (previous, next) = parse_reader_navigation(root, &comic, &chapter);

        Ok(ReaderChapter {
            title,
            // The chapter page does not expose the work's cover. History and
            // downloaded-detail views fall back to the title placeholder.
            thumb: String::new(),
            chapter_id: Some(chapter),
            chapter_title,
            pages,
            previous,
            next,
        })
    }
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    #[serde(default)]
    code: i64,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    results: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct SearchResults {
    #[serde(default)]
    list: Vec<SearchResultRaw>,
    #[serde(default)]
    total: u64,
    #[serde(default)]
    limit: u32,
}

#[derive(Debug, Deserialize)]
struct SearchResultRaw {
    #[serde(default)]
    name: String,
    #[serde(default)]
    alias: Option<String>,
    #[serde(default)]
    path_word: String,
    #[serde(default)]
    cover: Option<String>,
    #[serde(default)]
    author: Vec<AuthorRaw>,
    #[serde(default)]
    theme: Vec<NamedRaw>,
    #[serde(default)]
    popular: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct AuthorRaw {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct NamedRaw {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct DetailResults {
    #[serde(default)]
    comic: Option<ComicRaw>,
    #[serde(default)]
    groups: Option<BTreeMap<String, GroupSummaryRaw>>,
    #[serde(default)]
    popular: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ComicRaw {
    #[serde(default)]
    name: String,
    #[serde(default)]
    alias: Option<String>,
    #[serde(default)]
    cover: Option<String>,
    #[serde(default)]
    b_404: bool,
    #[serde(default)]
    status: Option<DisplayRaw>,
    #[serde(default)]
    region: Option<DisplayRaw>,
    #[serde(default)]
    author: Vec<AuthorRaw>,
    #[serde(default)]
    theme: Vec<NamedRaw>,
    #[serde(default)]
    brief: Option<String>,
    #[serde(default)]
    datetime_updated: Option<String>,
    #[serde(default)]
    last_chapter: Option<LastChapterRaw>,
    #[serde(default)]
    popular: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct DisplayRaw {
    #[serde(default)]
    display: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LastChapterRaw {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GroupSummaryRaw {
    #[serde(default)]
    path_word: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    count: u64,
}

#[derive(Debug, Deserialize)]
struct ChapterResults {
    #[serde(default)]
    list: Vec<ChapterRaw>,
    #[serde(default)]
    total: u64,
    #[serde(default)]
    limit: u32,
}

#[derive(Debug, Deserialize)]
struct ChapterRaw {
    #[serde(default)]
    uuid: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    index: u32,
}

#[derive(Debug, Deserialize)]
struct PageEntry {
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug)]
struct DetailParts {
    title: String,
    thumb: String,
    extra: CopymangaExtra,
    groups: Vec<GroupSummaryRaw>,
}

fn api_spec(url: Url) -> RequestSpec {
    RequestSpec::json(url).with_headers(api_headers())
}

fn api_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("platform", HeaderValue::from_static(API_PLATFORM));
    headers.insert("version", HeaderValue::from_static(API_VERSION));
    headers.insert("webp", HeaderValue::from_static("1"));
    headers.insert("authorization", HeaderValue::from_static("Token"));
    headers.insert("x-requested-with", HeaderValue::from_static(API_SOURCE));
    headers
}

fn join_url(base: &str, segments: &[&str]) -> Result<Url> {
    let mut url = Url::parse(base)
        .map_err(|error| AppError::Unexpected(format!("invalid copymanga URL: {error}")))?;
    {
        let mut path = url
            .path_segments_mut()
            .map_err(|_| AppError::Unexpected("copymanga URL cannot be used as a base".into()))?;
        for segment in segments {
            path.push(segment);
        }
    }
    Ok(url)
}

fn detail_spec(path_word: &str) -> Result<RequestSpec> {
    let mut url = join_url(API_BASE, &["api", "v3", "comic2", path_word])?;
    url.query_pairs_mut().append_pair("platform", API_PLATFORM);
    Ok(api_spec(url))
}

fn chapter_list_spec(path_word: &str, group: &str, offset: u32) -> Result<RequestSpec> {
    let mut url = join_url(
        API_BASE,
        &["api", "v3", "comic", path_word, "group", group, "chapters"],
    )?;
    url.query_pairs_mut()
        .append_pair("limit", &CHAPTER_PAGE_SIZE.to_string())
        .append_pair("offset", &offset.to_string())
        .append_pair("platform", API_PLATFORM);
    Ok(api_spec(url))
}

fn api_results<T: DeserializeOwned>(body: &[u8]) -> Result<Option<T>> {
    let response: ApiResponse = serde_json::from_slice(body)?;
    if response.code != 200 {
        let message = response.message.unwrap_or_default();
        return Err(AppError::Unexpected(format!(
            "copymanga API error {}: {message}",
            response.code
        )));
    }
    response
        .results
        .map(serde_json::from_value)
        .transpose()
        .map_err(Into::into)
}

fn parse_detail_parts(id: &str, body: &[u8]) -> Result<DetailParts> {
    let Some(results) = api_results::<DetailResults>(body)? else {
        return Err(not_found(id));
    };
    let Some(comic) = results.comic else {
        return Err(not_found(id));
    };
    if comic.b_404 {
        return Err(not_found(id));
    }

    let title = collapse_whitespace(&comic.name);
    if title.is_empty() {
        return Err(not_found(id));
    }
    let thumb = clean_text(comic.cover).unwrap_or_default();
    let aliases = split_aliases(comic.alias.as_deref());
    let original_title = aliases
        .iter()
        .find(|alias| alias.as_str() != title.as_str())
        .cloned();
    let extra = CopymangaExtra {
        original_title,
        authors: author_names(comic.author),
        status: display_of(comic.status),
        latest_chapter: comic
            .last_chapter
            .and_then(|chapter| clean_text(chapter.name)),
        updated_at: clean_text(comic.datetime_updated),
        region: display_of(comic.region),
        categories: theme_names(comic.theme),
        aliases,
        description: clean_text(comic.brief),
        popularity: comic.popular.or(results.popular),
        chapter_groups: Vec::new(),
    };

    let groups = results
        .groups
        .unwrap_or_default()
        .into_iter()
        .map(|(key, group)| {
            let path_word = clean_text(Some(group.path_word)).unwrap_or(key);
            GroupSummaryRaw {
                path_word,
                name: group.name,
                count: group.count,
            }
        })
        .collect::<Vec<_>>();

    Ok(DetailParts {
        title,
        thumb,
        extra,
        groups: sort_groups(groups),
    })
}

fn sort_groups(mut groups: Vec<GroupSummaryRaw>) -> Vec<GroupSummaryRaw> {
    // The main serialization should be read first; the remaining groups keep a
    // stable name order instead of relying on the JSON map order.
    groups.sort_by(|left, right| {
        let left_default = left.path_word != "default";
        let right_default = right.path_word != "default";
        left_default
            .cmp(&right_default)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.path_word.cmp(&right.path_word))
    });
    groups
}

async fn fetch_chapter_groups(
    client: &HttpClient,
    platform: &Copymanga,
    path_word: &str,
    summaries: Vec<GroupSummaryRaw>,
) -> Result<Vec<CopymangaChapterGroup>> {
    let mut groups = Vec::with_capacity(summaries.len());

    for summary in summaries {
        let group_path = summary.path_word.trim();
        if group_path.is_empty() || !is_safe_token(group_path, 128) {
            logging::warn(
                "copymanga",
                format!("ignoring invalid chapter group `{}`", summary.path_word),
            );
            continue;
        }

        let mut chapters = Vec::new();
        let mut offset = 0u32;
        let mut total = summary.count;
        let mut failed = None;

        loop {
            match fetch_chapter_page(client, platform, path_word, group_path, offset).await {
                Ok(page) => {
                    total = total.max(page.total);
                    let page_limit = if page.limit == 0 {
                        CHAPTER_PAGE_SIZE
                    } else {
                        page.limit
                    };
                    let received = page.list.len() as u32;
                    chapters.extend(page.list.into_iter().filter_map(chapter_ref));
                    offset = offset.saturating_add(page_limit);
                    if received < page_limit || u64::from(offset) >= total {
                        break;
                    }
                    // Defensive guard: a broken `total` must not loop forever.
                    if offset > 100_000 {
                        logging::warn(
                            "copymanga",
                            format!("chapter group `{group_path}` exceeded the offset guard"),
                        );
                        break;
                    }
                }
                Err(error) => {
                    failed = Some(error);
                    break;
                }
            }
        }

        if let Some(error) = failed {
            if group_path == "default" {
                return Err(error);
            }
            logging::warn(
                "copymanga",
                format!("failed to load chapter group `{group_path}`: {error}"),
            );
            chapters.clear();
        }

        groups.push(CopymangaChapterGroup {
            title: collapse_whitespace(&summary.name),
            chapters,
        });
    }

    Ok(groups)
}

async fn fetch_chapter_page(
    client: &HttpClient,
    platform: &Copymanga,
    path_word: &str,
    group: &str,
    offset: u32,
) -> Result<ChapterResults> {
    let spec = chapter_list_spec(path_word, group, offset)?;
    let fetched = http::fetch_document(client, platform, &spec).await?;
    let parsed = parse_chapter_page(&fetched.body);
    platform.note_request_outcome(&fetched.url, parsed.is_ok());
    parsed
}

fn parse_chapter_page(body: &[u8]) -> Result<ChapterResults> {
    let Some(results) = api_results::<ChapterResults>(body)? else {
        return Ok(ChapterResults {
            list: Vec::new(),
            total: 0,
            limit: 0,
        });
    };
    Ok(results)
}

fn chapter_ref(chapter: ChapterRaw) -> Option<CopymangaChapterRef> {
    let id = clean_text(Some(chapter.uuid))?;
    if !is_safe_token(&id, 64) {
        return None;
    }
    let title = clean_text(Some(chapter.name))
        .unwrap_or_else(|| format!("第{}話", chapter.index.saturating_add(1)));
    Some(CopymangaChapterRef { id, title })
}

fn build_detail(
    title: String,
    thumb: String,
    mut extra: CopymangaExtra,
    groups: Vec<CopymangaChapterGroup>,
) -> Result<ComicDetail> {
    let chapters = groups
        .iter()
        .flat_map(|group| group.chapters.iter())
        .map(|chapter| DetailChapter {
            id: Some(chapter.id.clone()),
            title: (!chapter.title.is_empty()).then(|| chapter.title.clone()),
        })
        .collect();
    extra.chapter_groups = groups;

    ComicDetail::new(Copymanga::ID, title, thumb, chapters, extra)
}

fn parse_reader_pages(html: &str) -> Result<Vec<ReaderPage>> {
    let Some(cct) = regex_capture(&CCT_RE, html) else {
        return Ok(Vec::new());
    };
    let Some(content_key) = regex_capture(&CONTENT_KEY_RE, html) else {
        return Ok(Vec::new());
    };
    if cct.len() != 16 || content_key.len() <= 16 || content_key.len() % 2 != 0 {
        return Ok(Vec::new());
    }

    let (iv, ciphertext) = content_key.split_at(16);
    let ciphertext = hex::decode(ciphertext)
        .map_err(|error| AppError::Unexpected(format!("invalid copymanga content key: {error}")))?;
    let plain = decrypt_cbc(&ciphertext, cct.as_bytes(), iv.as_bytes())?;
    let text = String::from_utf8(plain).map_err(|error| {
        AppError::Unexpected(format!("copymanga page manifest is not UTF-8: {error}"))
    })?;
    let entries: Vec<PageEntry> = serde_json::from_str(&text)?;

    Ok(entries
        .into_iter()
        .filter_map(|entry| {
            let raw = entry.url?;
            let raw = raw.trim();
            if raw.is_empty() {
                return None;
            }
            let raw = absolute_url(WEB_BASE, raw);
            Url::parse(&raw).ok().map(|url| ReaderPage {
                url: url.to_string(),
                width: None,
                height: None,
                local_path: None,
            })
        })
        .collect())
}

/// AES-128-CBC decryption with PKCS#7 padding. The web page uses CryptoJS
/// `AES.decrypt(ciphertext, Utf8.parse(cct), { iv: Utf8.parse(contentKey[0..16]) })`.
// The project supports Rust 1.85, where `usize::is_multiple_of` does not exist.
#[allow(clippy::manual_is_multiple_of)]
fn decrypt_cbc(ciphertext: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>> {
    if key.len() != 16 || iv.len() != 16 || ciphertext.is_empty() || ciphertext.len() % 16 != 0 {
        return Err(AppError::Unexpected(
            "copymanga content is not valid AES-128-CBC input".into(),
        ));
    }

    let cipher = aes::Aes128::new_from_slice(key)
        .map_err(|_| AppError::Unexpected("invalid copymanga content key".into()))?;
    let mut previous = <[u8; 16]>::try_from(iv)
        .map_err(|_| AppError::Unexpected("invalid copymanga content IV".into()))?;
    let mut plain = Vec::with_capacity(ciphertext.len());

    // The length check above guarantees every chunk is a full AES block.
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

fn unpad_pkcs7(mut data: Vec<u8>) -> Result<Vec<u8>> {
    let Some(&padding) = data.last() else {
        return Err(AppError::Unexpected(
            "copymanga content has no padding".into(),
        ));
    };
    let padding = padding as usize;
    if padding == 0
        || padding > 16
        || padding > data.len()
        || !data[data.len() - padding..]
            .iter()
            .all(|byte| *byte as usize == padding)
    {
        return Err(AppError::Unexpected(
            "copymanga content has invalid padding".into(),
        ));
    }
    data.truncate(data.len() - padding);
    Ok(data)
}

fn parse_reader_titles(root: ElementRef<'_>, comic: &str) -> (String, Option<String>) {
    let mut work = None;
    let mut chapter = None;

    if let Some(header) = text_of(root, "h4.header") {
        if let Some((left, right)) = header.rsplit_once('/') {
            work = clean_text(Some(left.to_string()));
            chapter = clean_text(Some(right.to_string()));
        } else {
            work = clean_text(Some(header));
        }
    }

    if chapter.is_none()
        && let Some(page_title) = text_of(root, "title")
    {
        let parts = page_title
            .split(" - ")
            .map(collapse_whitespace)
            .filter(|part| !part.is_empty() && !is_site_suffix(part))
            .collect::<Vec<_>>();
        if work.is_none() {
            work = parts.first().cloned();
        }
        chapter = parts.get(1).cloned();
    }

    let work = work
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| format!("copymanga {comic}"));
    let chapter = chapter.filter(|value| !value.is_empty() && value != &work);

    (work, chapter)
}

fn parse_reader_navigation(
    root: ElementRef<'_>,
    comic: &str,
    chapter: &str,
) -> (Option<ChapterRef>, Option<ChapterRef>) {
    let mut previous = None;
    let mut next = None;

    for link in root.select(&selector(
        "div.comicContent-prev a[href], div.comicContent-next a[href]",
    )) {
        let Some(href) = link.value().attr("href") else {
            continue;
        };
        let Some(id) = chapter_id_from_href(href, comic) else {
            continue;
        };
        if id == chapter {
            continue;
        }

        let label = collapse_whitespace(&link.text().collect::<String>());
        let direction = navigation_direction(link).or_else(|| {
            if label.contains("下一") {
                Some(true)
            } else if label.contains("上一") {
                Some(false)
            } else {
                None
            }
        });
        let title = if !label.is_empty() {
            label
        } else if direction == Some(true) {
            "下一話".to_owned()
        } else {
            "上一話".to_owned()
        };
        let reference = ChapterRef { id, title };

        match direction {
            Some(true) if next.is_none() => next = Some(reference),
            Some(false) if previous.is_none() => previous = Some(reference),
            _ => {}
        }
    }

    (previous, next)
}

fn navigation_direction(link: ElementRef<'_>) -> Option<bool> {
    let mut current = Some(link);
    while let Some(element) = current {
        if let Some(classes) = element.value().attr("class") {
            if classes
                .split_whitespace()
                .any(|class| class == "comicContent-next")
            {
                return Some(true);
            }
            if classes
                .split_whitespace()
                .any(|class| class == "comicContent-prev")
            {
                return Some(false);
            }
        }
        current = element.parent().and_then(ElementRef::wrap);
    }
    None
}

fn chapter_id_from_href(href: &str, comic: &str) -> Option<String> {
    let base = Url::parse(WEB_BASE).ok()?;
    let url = base.join(href.trim()).ok()?;
    let segments = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.len() != 4
        || segments[0] != "comic"
        || segments[1] != comic
        || segments[2] != "chapter"
    {
        return None;
    }
    chapter_id_value(segments[3]).ok()
}

fn regex_capture(regex: &Regex, text: &str) -> Option<String> {
    regex
        .captures(text)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_string())
}

fn clean_text(value: Option<String>) -> Option<String> {
    value
        .map(|text| collapse_whitespace(&text))
        .filter(|text| !text.is_empty())
}

fn display_of(value: Option<DisplayRaw>) -> Option<String> {
    value.and_then(|item| clean_text(item.display))
}

fn author_names(values: Vec<AuthorRaw>) -> Vec<String> {
    values
        .into_iter()
        .map(|author| collapse_whitespace(&author.name))
        .filter(|name| !name.is_empty())
        .collect()
}

fn theme_names(values: Vec<NamedRaw>) -> Vec<String> {
    values
        .into_iter()
        .map(|theme| collapse_whitespace(&theme.name))
        .filter(|name| !name.is_empty())
        .collect()
}

fn split_aliases(raw: Option<&str>) -> Vec<String> {
    raw.unwrap_or_default()
        .split([',', '，', ';', '；'])
        .map(collapse_whitespace)
        .filter(|value| !value.is_empty())
        .fold(Vec::new(), |mut aliases, value| {
            if !aliases.contains(&value) {
                aliases.push(value);
            }
            aliases
        })
}

/// Swap one CDN suffix for another while keeping every subdomain label.
fn alternate_cdn_host(host: &str, from: &str, to: &str) -> Option<String> {
    let prefix = host.strip_suffix(from)?;
    if !prefix.is_empty() && !prefix.ends_with('.') {
        return None;
    }
    Some(format!("{prefix}{to}"))
}

fn is_site_suffix(value: &str) -> bool {
    value.contains("拷貝漫畫")
        || value.contains("拷贝漫画")
        || value.contains("CopyManga")
        || value.contains("copymanga")
}

fn is_safe_token(value: &str, max_length: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_length
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

fn work_id(id: &str) -> Result<String> {
    let trimmed = id.trim();
    if !is_safe_token(trimmed, 128) {
        return Err(AppError::InvalidInput {
            field: "id",
            reason: "copymanga work ids are path words (letters, digits, `-`, `_`)".into(),
        });
    }
    Ok(trimmed.to_string())
}

fn chapter_id_value(id: &str) -> Result<String> {
    let trimmed = id.trim();
    if !is_safe_token(trimmed, 64) {
        return Err(AppError::InvalidInput {
            field: "chapterId",
            reason: "copymanga chapter ids are UUIDs (letters, digits, `-`)".into(),
        });
    }
    Ok(trimmed.to_string())
}

fn not_found(id: &str) -> AppError {
    AppError::NotFound {
        resource: format!("copymanga comic `{id}`"),
    }
}

/// Replace a decryption failure with the chapter-specific not-found error the
/// reader can display, except for malformed upstream data.
fn chapter_error(chapter: &str, error: AppError) -> AppError {
    match error {
        AppError::Unexpected(reason) => AppError::Unexpected(format!(
            "copymanga chapter `{chapter}` page manifest: {reason}"
        )),
        other => other,
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use aes::cipher::BlockEncrypt;

    const SEARCH_JSON: &str = r#"
{
  "code": 200,
  "message": "请求成功",
  "results": {
    "list": [
      {
        "name": "戀愛代理",
        "alias": "恋爱代理,\t恋爱代行,恋愛代行",
        "path_word": "lianaidaixing",
        "cover": "https://sl.mangafunb.fun/l/lianaidaixing/cover/1700201434.jpg.328x422.jpg",
        "author": [
          { "name": "西沢5㍉", "alias": "西沢5㍉" },
          { "name": "赤坂アカ", "alias": null }
        ],
        "popular": 364200,
        "theme": [{ "name": "愛情", "path_word": "aiqing" }]
      },
      {
        "name": "   ",
        "alias": null,
        "path_word": "broken path",
        "cover": null,
        "author": [],
        "popular": null,
        "theme": []
      }
    ],
    "total": 274,
    "limit": 21,
    "offset": 0
  }
}
"#;

    const DETAIL_JSON: &str = r#"
{
  "code": 200,
  "message": "请求成功",
  "results": {
    "is_lock": false,
    "is_login": false,
    "comic": {
      "uuid": "1f3b2590-e38e-11ed-a221-024352452ce0",
      "b_404": false,
      "b_hidden": false,
      "ban": 0,
      "name": "戀愛代理",
      "alias": "恋爱代理,\t恋爱代行,恋愛代行",
      "path_word": "lianaidaixing",
      "cover": "https://sl.mangafunb.fun/l/lianaidaixing/cover/1700201434.jpg.328x422.jpg",
      "status": { "value": 0, "display": "連載中" },
      "region": { "value": 0, "display": "日本" },
      "author": [
        { "name": "西沢5㍉", "path_word": "xzkr" },
        { "name": "赤坂アカ", "path_word": "chibanaka" }
      ],
      "theme": [
        { "name": "愛情", "path_word": "aiqing" },
        { "name": "歡樂向", "path_word": "huanlexiang" },
        { "name": "校园", "path_word": "xiaoyuan" }
      ],
      "brief": "赤坂明&西澤5㍉老師搭檔創作的戀愛喜劇新作",
      "datetime_updated": "2024-09-20",
      "last_chapter": { "uuid": "9c7df98e-76a5-11ef-b587-3f487b7d9a9a", "name": "第39話" },
      "popular": 364200
    },
    "popular": 364200,
    "groups": {
      "default": { "path_word": "default", "count": 39, "name": "默認" },
      "tankobon": { "path_word": "tankobon", "count": 2, "name": "单行本" }
    }
  }
}
"#;

    fn encrypted_content_key(payload: &str, cct: &str, iv: &[u8; 16]) -> String {
        let cipher = aes::Aes128::new_from_slice(cct.as_bytes()).expect("16-byte key");
        let padding = 16 - (payload.len() % 16);
        let mut padded = payload.as_bytes().to_vec();
        padded.extend(std::iter::repeat_n(padding as u8, padding));

        let mut previous = *iv;
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
        let iv_text = std::str::from_utf8(iv).expect("ASCII IV");
        format!("{iv_text}{}", hex::encode(encrypted))
    }

    fn reader_html(payload: &str, previous: Option<&str>, next: Option<&str>) -> String {
        let iv = b"0123456789abcdef";
        let content_key = encrypted_content_key(payload, "0p0zzpvv.nmn.oop", iv);
        let previous = previous
            .map(|id| {
                format!(
                    r#"<div class="comicContent-prev"><a href="/comic/lianaidaixing/chapter/{id}">上一話</a></div>"#
                )
            })
            .unwrap_or_else(|| {
                r#"<div class="comicContent-prev"><a href="" class="prev-null">上一話</a></div>"#
                    .to_string()
            });
        let next = next
            .map(|id| {
                format!(
                    r#"<div class="comicContent-next"><a href="/comic/lianaidaixing/chapter/{id}">下一話</a></div>"#
                )
            })
            .unwrap_or_else(|| {
                r#"<div class="comicContent-next"><a href="" class="prev-null">下一話</a></div>"#
                    .to_string()
            });

        format!(
            r#"<html><head><title>戀愛代理 - 第01話 - 拷貝漫畫 拷贝漫画</title></head>
<body>
<h4 class="header">戀愛代理/第01話</h4>
{previous}
{next}
<script>
var cct = '0p0zzpvv.nmn.oop';
var contentKey = '{content_key}';
</script>
</body></html>"#
        )
    }

    #[test]
    fn builds_search_and_detail_urls() {
        let search = Copymanga.search_request("test query", 2).unwrap();
        let url = search.url.as_str();
        assert!(url.starts_with("https://api.2024manga.com/api/v3/search/comic?"));
        assert!(url.contains("q=test+query"));
        assert!(url.contains("offset=21"));
        assert!(url.contains("limit=21"));
        assert_eq!(
            search.headers.get("platform").unwrap().to_str().unwrap(),
            "3"
        );

        let detail = detail_spec("lianaidaixing").unwrap();
        assert_eq!(
            detail.url.as_str(),
            "https://api.2024manga.com/api/v3/comic2/lianaidaixing?platform=3"
        );
        assert!(work_id("bad/path").is_err());
        assert!(work_id("../x").is_err());
        assert_eq!(work_id("OjounoYokushitsu").unwrap(), "OjounoYokushitsu");
    }

    #[test]
    fn parses_search_results() {
        let response = Copymanga
            .parse_search("恋爱", 1, SEARCH_JSON.as_bytes())
            .unwrap();
        assert_eq!(response.total, 274);
        assert_eq!(response.page_size, 21);
        // The invalid path word is skipped instead of failing the whole page.
        assert_eq!(response.result.len(), 1);

        let first = &response.result[0];
        assert_eq!(first.platform, "copymanga");
        assert_eq!(first.id, "lianaidaixing");
        assert_eq!(first.title, "戀愛代理");
        assert_eq!(
            first.thumb,
            "https://sl.mangafunb.fun/l/lianaidaixing/cover/1700201434.jpg.328x422.jpg"
        );

        let extra: CopymangaExtra = serde_json::from_value(first.extra.clone()).unwrap();
        assert_eq!(extra.original_title.as_deref(), Some("恋爱代理"));
        assert_eq!(extra.authors, ["西沢5㍉", "赤坂アカ"]);
        assert_eq!(extra.categories, ["愛情"]);
        assert_eq!(extra.aliases, ["恋爱代理", "恋爱代行", "恋愛代行"]);
        assert_eq!(extra.popularity, Some(364200));
        assert!(extra.chapter_groups.is_empty());
        assert!(extra.status.is_none());
    }

    #[test]
    fn parses_detail_groups_and_orders_default_first() {
        let parts = parse_detail_parts("lianaidaixing", DETAIL_JSON.as_bytes()).unwrap();
        assert_eq!(parts.title, "戀愛代理");
        assert_eq!(
            parts
                .groups
                .iter()
                .map(|group| group.path_word.as_str())
                .collect::<Vec<_>>(),
            ["default", "tankobon"]
        );

        let extra = parts.extra;
        assert_eq!(extra.original_title.as_deref(), Some("恋爱代理"));
        assert_eq!(extra.authors, ["西沢5㍉", "赤坂アカ"]);
        assert_eq!(extra.status.as_deref(), Some("連載中"));
        assert_eq!(extra.latest_chapter.as_deref(), Some("第39話"));
        assert_eq!(extra.updated_at.as_deref(), Some("2024-09-20"));
        assert_eq!(extra.region.as_deref(), Some("日本"));
        assert_eq!(extra.categories, ["愛情", "歡樂向", "校园"]);
        assert_eq!(extra.aliases, ["恋爱代理", "恋爱代行", "恋愛代行"]);
        assert_eq!(
            extra.description.as_deref(),
            Some("赤坂明&西澤5㍉老師搭檔創作的戀愛喜劇新作")
        );
        assert_eq!(extra.popularity, Some(364200));
        assert!(extra.chapter_groups.is_empty());
    }

    #[test]
    fn a_missing_detail_is_not_found() {
        let error = parse_detail_parts("missing", br#"{"code":200,"results":null}"#).unwrap_err();
        assert_eq!(error.code(), ErrorCode::NotFound);

        let error = parse_detail_parts(
            "missing",
            br#"{"code":200,"results":{"comic":{"b_404":true,"name":"x"}}}"#,
        )
        .unwrap_err();
        assert_eq!(error.code(), ErrorCode::NotFound);
    }

    #[test]
    fn builds_reader_urls() {
        let request = Copymanga
            .reader_chapter_request(
                "lianaidaixing",
                Some("b9741745-e439-11ed-9b25-024352452ce0"),
            )
            .unwrap();
        assert_eq!(
            request.url.as_str(),
            "https://www.mangacopy.com/comic/lianaidaixing/chapter/b9741745-e439-11ed-9b25-024352452ce0"
        );

        assert!(
            Copymanga
                .reader_chapter_request("lianaidaixing", None)
                .is_err()
        );
        assert!(
            Copymanga
                .reader_chapter_request("lianaidaixing", Some("../x"))
                .is_err()
        );
        assert!(
            Copymanga
                .reader_chapter_request("bad/path", Some("x"))
                .is_err()
        );
    }

    #[test]
    fn parses_reader_pages_and_navigation() {
        let html = reader_html(
            r#"[{"url": "https://sl.mangafunb.fun/l/lianaidaixing/1.jpg.c1500x.jpg"}]"#,
            Some("11111111-1111-1111-1111-111111111111"),
            Some("22222222-2222-2222-2222-222222222222"),
        );
        let chapter = Copymanga
            .parse_reader_chapter(
                "lianaidaixing",
                Some("b9741745-e439-11ed-9b25-024352452ce0"),
                html.as_bytes(),
            )
            .unwrap();

        assert_eq!(chapter.title, "戀愛代理");
        assert_eq!(chapter.chapter_title.as_deref(), Some("第01話"));
        assert_eq!(
            chapter.chapter_id.as_deref(),
            Some("b9741745-e439-11ed-9b25-024352452ce0")
        );
        assert!(chapter.thumb.is_empty());
        assert_eq!(chapter.pages.len(), 1);
        assert_eq!(
            chapter.pages[0].url,
            "https://sl.mangafunb.fun/l/lianaidaixing/1.jpg.c1500x.jpg"
        );
        assert_eq!(
            chapter.previous.as_ref().map(|item| item.id.as_str()),
            Some("11111111-1111-1111-1111-111111111111")
        );
        assert_eq!(
            chapter.previous.as_ref().map(|item| item.title.as_str()),
            Some("上一話")
        );
        assert_eq!(
            chapter.next.as_ref().map(|item| item.id.as_str()),
            Some("22222222-2222-2222-2222-222222222222")
        );
    }

    #[test]
    fn a_chapter_without_a_manifest_is_not_found() {
        let error = Copymanga
            .parse_reader_chapter("lianaidaixing", Some("chapter-uuid"), b"<html></html>")
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::NotFound);
    }

    #[test]
    fn rejects_malformed_content_keys() {
        let html = r#"<html><body><script>
            var cct = '0p0zzpvv.nmn.oop';
            var contentKey = 'short';
        </script></body></html>"#;
        let pages = parse_reader_pages(html).unwrap();
        assert!(pages.is_empty());
    }

    #[test]
    fn image_fallbacks_move_between_the_two_cdn_domains() {
        let url = Url::parse("https://sl.mangafunb.fun/l/lianaidaixing/1.jpg.c1500x.jpg").unwrap();
        let fallbacks = Copymanga.image_fallbacks(&url);
        assert_eq!(fallbacks.len(), 1);
        assert_eq!(fallbacks[0].host_str(), Some("sl.mangafuna.xyz"));
        assert_eq!(fallbacks[0].path(), url.path());

        let alternate = Url::parse("https://sl.mangafuna.xyz/l/lianaidaixing/1.jpg").unwrap();
        assert_eq!(
            Copymanga.image_fallbacks(&alternate)[0].host_str(),
            Some("sl.mangafunb.fun")
        );
        assert!(
            Copymanga
                .image_fallbacks(&Url::parse("https://example.com/a.jpg").unwrap())
                .is_empty()
        );
    }

    #[test]
    fn cache_key_uses_the_canonical_cdn_and_drops_query() {
        let alternate = Url::parse("https://sl.mangafuna.xyz/l/1.jpg?x=1").unwrap();
        assert_eq!(
            Copymanga.cache_key_url(&alternate),
            "https://sl.mangafunb.fun/l/1.jpg"
        );

        let plain = Url::parse("https://sc.mangafunb.fun/c/1.jpg").unwrap();
        assert_eq!(
            Copymanga.cache_key_url(&plain),
            "https://sc.mangafunb.fun/c/1.jpg"
        );
    }

    fn contract_extra(chapter_groups: Vec<CopymangaChapterGroup>) -> CopymangaExtra {
        CopymangaExtra {
            original_title: Some("恋爱代理".into()),
            authors: vec!["西沢5㍉".into(), "赤坂アカ".into()],
            status: Some("連載中".into()),
            latest_chapter: Some("第39話".into()),
            updated_at: Some("2024-09-20".into()),
            region: Some("日本".into()),
            categories: vec!["愛情".into(), "歡樂向".into(), "校园".into()],
            aliases: vec!["恋爱代理".into(), "恋爱代行".into(), "恋愛代行".into()],
            description: Some("赤坂明&西澤5㍉老師搭檔創作的戀愛喜劇新作".into()),
            popularity: Some(364200),
            chapter_groups,
        }
    }

    #[test]
    fn matches_the_platform_wire_fixture() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../src/platforms/copymanga/fixtures/wire.json"
        ))
        .unwrap();

        let mut search_extra = contract_extra(Vec::new());
        search_extra.status = None;
        search_extra.latest_chapter = None;
        search_extra.updated_at = None;
        search_extra.region = None;
        search_extra.categories = vec!["愛情".into()];
        search_extra.description = None;

        let search = SearchItem::new(
            Copymanga::ID,
            "lianaidaixing",
            "戀愛代理",
            "https://sl.mangafunb.fun/l/lianaidaixing/cover/1700201434.jpg.328x422.jpg",
            search_extra,
        )
        .unwrap();
        assert_eq!(serde_json::to_value(search).unwrap(), fixture["search"]);

        let chapters = vec![
            CopymangaChapterRef {
                id: "b9741745-e439-11ed-9b25-024352452ce0".into(),
                title: "第01話".into(),
            },
            CopymangaChapterRef {
                id: "a036001e-ef46-11ed-8221-024352452ce0".into(),
                title: "第02話".into(),
            },
        ];
        let detail = ComicDetail::new(
            Copymanga::ID,
            "戀愛代理",
            "https://sl.mangafunb.fun/l/lianaidaixing/cover/1700201434.jpg.328x422.jpg",
            chapters
                .iter()
                .map(|chapter| DetailChapter {
                    id: Some(chapter.id.clone()),
                    title: Some(chapter.title.clone()),
                })
                .collect(),
            contract_extra(vec![CopymangaChapterGroup {
                title: "默認".into(),
                chapters,
            }]),
        )
        .unwrap();
        assert_eq!(serde_json::to_value(detail).unwrap(), fixture["detail"]);
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn live_round_trip() {
        let client = HttpClient::default();
        let search_spec = Copymanga.search_request("芙莉莲", 1).unwrap();
        let fetched = http::fetch_document(&client, &Copymanga, &search_spec)
            .await
            .unwrap();
        let search = Copymanga.parse_search("芙莉莲", 1, &fetched.body).unwrap();
        let first = search.result.first().expect("search results");
        let id = first.id.clone();
        eprintln!("search first: {} ({})", first.title, id);

        let detail = Copymanga.comic_detail(&client, &id).await.unwrap();
        let chapter = detail
            .chapters
            .first()
            .and_then(|chapter| chapter.id.clone())
            .expect("first chapter");
        eprintln!(
            "detail: {} chapters={} groups={}",
            detail.title,
            detail.chapters.len(),
            detail.extra["chapterGroups"]
                .as_array()
                .map_or(0, |groups| groups.len())
        );

        let reader_spec = Copymanga
            .reader_chapter_request(&id, Some(&chapter))
            .unwrap();
        let fetched = http::fetch_document(&client, &Copymanga, &reader_spec)
            .await
            .unwrap();
        let reader = Copymanga
            .parse_reader_chapter(&id, Some(&chapter), &fetched.body)
            .unwrap();
        eprintln!(
            "reader: {} / {:?} pages={}",
            reader.title,
            reader.chapter_title,
            reader.pages.len()
        );
        assert!(!reader.pages.is_empty());

        let page_url = Url::parse(&reader.pages[0].url).unwrap();
        let image = http::download_image(&client, &Copymanga, &page_url)
            .await
            .unwrap();
        eprintln!("first image bytes={}", image.bytes.len());
        assert!(!image.bytes.is_empty());
    }
}
