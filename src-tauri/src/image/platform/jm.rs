//! JM uses the mobile APP API rather than the HTML site.
//!
//! The site answers 403 for both search and album pages on region-locked
//! networks, while the APP hosts keep working and return structured JSON. The
//! protocol details (signing, decryption, URL rules) live in [`super::jm_api`].

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

use reqwest::RequestBuilder;
use reqwest::header::{ACCEPT_LANGUAGE, REFERER, USER_AGENT};
use url::Url;

use crate::error::{AppError, Result};
use crate::image::http::{self, HttpClient};
use crate::image::model::{
    ChapterRef, ComicDetail, DetailExtra, JmChapterRef, JmDetailExtra, JmExtra, ReaderChapter,
    ReaderPage, SearchExtra, SearchItem, SearchResponse,
};
use crate::logging;

use super::jm_api::{self, Album, Chapter, SearchPage};
use super::{ImageDecode, Platform, RequestSpec, host_matches, jm_id};

/// Hosts JM serves the APP API, the image CDN and the legacy HTML site from.
/// Images are fetched with an explicit platform, but keeping every host here
/// also covers URLs copied from the site.
const HOST_SUFFIXES: &[&str] = &[
    // APP API
    "cdnhjk.net",
    "cdngwc.cc",
    "cdngwc.net",
    "cdngwc.club",
    // Image CDN
    "jmapiproxy.com",
    "jmapiproxy1.cc",
    "jmapiproxy2.cc",
    "jmapinodeudzn.net",
    // HTML site
    "18comic.vip",
    "18comic.org",
    "18comic.me",
    "jmcomic.me",
    "jmcomic.org",
    "jmcomic.vip",
];

const PAGE_SIZE: u32 = 80;

pub(super) struct Jm {
    /// Index into `jm_api::API_DOMAINS` of the last host that answered. It
    /// keeps every request from paying a failed attempt on a blocked host.
    preferred: AtomicUsize,
}

impl Jm {
    pub(super) const fn new() -> Self {
        Self {
            preferred: AtomicUsize::new(0),
        }
    }

    /// Build one API request plus every other domain as a fallback, starting
    /// at the host that last worked.
    fn api_spec(&self, build: impl Fn(&str) -> Result<Url>) -> Result<RequestSpec> {
        let domains = jm_api::API_DOMAINS;
        let start = self.preferred.load(Ordering::Relaxed) % domains.len();
        let mut urls = Vec::with_capacity(domains.len());

        for offset in 0..domains.len() {
            urls.push(build(domains[(start + offset) % domains.len()])?);
        }

        let mut urls = urls.into_iter();
        let primary = urls
            .next()
            .ok_or_else(|| AppError::Unexpected("jm has no API domain".into()))?;

        Ok(RequestSpec::json(primary)
            .with_headers(jm_api::document_headers(jm_api::timestamp()))
            .with_fallbacks(urls.collect()))
    }
}

