use reqwest::RequestBuilder;
use reqwest::header::{ACCEPT_LANGUAGE, REFERER};
use serde::Deserialize;
use url::Url;

use crate::error::{AppError, Result};
use crate::image::model::{
    ComicDetail, DetailExtra, NhentaiDetailExtra, NhentaiSearchExtra, NhentaiTag, ReaderChapter,
    ReaderPage, SearchExtra, SearchItem, SearchResponse,
};

use super::{Catalog, ImageProfile, RequestSpec, host_matches, numeric_id};

const BASE_URL: &str = "https://nhentai.net";
const HOST_SUFFIXES: &[&str] = &["nhentai.net"];
const REFERER_VALUE: &str = "https://nhentai.net/";
const ACCEPT_LANGUAGE_VALUE: &str = "en-US,en;q=0.9";
const FALLBACK_PAGE_SIZE: u32 = 25;

pub(super) struct Nhentai;

impl ImageProfile for Nhentai {
    fn id(&self) -> &'static str {
        "nhentai"
    }

    fn matches(&self, url: &Url) -> bool {
        host_matches(url, HOST_SUFFIXES)
    }

    fn configure(&self, request: RequestBuilder, _url: &Url) -> RequestBuilder {
        request
            .header(REFERER, REFERER_VALUE)
            .header(ACCEPT_LANGUAGE, ACCEPT_LANGUAGE_VALUE)
    }
}

impl Catalog for Nhentai {
    fn search_request(&self, query: &str, page: u32) -> Result<RequestSpec> {
        let mut url = Url::parse(&format!("{BASE_URL}/api/v2/search")).map_err(|error| {
            crate::error::AppError::Unexpected(format!("invalid nhentai URL: {error}"))
        })?;
        url.query_pairs_mut()
            .append_pair("query", query)
            .append_pair("page", &page.to_string());
        Ok(RequestSpec::json(url))
    }

