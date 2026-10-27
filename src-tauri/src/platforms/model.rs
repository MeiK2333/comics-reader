//! Wire types for platform search and comic detail commands.
//!
//! `platform` identifies the producer and `extra` carries only that platform's
//! display fields. Each platform serializes its own typed struct into `extra`,
//! so the shared model never needs a new enum variant when a platform is added.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, Result};

/// Paginated search response. `extra` on each item is platform-specific.
#[derive(Debug, Serialize)]
pub(crate) struct SearchResponse {
    pub total: u64,
    pub page_size: u32,
    pub result: Vec<SearchItem>,
}

/// One search result. `platform` is the discriminant the frontend narrows on.
#[derive(Debug, Serialize)]
pub(crate) struct SearchItem {
    pub id: String,
    pub title: String,
    pub thumb: String,
    pub platform: String,
    pub extra: Value,
}

impl SearchItem {
    pub(crate) fn new(
        platform: &'static str,
        id: impl Into<String>,
        title: impl Into<String>,
        thumb: impl Into<String>,
        extra: impl Serialize,
    ) -> Result<Self> {
        Ok(Self {
            id: id.into(),
            title: title.into(),
            thumb: thumb.into(),
            platform: platform.to_owned(),
            extra: extra_value(extra)?,
        })
    }
}

/// Detail response. `chapters` is the cross-platform reading order; platforms
/// with one implicit chapter return a single chapter with no id.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ComicDetail {
    pub title: String,
    pub thumb: String,
    pub platform: String,
    pub chapters: Vec<DetailChapter>,
    pub extra: Value,
}

impl ComicDetail {
    pub(crate) fn new(
        platform: &'static str,
        title: impl Into<String>,
        thumb: impl Into<String>,
        chapters: Vec<DetailChapter>,
        extra: impl Serialize,
    ) -> Result<Self> {
        Ok(Self {
            title: title.into(),
            thumb: thumb.into(),
            platform: platform.to_owned(),
            chapters,
            extra: extra_value(extra)?,
        })
    }
}

/// One chapter in a work's reading order. `id` is `None` for single-chapter
/// galleries; `title` is `None` when the upstream page does not label it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DetailChapter {
    pub id: Option<String>,
    pub title: Option<String>,
}

fn extra_value(extra: impl Serialize) -> Result<Value> {
    serde_json::to_value(extra).map_err(|error| {
        AppError::Unexpected(format!("failed to serialize platform fields: {error}"))
    })
}

/// One chapter's page list. Single-work platforms leave `chapter_id` and the
/// navigation refs empty; chaptered platforms fill them in later.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReaderChapter {
    /// Display title of the work, used for the native window title.
    pub title: String,
    /// Cover thumbnail, reused by the reading-history list.
    pub thumb: String,
    pub chapter_id: Option<String>,
    pub chapter_title: Option<String>,
    pub pages: Vec<ReaderPage>,
    pub previous: Option<ChapterRef>,
    pub next: Option<ChapterRef>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReaderPage {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// Absolute path of a downloaded page. When set, the reader loads the
    /// local file through the asset protocol instead of fetching `url`.
    pub local_path: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChapterRef {
    pub id: String,
    pub title: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct TestExtra {
        tags: Vec<&'static str>,
        author: Option<&'static str>,
    }

    fn extra() -> TestExtra {
        TestExtra {
            tags: vec!["tag-a", "tag-b"],
            author: Some("Author"),
        }
    }

    #[test]
    fn search_response_uses_page_size_on_the_wire() {
        let response = SearchResponse {
            total: 1,
            page_size: 25,
            result: Vec::new(),
        };

        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value["page_size"], 25);
        assert!(value.get("pageSize").is_none());
    }

    #[test]
    fn search_item_serializes_platform_and_typed_extra() {
        let item =
            SearchItem::new("jm", "123", "Example", "https://cdn.example/1.jpg", extra()).unwrap();

        let value = serde_json::to_value(item).unwrap();
        assert_eq!(value["platform"], "jm");
        assert_eq!(value["extra"]["tags"], json!(["tag-a", "tag-b"]));
        // The discriminant lives on the item, not inside the platform fields.
        assert!(value["extra"].get("platform").is_none());
    }

    #[test]
    fn comic_detail_serializes_core_chapters_and_typed_extra() {
        let detail = ComicDetail::new(
            "jm",
            "Example",
            "https://cdn.example/1.jpg",
            vec![DetailChapter {
                id: Some("1".into()),
                title: Some("Chapter 1".into()),
            }],
            extra(),
        )
        .unwrap();

        let value = serde_json::to_value(detail).unwrap();
        assert_eq!(value["platform"], "jm");
        assert_eq!(value["title"], "Example");
        assert_eq!(value["chapters"][0]["id"], "1");
        assert_eq!(value["chapters"][0]["title"], "Chapter 1");
        assert_eq!(value["extra"]["author"], "Author");
        assert_eq!(value["extra"]["tags"], json!(["tag-a", "tag-b"]));
        // The discriminant is a sibling of `extra`, not a duplicate inside it.
        assert!(value["extra"].get("platform").is_none());
    }

    #[test]
    fn reader_chapter_matches_the_shared_wire_contract() {
        let chapter = ReaderChapter {
            title: "Example Gallery".into(),
            thumb: "https://t.nhentai.net/galleries/1/cover.jpg".into(),
            chapter_id: Some("12".into()),
            chapter_title: Some("Chapter 12".into()),
            pages: vec![
                ReaderPage {
                    url: "https://example.com/pages/1.jpg".into(),
                    width: Some(800),
                    height: Some(1200),
                    local_path: None,
                },
                ReaderPage {
                    url: "https://example.com/pages/2.jpg".into(),
                    width: None,
                    height: None,
                    local_path: Some("/tmp/downloads/generic/1/12/2.jpg".into()),
                },
            ],
            previous: Some(ChapterRef {
                id: "11".into(),
                title: "Chapter 11".into(),
            }),
            next: None,
        };
        let actual = serde_json::to_value(&chapter).unwrap();
        let expected: serde_json::Value =
            serde_json::from_str(include_str!("../../../contracts/reader-chapter.json")).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn reader_chapter_uses_camel_case_on_the_wire() {
        let chapter = ReaderChapter {
            title: "Example".into(),
            thumb: "https://cdn.example/cover.jpg".into(),
            chapter_id: Some("12".into()),
            chapter_title: Some("Chapter 12".into()),
            pages: vec![ReaderPage {
                url: "https://cdn.example/1.jpg".into(),
                width: Some(800),
                height: Some(1200),
                local_path: None,
            }],
            previous: Some(ChapterRef {
                id: "11".into(),
                title: "Chapter 11".into(),
            }),
            next: None,
        };

        let value = serde_json::to_value(chapter).unwrap();
        assert_eq!(value["title"], "Example");
        assert_eq!(value["thumb"], "https://cdn.example/cover.jpg");
        assert_eq!(value["chapterId"], "12");
        assert_eq!(value["chapterTitle"], "Chapter 12");
        assert_eq!(value["pages"][0]["width"], 800);
        assert_eq!(value["pages"][0]["localPath"], serde_json::Value::Null);
        assert_eq!(value["previous"]["id"], "11");
        assert!(value["next"].is_null());
        assert!(value.get("chapter_id").is_none());
    }
}