impl Platform for Jm {
    fn id(&self) -> &'static str {
        "jm"
    }

    /// Bumped when the profile moved from the HTML site to the APP API: same
    /// URL shape, but different headers and image hosts.
    fn cache_version(&self) -> u32 {
        3
    }

    fn matches(&self, url: &Url) -> bool {
        host_matches(url, HOST_SUFFIXES)
    }

    fn configure(&self, request: RequestBuilder, url: &Url) -> RequestBuilder {
        // Each API mirror should see its own origin as the referer; image
        // hosts always use the active API origin.
        let referer = match url.host_str() {
            Some(host) if jm_api::is_api_host(host) => format!("https://{host}"),
            _ => jm_api::referer(),
        };

        request
            .header(USER_AGENT, jm_api::APP_USER_AGENT)
            .header(REFERER, referer)
            .header(ACCEPT_LANGUAGE, jm_api::accept_language())
            .header("x-requested-with", jm_api::requested_with())
    }

    fn note_request_outcome(&self, url: &Url, success: bool) {
        let Some(position) = url.host_str().and_then(|host| {
            jm_api::API_DOMAINS
                .iter()
                .position(|domain| *domain == host)
        }) else {
            return;
        };

        if success {
            self.preferred.store(position, Ordering::Relaxed);
        } else if self.preferred.load(Ordering::Relaxed) == position {
            // Move on so the next request starts at the following mirror.
            self.preferred.store(
                (position + 1) % jm_api::API_DOMAINS.len(),
                Ordering::Relaxed,
            );
        }
    }

    fn image_fallbacks(&self, url: &Url) -> Vec<Url> {
        jm_api::image_alternates(url)
    }

    fn image_decode(&self, url: &Url) -> Option<ImageDecode> {
        jm_api::page_image(url)
            .map(|(photo_id, filename)| ImageDecode::JmScramble { photo_id, filename })
    }

    fn prepare<'a>(
        &'a self,
        client: &'a HttpClient,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if let Err(error) = ensure_app_version(client, self).await {
                // The request itself can still succeed with the built-in
                // version, so a failed sync must not block the command.
                logging::warn("jm", format!("failed to sync the jm app version: {error}"));
            }
        })
    }

    /// Photo and cover images are served from interchangeable CDNs and the
    /// HTML site appends a disposable query, so key on the path alone: one
    /// asset keeps one cache entry no matter which host answered.
    fn cache_key_url(&self, url: &Url) -> String {
        if url.path().starts_with("/media/") {
            return format!("jm{}", url.path());
        }

        let mut cache_url = url.clone();
        cache_url.set_query(None);
        cache_url.to_string()
    }

    fn search_request(&self, query: &str, page: u32) -> Result<RequestSpec> {
        self.api_spec(|domain| jm_api::search_url(domain, query, page))
    }

    fn parse_search(&self, _query: &str, _page: u32, body: &[u8]) -> Result<SearchResponse> {
        let Some(page) = jm_api::decode::<SearchPage>(body, jm_api::timestamp())? else {
            return Ok(SearchResponse {
                total: 0,
                page_size: PAGE_SIZE,
                result: Vec::new(),
            });
        };

        let mut result = Vec::with_capacity(page.content.len());
        for item in page.content {
            let Some(id) = item
                .id
                .map(|value| value.text())
                .filter(|id| !id.is_empty())
            else {
                continue;
            };
            let title = item
                .name
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| format!("JM{id}"));
            let extra = JmExtra {
                // The APP search payload has no tags; author and category are
                // the only metadata it exposes.
                tags: Vec::new(),
                author: non_empty(item.author),
                category: item
                    .category
                    .and_then(|category| non_empty(Some(category.title))),
            };
            let thumb = jm_api::cover_url(&id);

            result.push(SearchItem::new(id, title, thumb, SearchExtra::Jm(extra)));
        }

        let total = page
            .total
            .as_ref()
            .and_then(|value| value.as_u64())
            .unwrap_or(0);

        Ok(SearchResponse {
            total: total.max(result.len() as u64),
            page_size: PAGE_SIZE,
            result,
        })
    }

    fn detail_request(&self, id: &str) -> Result<RequestSpec> {
        let id = jm_id(id)?;
        self.api_spec(|domain| jm_api::album_url(domain, &id))
    }

    fn parse_detail(&self, id: &str, body: &[u8]) -> Result<ComicDetail> {
        let Some(album) = jm_api::decode::<Album>(body, jm_api::timestamp())? else {
            return Err(album_not_found(id));
        };

        let id = jm_id(id)?;
        let Album {
            name,
            author,
            tags,
            works,
            actors,
            series,
            total_photos,
            addtime,
            total_views,
            likes,
            comment_total,
        } = album;

        let title = name.trim().to_string();
        if title.is_empty() {
            return Err(album_not_found(&id));
        }

        let mut chapters = series
            .into_iter()
            .filter_map(|series| {
                let chapter_id = series
                    .id
                    .map(|value| value.text())
                    .filter(|value| !value.is_empty())?;

                Some(JmChapterRef {
                    id: chapter_id,
                    title: series.name.trim().to_string(),
                    sort: series
                        .sort
                        .as_ref()
                        .and_then(jm_api::Scalar::as_u64)
                        .unwrap_or(0) as u32,
                })
            })
            .collect::<Vec<_>>();
        // JM reports a multi-chapter album through `series`; a single-chapter
        // album has none, and its only chapter shares the album id.
        if chapters.is_empty() {
            chapters.push(JmChapterRef {
                id: id.clone(),
                title: title.clone(),
                sort: 1,
            });
        }
        chapters.sort_by_key(|chapter| chapter.sort);

        let extra = JmDetailExtra {
            authors: clean(author),
            tags: clean(tags),
            works: clean(works),
            actors: clean(actors),
            page_count: total_photos
                .and_then(|value| value.as_u64())
                .and_then(|value| u32::try_from(value).ok()),
            pub_date: addtime
                .and_then(|value| value.as_i64())
                .and_then(jm_api::format_date),
            // The APP payload has no update date; the HTML-only field is gone.
            update_date: None,
            likes: likes.as_ref().and_then(item_text),
            views: total_views.as_ref().and_then(item_text),
            comment_count: comment_total.and_then(|value| value.as_u64()),
            chapters,
        };

        Ok(ComicDetail::new(
            title,
            jm_api::cover_url(&id),
            DetailExtra::Jm(extra),
        ))
    }

    fn reader_chapter_request(&self, id: &str, chapter_id: Option<&str>) -> Result<RequestSpec> {
        let album = jm_id(id)?;
        // A single-chapter album's only chapter shares the album id, so a
        // deep link without a chapter still works for the common case.
        let chapter = match chapter_id {
            Some(chapter) => jm_id(chapter)?,
            None => album,
        };

        self.api_spec(|domain| jm_api::chapter_url(domain, &chapter))
    }

    fn parse_reader_chapter(
        &self,
        id: &str,
        chapter_id: Option<&str>,
        body: &[u8],
    ) -> Result<ReaderChapter> {
        let album = jm_id(id)?;
        let photo_id = match chapter_id {
            Some(chapter) => jm_id(chapter)?,
            None => album,
        };

        let Some(chapter) = jm_api::decode::<Chapter>(body, jm_api::timestamp())? else {
            return Err(AppError::NotFound {
                resource: format!("jm chapter `{photo_id}`"),
            });
        };

        let name = chapter.name.trim().to_string();
        let title = if name.is_empty() {
            format!("JM{photo_id}")
        } else {
            name.clone()
        };
        // `series_id` is `0` for a single-chapter album, where the photo id
        // doubles as the album id.
        let album_id = chapter
            .series_id
            .as_ref()
            .and_then(jm_api::Scalar::as_u64)
            .filter(|value| *value > 0)
            .map(|value| value.to_string())
            .unwrap_or_else(|| photo_id.clone());

        let mut series = chapter
            .series
            .into_iter()
            .filter_map(|item| {
                let id = item
                    .id
                    .map(|value| value.text())
                    .filter(|id| !id.is_empty())?;
                Some((
                    id,
                    item.name.trim().to_string(),
                    item.sort
                        .as_ref()
                        .and_then(jm_api::Scalar::as_u64)
                        .unwrap_or(0),
                ))
            })
            .collect::<Vec<_>>();
        series.sort_by_key(|(_, _, sort)| *sort);

        let position = series.iter().position(|(id, _, _)| *id == photo_id);
        let previous = position
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| series.get(index))
            .map(chapter_ref);
        let next = position
            .and_then(|index| series.get(index + 1))
            .map(chapter_ref);

        let pages = chapter
            .images
            .into_iter()
            .filter(|file| jm_api::is_safe_filename(file))
            .map(|file| ReaderPage {
                url: jm_api::page_url(&photo_id, &file),
                // JM does not expose page dimensions; the viewer measures
                // them once the image loads.
                width: None,
                height: None,
                local_path: None,
            })
            .collect::<Vec<_>>();
        if pages.is_empty() {
            return Err(AppError::NotFound {
                resource: format!("jm chapter `{photo_id}` pages"),
            });
        }

        Ok(ReaderChapter {
            title,
            thumb: jm_api::cover_url(&album_id),
            chapter_id: Some(photo_id),
            chapter_title: (!name.is_empty()).then_some(name),
            pages,
            previous,
            next,
        })
    }
}