    fn parse_search(&self, _query: &str, _page: u32, body: &[u8]) -> Result<SearchResponse> {
        let response: ApiSearch = serde_json::from_slice(body)?;
        let result = response
            .result
            .into_iter()
            .map(|item| -> Result<SearchItem> {
                let japanese_title = non_empty(item.japanese_title);
                let title = non_empty(item.english_title)
                    .or_else(|| japanese_title.clone())
                    .unwrap_or_else(|| format!("#{}", item.id));
                let thumb = item
                    .thumbnail
                    .map(|path| thumbnail_url(&path))
                    .unwrap_or_default();
                let extra = NhentaiSearchExtra {
                    media_id: item.media_id,
                    japanese_title,
                    thumbnail_width: item.thumbnail_width,
                    thumbnail_height: item.thumbnail_height,
                    num_pages: item.num_pages,
                    num_favorites: item.num_favorites,
                    tag_ids: item.tag_ids,
                    blacklisted: item.blacklisted,
                };

                Ok(SearchItem::new(
                    item.id.to_string(),
                    title,
                    thumb,
                    SearchExtra::Nhentai(extra),
                ))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(SearchResponse {
            total: response.total,
            page_size: if response.per_page == 0 {
                FALLBACK_PAGE_SIZE
            } else {
                response.per_page
            },
            result,
        })
    }

    fn detail_request(&self, id: &str) -> Result<RequestSpec> {
        let id = numeric_id(id, "nhentai")?;
        let url = Url::parse(&format!("{BASE_URL}/api/v2/galleries/{id}")).map_err(|error| {
            crate::error::AppError::Unexpected(format!("invalid nhentai URL: {error}"))
        })?;
        Ok(RequestSpec::json(url))
    }

    fn parse_detail(&self, id: &str, body: &[u8]) -> Result<ComicDetail> {
        Ok(parse_gallery(id, body)?.0)
    }

    fn reader_chapter_request(&self, id: &str, _chapter_id: Option<&str>) -> Result<RequestSpec> {
        // A gallery is one implicit chapter, so the detail endpoint already
        // contains the complete page list.
        self.detail_request(id)
    }

    fn parse_reader_chapter(
        &self,
        id: &str,
        _chapter_id: Option<&str>,
        body: &[u8],
    ) -> Result<ReaderChapter> {
        let (detail, pages) = parse_gallery(id, body)?;
        if pages.is_empty() {
            return Err(AppError::NotFound {
                resource: format!("nhentai gallery `{id}` pages"),
            });
        }

        Ok(ReaderChapter {
            title: detail.title,
            thumb: detail.thumb,
            chapter_id: None,
            chapter_title: None,
            pages,
            previous: None,
            next: None,
        })
    }
}

fn parse_gallery(id: &str, body: &[u8]) -> Result<(ComicDetail, Vec<ReaderPage>)> {
    let response: ApiDetail = serde_json::from_slice(body)?;
    let ApiDetail {
        media_id,
        title,
        cover,
        thumbnail,
        scanlator,
        upload_date,
        tags,
        num_pages,
        num_favorites,
        pages,
    } = response;

    let japanese_title = non_empty(title.japanese);
    let pretty_title = non_empty(title.pretty);
    let display_title = non_empty(title.english)
        .or_else(|| pretty_title.clone())
        .or_else(|| japanese_title.clone())
        .unwrap_or_else(|| format!("#{id}"));
    let thumb = thumbnail
        .as_ref()
        .map(|image| thumbnail_url(&image.path))
        .unwrap_or_default();
    let cover_path = cover.as_ref().map(|image| cover_url(&image.path));
    let cover_width = cover.as_ref().and_then(|image| image.width);
    let cover_height = cover.as_ref().and_then(|image| image.height);

    let extra = NhentaiDetailExtra {
        media_id,
        japanese_title,
        pretty_title,
        num_pages,
        num_favorites,
        scanlator: non_empty(scanlator),
        upload_date,
        cover: cover_path,
        cover_width,
        cover_height,
        tags,
    };

    let pages = pages
        .into_iter()
        .filter(|page| !page.path.trim().is_empty())
        .map(|page| ReaderPage {
            url: page_url(&page.path),
            width: page.width,
            height: page.height,
            local_path: None,
        })
        .collect();

    Ok((
        ComicDetail::new(display_title, thumb, DetailExtra::Nhentai(extra)),
        pages,
    ))
}

#[derive(Deserialize)]
struct ApiSearch {
    result: Vec<ApiSearchItem>,
    per_page: u32,
    total: u64,
}

#[derive(Deserialize)]
struct ApiSearchItem {
    id: u64,
    media_id: String,
    #[serde(default)]
    english_title: Option<String>,
    #[serde(default)]
    japanese_title: Option<String>,
    #[serde(default)]
    thumbnail: Option<String>,
    #[serde(default)]
    thumbnail_width: Option<u32>,
    #[serde(default)]
    thumbnail_height: Option<u32>,
    #[serde(default)]
    num_pages: Option<u32>,
    #[serde(default)]
    num_favorites: Option<u32>,
    #[serde(default)]
    tag_ids: Vec<u64>,
    #[serde(default)]
    blacklisted: Option<bool>,
}

#[derive(Deserialize)]
struct ApiDetail {
    media_id: String,
    title: ApiTitle,
    #[serde(default)]
    cover: Option<ApiImage>,
    #[serde(default)]
    thumbnail: Option<ApiImage>,
    #[serde(default)]
    scanlator: Option<String>,
    #[serde(default)]
    upload_date: Option<u64>,
    #[serde(default)]
    tags: Vec<NhentaiTag>,
    #[serde(default)]
    num_pages: Option<u32>,
    #[serde(default)]
    num_favorites: Option<u32>,
    #[serde(default)]
    pages: Vec<ApiPage>,
}

#[derive(Deserialize)]
struct ApiTitle {
    #[serde(default)]
    english: Option<String>,
    #[serde(default)]
    japanese: Option<String>,
    #[serde(default)]
    pretty: Option<String>,
}

#[derive(Deserialize)]
struct ApiImage {
    path: String,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
}

#[derive(Deserialize)]
struct ApiPage {
    path: String,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn thumbnail_url(path: &str) -> String {
    format!("https://t.nhentai.net/{}", path.trim_start_matches('/'))
}

fn cover_url(path: &str) -> String {
    format!("https://i.nhentai.net/{}", path.trim_start_matches('/'))
}

fn page_url(path: &str) -> String {
    format!("https://i.nhentai.net/{}", path.trim_start_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::model::SearchPlatform;

    const SEARCH_JSON: &str = r#"
{
  "result": [
    {
      "id": 42,
      "media_id": "100",
      "english_title": "Example Gallery",
      "japanese_title": "サンプル",
      "thumbnail": "galleries/100/thumb.webp",
      "thumbnail_width": 250,
      "thumbnail_height": 350,
      "num_pages": 20,
      "num_favorites": 5,
      "tag_ids": [1, 2],
      "blacklisted": false
    }
  ],
  "num_pages": 4,
  "per_page": 25,
  "total": 100
}
"#;

    const DETAIL_JSON: &str = r#"
{
  "id": 42,
  "media_id": "100",
  "title": {
    "english": "Example Gallery",
    "japanese": "サンプル",
    "pretty": "Example"
  },
  "cover": { "path": "galleries/100/cover.webp", "width": 350, "height": 500 },
  "thumbnail": { "path": "galleries/100/thumb.webp", "width": 250, "height": 350 },
  "scanlator": "Example Scans",
  "upload_date": 1700000000,
  "tags": [
    { "id": 1, "type": "tag", "name": "example", "slug": "example", "count": 10 }
  ],
  "num_pages": 20,
  "num_favorites": 5,
  "pages": [
    { "number": 1, "path": "galleries/100/1.jpg", "width": 1280, "height": 1800 },
    { "number": 2, "path": "galleries/100/2.png", "width": 1280, "height": 1800 }
  ]
}
"#;

    #[test]
    fn builds_search_and_detail_urls() {
        let search = Nhentai.search_request("test query", 3).unwrap();
        let url = search.url.as_str();
        assert!(url.starts_with("https://nhentai.net/api/v2/search?"));
        assert!(url.contains("query=test+query"));
        assert!(url.contains("page=3"));

        let detail = Nhentai.detail_request("42").unwrap();
        assert_eq!(
            detail.url.as_str(),
            "https://nhentai.net/api/v2/galleries/42"
        );
        assert!(Nhentai.detail_request("42/extra").is_err());
    }

    #[test]
    fn parses_search_results() {
        let response = Nhentai
            .parse_search("test", 1, SEARCH_JSON.as_bytes())
            .unwrap();
        assert_eq!(response.total, 100);
        assert_eq!(response.page_size, 25);
        assert_eq!(response.result.len(), 1);

        let first = &response.result[0];
        assert_eq!(first.platform, SearchPlatform::Nhentai);
        assert_eq!(first.id, "42");
        assert_eq!(first.title, "Example Gallery");
        assert_eq!(
            first.thumb,
            "https://t.nhentai.net/galleries/100/thumb.webp"
        );

        let SearchExtra::Nhentai(extra) = &first.extra else {
            panic!("expected nhentai search extra");
        };
        assert_eq!(extra.media_id, "100");
        assert_eq!(extra.japanese_title.as_deref(), Some("サンプル"));
        assert_eq!(extra.tag_ids, [1, 2]);
    }

    #[test]
    fn parses_detail() {
        let detail = Nhentai.parse_detail("42", DETAIL_JSON.as_bytes()).unwrap();
        assert_eq!(detail.platform, SearchPlatform::Nhentai);
        assert_eq!(detail.title, "Example Gallery");
        assert_eq!(
            detail.thumb,
            "https://t.nhentai.net/galleries/100/thumb.webp"
        );

        let DetailExtra::Nhentai(extra) = detail.extra else {
            panic!("expected nhentai detail extra");
        };
        assert_eq!(
            extra.cover.as_deref(),
            Some("https://i.nhentai.net/galleries/100/cover.webp")
        );
        assert_eq!(extra.cover_width, Some(350));
        assert_eq!(extra.cover_height, Some(500));
        assert_eq!(extra.scanlator.as_deref(), Some("Example Scans"));
        assert_eq!(extra.tags.len(), 1);
        assert_eq!(extra.tags[0].kind, "tag");
        assert_eq!(extra.tags[0].name, "example");
        assert_eq!(extra.tags[0].count, Some(10));
    }

    #[test]
    fn parses_reader_pages() {
        let chapter = Nhentai
            .parse_reader_chapter("42", None, DETAIL_JSON.as_bytes())
            .unwrap();

        assert_eq!(chapter.title, "Example Gallery");
        assert_eq!(
            chapter.thumb,
            "https://t.nhentai.net/galleries/100/thumb.webp"
        );
        assert!(chapter.chapter_id.is_none());
        assert!(chapter.chapter_title.is_none());
        assert_eq!(chapter.pages.len(), 2);
        assert_eq!(
            chapter.pages[0].url,
            "https://i.nhentai.net/galleries/100/1.jpg"
        );
        assert_eq!(chapter.pages[0].width, Some(1280));
        assert_eq!(chapter.pages[0].height, Some(1800));
        assert_eq!(
            chapter.pages[1].url,
            "https://i.nhentai.net/galleries/100/2.png"
        );
    }

    #[test]
    fn reader_rejects_a_gallery_without_pages() {
        let body = r#"{
            "media_id": "100",
            "title": { "english": "Example Gallery" },
            "pages": []
        }"#;
        let error = Nhentai
            .parse_reader_chapter("42", None, body.as_bytes())
            .unwrap_err();
        assert_eq!(error.code(), crate::error::ErrorCode::NotFound);
    }
}