/// Refresh the signed app version once per process. `/setting` is the same
/// call the APP makes on startup; a failure leaves the built-in version in
/// place and is retried by the next command.
static VERSION_SYNC: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();

async fn ensure_app_version(client: &HttpClient, platform: &dyn Platform) -> Result<()> {
    VERSION_SYNC
        .get_or_try_init(|| async { sync_app_version(client, platform).await })
        .await
        .map(|_| ())
}

async fn sync_app_version(client: &HttpClient, platform: &dyn Platform) -> Result<()> {
    let ts = jm_api::timestamp();
    let mut urls = jm_api::API_DOMAINS
        .iter()
        .map(|domain| jm_api::setting_url(domain));
    let primary = urls
        .next()
        .ok_or_else(|| AppError::Unexpected("jm has no API domain".into()))??;
    let fallbacks = urls.collect::<Result<Vec<_>>>()?;
    let spec = RequestSpec::json(primary)
        .with_headers(jm_api::document_headers(ts))
        .with_fallbacks(fallbacks);

    let body = http::fetch_document(client, platform, &spec).await?;
    let Some(setting) = jm_api::decode::<jm_api::Setting>(&body, ts)? else {
        return Ok(());
    };
    let Some(version) = setting
        .jm3_version
        .map(|version| version.trim().to_string())
        .filter(|version| !version.is_empty())
    else {
        return Ok(());
    };

    if jm_api::version_is_newer(&version, &jm_api::app_version()) {
        jm_api::set_app_version(&version);
        logging::info("jm", format!("using jm app version {version}"));
    }
    Ok(())
}

/// Look up the album's scramble threshold, caching it for the process. One
/// extra request per chapter is enough: pages in a chapter share the album.
pub(crate) async fn scramble_id(
    client: &HttpClient,
    platform: &dyn Platform,
    photo_id: &str,
) -> Result<i64> {
    if let Some(value) = SCRAMBLE_IDS
        .lock()
        .ok()
        .and_then(|cache| cache.get(photo_id).copied())
    {
        return Ok(value);
    }

    let ts = jm_api::timestamp();
    let mut urls = jm_api::API_DOMAINS
        .iter()
        .map(|domain| jm_api::scramble_url(domain, photo_id, ts));
    let primary = urls
        .next()
        .ok_or_else(|| AppError::Unexpected("jm has no API domain".into()))??;
    let fallbacks = urls.collect::<Result<Vec<_>>>()?;
    let spec = RequestSpec::html(primary)
        .with_headers(jm_api::scramble_headers(ts))
        .with_fallbacks(fallbacks);

    let body = http::fetch_document(client, platform, &spec).await?;
    let value = match jm_api::parse_scramble_id(&body) {
        Some(value) => value,
        None => {
            // Falling back keeps pages readable, but a missing variable means
            // the endpoint changed and should show up in the log.
            logging::warn(
                "jm",
                format!(
                    "scramble_id missing in a {}-byte response; using {}",
                    body.len(),
                    jm_api::DEFAULT_SCRAMBLE_ID
                ),
            );
            jm_api::DEFAULT_SCRAMBLE_ID
        }
    };

    if let Ok(mut cache) = SCRAMBLE_IDS.lock() {
        cache.insert(photo_id.to_string(), value);
    }
    Ok(value)
}

static SCRAMBLE_IDS: LazyLock<Mutex<HashMap<String, i64>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn chapter_ref((id, name, _): &(String, String, u64)) -> ChapterRef {
    ChapterRef {
        id: id.clone(),
        title: if name.is_empty() {
            id.clone()
        } else {
            name.clone()
        },
    }
}

fn item_text(value: &jm_api::Scalar) -> Option<String> {
    let text = value.text();
    (!text.is_empty()).then_some(text)
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn clean(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn album_not_found(id: &str) -> AppError {
    AppError::NotFound {
        resource: format!("jm album `{id}`"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;
    use crate::image::model::SearchPlatform;

    const SEARCH_JSON: &str = r#"
{
  "search_query": "test",
  "total": 47,
  "content": [
    {
      "id": 1480638,
      "author": "SGNF",
      "name": "TURING TEST",
      "image": "",
      "category": { "id": "1", "title": "同人" }
    },
    {
      "id": "350234",
      "author": "",
      "name": "   ",
      "category": null
    }
  ]
}
"#;

    const DETAIL_JSON: &str = r#"
{
  "id": 350234,
  "name": "董卓 上+下",
  "author": ["N/A", " 孫笑川 ", ""],
  "tags": ["中文", " "],
  "works": ["三國"],
  "actors": ["董卓"],
  "series": [
    { "id": "350235", "name": "下", "sort": "2" },
    { "id": "350234", "name": "", "sort": 1 }
  ],
  "total_photos": 50,
  "addtime": "1655254028",
  "total_views": "1765896",
  "likes": "93058",
  "comment_total": "9803"
}
"#;

    const CHAPTER_JSON: &str = r#"
{
  "id": 350235,
  "name": "董卓 下",
  "series_id": "400222",
  "series": [
    { "id": "350234", "name": "上", "sort": "1" },
    { "id": "350235", "name": "下", "sort": "2" },
    { "id": "350236", "name": "完", "sort": "3" }
  ],
  "images": ["00001.webp", "00002.jpg", "../evil.webp", ""]
}
"#;

    /// Encrypt a payload the way JM does, so parsers run through the real
    /// decryption path instead of a test-only shortcut.
    fn envelope(payload: &str) -> Vec<u8> {
        let encrypted = jm_api::encrypt_for_test(payload, jm_api::timestamp());
        format!(r#"{{"code":200,"data":"{encrypted}"}}"#).into_bytes()
    }

    #[test]
    fn builds_search_urls_with_mirrors() {
        let jm = Jm::new();
        let search = jm.search_request("test 1", 2).unwrap();
        let url = search.url.as_str();
        assert!(url.starts_with("https://www.cdnhjk.net/search?"));
        assert!(url.contains("search_query=test+1"));
        assert!(url.contains("page=2"));
        assert_eq!(
            search.headers.get("tokenparam").unwrap().to_str().unwrap(),
            format!("{},2.1.12", jm_api::timestamp())
        );

        // Every other APP domain is a fallback for the same request.
        let mirror_hosts = search
            .fallbacks
            .iter()
            .map(|url| url.host_str().unwrap().to_string())
            .collect::<Vec<_>>();
        assert_eq!(mirror_hosts.len(), jm_api::API_DOMAINS.len() - 1);
        assert!(!mirror_hosts.contains(&"www.cdnhjk.net".to_string()));

        let detail = jm.detail_request("JM123").unwrap();
        assert_eq!(detail.url.as_str(), "https://www.cdnhjk.net/album?id=123");
        assert!(jm.detail_request("123/456").is_err());
    }

    #[test]
    fn remembers_the_mirror_that_worked() {
        let jm = Jm::new();
        let working =
            Url::parse(&format!("https://{}/album?id=1", jm_api::API_DOMAINS[2])).unwrap();

        jm.note_request_outcome(&working, true);
        let detail = jm.detail_request("1").unwrap();
        assert_eq!(detail.url.host_str(), Some(jm_api::API_DOMAINS[2]));
        // The remaining mirrors wrap around, so none is skipped.
        let hosts = detail
            .fallbacks
            .iter()
            .map(|url| url.host_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(hosts.len(), jm_api::API_DOMAINS.len() - 1);
        assert_eq!(hosts[0], jm_api::API_DOMAINS[3]);
        assert_eq!(hosts[1], jm_api::API_DOMAINS[0]);
    }

    #[test]
    fn a_failed_primary_moves_the_starting_mirror() {
        let jm = Jm::new();
        let broken = Url::parse(&format!("https://{}/album?id=1", jm_api::API_DOMAINS[0])).unwrap();

        jm.note_request_outcome(&broken, false);
        let detail = jm.detail_request("1").unwrap();
        assert_eq!(detail.url.host_str(), Some(jm_api::API_DOMAINS[1]));
    }

    #[test]
    fn image_fallbacks_move_the_same_path_to_another_cdn() {
        let jm = Jm::new();
        let url = Url::parse("https://cdn-msp.jmapiproxy1.cc/media/albums/350234_3x4.jpg").unwrap();
        let alternates = jm.image_fallbacks(&url);

        assert_eq!(alternates.len(), 5);
        assert!(
            alternates
                .iter()
                .all(|candidate| candidate.path() == url.path())
        );
        assert!(
            alternates
                .iter()
                .all(|candidate| candidate.host_str() != url.host_str())
        );
        assert!(
            jm.image_fallbacks(&Url::parse("https://example.com/a.jpg").unwrap())
                .is_empty()
        );
    }

    #[test]
    fn parses_search_results() {
        let response = Jm::new()
            .parse_search("test", 1, &envelope(SEARCH_JSON))
            .unwrap();
        assert_eq!(response.total, 47);
        assert_eq!(response.page_size, 80);
        assert_eq!(response.result.len(), 2);

        let first = &response.result[0];
        assert_eq!(first.platform, SearchPlatform::Jm);
        assert_eq!(first.id, "1480638");
        assert_eq!(first.title, "TURING TEST");
        assert_eq!(
            first.thumb,
            "https://cdn-msp.jmapiproxy1.cc/media/albums/1480638_3x4.jpg"
        );

        let SearchExtra::Jm(extra) = &first.extra else {
            panic!("expected jm search extra");
        };
        assert_eq!(extra.author.as_deref(), Some("SGNF"));
        assert_eq!(extra.category.as_deref(), Some("同人"));
        assert!(extra.tags.is_empty());

        let second = &response.result[1];
        assert_eq!(second.id, "350234");
        assert_eq!(second.title, "JM350234");
        let SearchExtra::Jm(extra) = &second.extra else {
            panic!("expected jm search extra");
        };
        assert!(extra.author.is_none());
        assert!(extra.category.is_none());
    }

    #[test]
    fn parses_detail() {
        let detail = Jm::new()
            .parse_detail("350234", &envelope(DETAIL_JSON))
            .unwrap();
        assert_eq!(detail.platform, SearchPlatform::Jm);
        assert_eq!(detail.title, "董卓 上+下");
        assert_eq!(
            detail.thumb,
            "https://cdn-msp.jmapiproxy1.cc/media/albums/350234_3x4.jpg"
        );

        let DetailExtra::Jm(extra) = detail.extra else {
            panic!("expected jm detail extra");
        };
        assert_eq!(extra.authors, ["N/A", "孫笑川"]);
        assert_eq!(extra.tags, ["中文"]);
        assert_eq!(extra.works, ["三國"]);
        assert_eq!(extra.actors, ["董卓"]);
        assert_eq!(extra.page_count, Some(50));
        assert_eq!(extra.pub_date.as_deref(), Some("2022-06-15"));
        assert_eq!(extra.update_date, None);
        assert_eq!(extra.likes.as_deref(), Some("93058"));
        assert_eq!(extra.views.as_deref(), Some("1765896"));
        assert_eq!(extra.comment_count, Some(9803));
        assert_eq!(extra.chapters.len(), 2);
        assert_eq!(extra.chapters[0].id, "350234");
        assert_eq!(extra.chapters[0].sort, 1);
        assert_eq!(extra.chapters[1].id, "350235");
        assert_eq!(extra.chapters[1].title, "下");
    }

    #[test]
    fn single_chapter_album_synthesizes_one_chapter() {
        let body =
            envelope(r#"{"id": 1480638, "name": "TURING TEST", "total_photos": 22, "series": []}"#);
        let detail = Jm::new().parse_detail("1480638", &body).unwrap();
        let DetailExtra::Jm(extra) = detail.extra else {
            panic!("expected jm detail extra");
        };
        assert_eq!(extra.chapters.len(), 1);
        assert_eq!(extra.chapters[0].id, "1480638");
        assert_eq!(extra.chapters[0].title, "TURING TEST");
        assert_eq!(extra.chapters[0].sort, 1);
        assert_eq!(extra.page_count, Some(22));
        assert_eq!(extra.pub_date, None);
    }

    #[test]
    fn missing_album_is_not_found() {
        let error = Jm::new()
            .parse_detail("999999999", br#"{"code":200,"data":[]}"#)
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::NotFound);

        let error = Jm::new()
            .parse_detail("350234", &envelope(r#"{"id":350234,"name":"  "}"#))
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::NotFound);
    }

    #[test]
    fn empty_search_is_an_empty_page() {
        let response = Jm::new()
            .parse_search("nothing", 1, br#"{"code":200,"data":[]}"#)
            .unwrap();
        assert_eq!(response.total, 0);
        assert!(response.result.is_empty());
    }

    #[test]
    fn builds_reader_requests() {
        let jm = Jm::new();
        let explicit = jm.reader_chapter_request("350234", Some("350235")).unwrap();
        assert_eq!(
            explicit.url.as_str(),
            "https://www.cdnhjk.net/chapter?id=350235"
        );

        // A single-chapter album reuses the album id as the chapter id.
        let implicit = jm.reader_chapter_request("350234", None).unwrap();
        assert_eq!(
            implicit.url.as_str(),
            "https://www.cdnhjk.net/chapter?id=350234"
        );
        assert!(jm.reader_chapter_request("350234", Some("../1")).is_err());
    }

    #[test]
    fn parses_a_reader_chapter() {
        let chapter = Jm::new()
            .parse_reader_chapter("400222", Some("350235"), &envelope(CHAPTER_JSON))
            .unwrap();

        assert_eq!(chapter.title, "董卓 下");
        assert_eq!(chapter.chapter_title.as_deref(), Some("董卓 下"));
        assert_eq!(chapter.chapter_id.as_deref(), Some("350235"));
        assert_eq!(
            chapter.thumb,
            "https://cdn-msp.jmapiproxy1.cc/media/albums/400222_3x4.jpg"
        );
        assert_eq!(
            chapter.previous.as_ref().map(|item| item.id.as_str()),
            Some("350234")
        );
        assert_eq!(
            chapter.previous.as_ref().map(|item| item.title.as_str()),
            Some("上")
        );
        assert_eq!(
            chapter.next.as_ref().map(|item| item.id.as_str()),
            Some("350236")
        );
        // Unsafe and empty file names are dropped.
        assert_eq!(chapter.pages.len(), 2);
        assert_eq!(
            chapter.pages[0].url,
            "https://cdn-msp.jmapiproxy1.cc/media/photos/350235/00001.webp"
        );
        assert_eq!(
            chapter.pages[1].url,
            "https://cdn-msp.jmapiproxy1.cc/media/photos/350235/00002.jpg"
        );
        assert!(chapter.pages[0].width.is_none());
    }

    #[test]
    fn single_chapter_reader_uses_the_photo_as_the_album() {
        let body = envelope(
            r#"{"id": 1480638, "name": "", "series_id": "0", "series": [], "images": ["00001.webp"]}"#,
        );
        let chapter = Jm::new()
            .parse_reader_chapter("1480638", None, &body)
            .unwrap();

        assert_eq!(chapter.title, "JM1480638");
        assert!(chapter.chapter_title.is_none());
        assert_eq!(
            chapter.thumb,
            "https://cdn-msp.jmapiproxy1.cc/media/albums/1480638_3x4.jpg"
        );
        assert!(chapter.previous.is_none());
        assert!(chapter.next.is_none());
    }

    #[test]
    fn a_chapter_without_pages_is_not_found() {
        let body = envelope(r#"{"id": 1, "name": "Empty", "images": []}"#);
        let error = Jm::new()
            .parse_reader_chapter("1", None, &body)
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::NotFound);

        let error = Jm::new()
            .parse_reader_chapter("999999999", None, br#"{"code":200,"data":[]}"#)
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::NotFound);
    }

    #[test]
    fn page_images_are_marked_for_descrambling() {
        let jm = Jm::new();
        let page =
            Url::parse("https://cdn-msp.jmapiproxy1.cc/media/photos/350235/00001.webp").unwrap();
        let Some(ImageDecode::JmScramble { photo_id, filename }) = jm.image_decode(&page) else {
            panic!("expected a scramble recipe");
        };
        assert_eq!(photo_id, "350235");
        assert_eq!(filename, "00001");

        let cover =
            Url::parse("https://cdn-msp.jmapiproxy1.cc/media/albums/350234_3x4.jpg").unwrap();
        assert!(jm.image_decode(&cover).is_none());
    }

    #[test]
    fn cache_key_ignores_cdn_host_and_query() {
        let signed = Url::parse(
            "https://cdn-msp3.jmapiproxy2.cc/media/photos/350235/00001.webp?v=1793017177",
        )
        .unwrap();
        assert_eq!(
            Jm::new().cache_key_url(&signed),
            "jm/media/photos/350235/00001.webp"
        );

        let cover =
            Url::parse("https://cdn-msp.jmapiproxy1.cc/media/albums/350234_3x4.jpg").unwrap();
        assert_eq!(
            Jm::new().cache_key_url(&cover),
            "jm/media/albums/350234_3x4.jpg"
        );

        let other = Url::parse("https://www.cdnhjk.net/a.jpg?v=1").unwrap();
        assert_eq!(
            Jm::new().cache_key_url(&other),
            "https://www.cdnhjk.net/a.jpg"
        );
    }
}
