//! Wire types for platform search and comic detail commands.

use serde::{Deserialize, Serialize};

/// Paginated search response. `extra` on each item is platform-specific.
#[derive(Debug, Serialize)]
pub(crate) struct SearchResponse {
    pub total: u64,
    pub page_size: u32,
    pub result: Vec<SearchItem>,
}

/// One search result. `platform` is the discriminant the frontend narrows on,
/// so each platform can render its own fields without guessing.
#[derive(Debug, Serialize)]
pub(crate) struct SearchItem {
    pub id: String,
    pub title: String,
    pub thumb: String,
    pub platform: SearchPlatform,
    pub extra: SearchExtra,
}

impl SearchItem {
    /// Derive the wire platform from `extra` so the two can never disagree.
    pub(crate) fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        thumb: impl Into<String>,
        extra: SearchExtra,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            thumb: thumb.into(),
            platform: extra.platform(),
            extra,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum SearchPlatform {
    Manhuagui,
    Jm,
    Nhentai,
}

/// Platform-specific search fields. Serialized untagged, so the item payload
/// keeps its `{ id, title, thumb, platform, extra: {...} }` shape.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum SearchExtra {
    Manhuagui(ManhuaguiExtra),
    Jm(JmExtra),
    Nhentai(NhentaiSearchExtra),
}

impl SearchExtra {
    fn platform(&self) -> SearchPlatform {
        match self {
            Self::Manhuagui(_) => SearchPlatform::Manhuagui,
            Self::Jm(_) => SearchPlatform::Jm,
            Self::Nhentai(_) => SearchPlatform::Nhentai,
        }
    }
}

/// Fields both search and detail return for manhuagui.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManhuaguiExtra {
    pub original_title: Option<String>,
    pub authors: Vec<String>,
    pub status: Option<String>,
    pub latest_chapter: Option<String>,
    pub updated_at: Option<String>,
    pub year: Option<String>,
    pub region: Option<String>,
    pub categories: Vec<String>,
    pub aliases: Vec<String>,
    pub description: Option<String>,
    pub score: Option<f32>,
    pub rating_count: Option<u64>,
}

/// Search results only expose tags; the rest of the album metadata needs the
/// detail request.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JmExtra {
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NhentaiSearchExtra {
    pub media_id: String,
    pub japanese_title: Option<String>,
    pub thumbnail_width: Option<u32>,
    pub thumbnail_height: Option<u32>,
    pub num_pages: Option<u32>,
    pub num_favorites: Option<u32>,
    pub tag_ids: Vec<u64>,
    pub blacklisted: Option<bool>,
}

/// Detail response. `platform` is the discriminant the frontend narrows on;
/// `extra` carries only that platform's detail fields.
#[derive(Debug, Serialize)]
pub(crate) struct ComicDetail {
    pub title: String,
    pub thumb: String,
    pub platform: SearchPlatform,
    pub extra: DetailExtra,
}

impl ComicDetail {
    /// Derive the wire platform from `extra` so the two can never disagree.
    pub(crate) fn new(
        title: impl Into<String>,
        thumb: impl Into<String>,
        extra: DetailExtra,
    ) -> Self {
        Self {
            title: title.into(),
            thumb: thumb.into(),
            platform: extra.platform(),
            extra,
        }
    }
}

/// Platform-specific detail fields. Serialized untagged, so the detail payload
/// keeps its `{ platform, title, thumb, extra: {...} }` shape.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub(crate) enum DetailExtra {
    Manhuagui(ManhuaguiExtra),
    Jm(JmDetailExtra),
    Nhentai(NhentaiDetailExtra),
}

impl DetailExtra {
    fn platform(&self) -> SearchPlatform {
        match self {
            Self::Manhuagui(_) => SearchPlatform::Manhuagui,
            Self::Jm(_) => SearchPlatform::Jm,
            Self::Nhentai(_) => SearchPlatform::Nhentai,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct JmDetailExtra {
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub works: Vec<String>,
    pub actors: Vec<String>,
    pub page_count: Option<u32>,
    pub pub_date: Option<String>,
    pub update_date: Option<String>,
    pub likes: Option<String>,
    pub views: Option<String>,
    pub comment_count: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct NhentaiTag {
    pub id: u64,
    #[serde(rename = "type")]
    pub kind: String,
    pub name: String,
    pub slug: String,
    #[serde(default)]
    pub count: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NhentaiDetailExtra {
    pub media_id: String,
    pub japanese_title: Option<String>,
    pub pretty_title: Option<String>,
    pub num_pages: Option<u32>,
    pub num_favorites: Option<u32>,
    pub scanlator: Option<String>,
    pub upload_date: Option<u64>,
    pub cover: Option<String>,
    pub cover_width: Option<u32>,
    pub cover_height: Option<u32>,
    pub tags: Vec<NhentaiTag>,
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
pub(crate) struct ReaderPage {
    pub url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
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
        let item = SearchItem::new(
            "123",
            "Example",
            "https://cdn.example/1.jpg",
            SearchExtra::Jm(JmExtra {
                tags: vec!["tag-a".into(), "tag-b".into()],
            }),
        );

        let value = serde_json::to_value(item).unwrap();
        assert_eq!(value["platform"], "jm");
        assert_eq!(value["extra"]["tags"], json!(["tag-a", "tag-b"]));
        // The discriminant lives on the item, not inside the platform fields.
        assert!(value["extra"].get("platform").is_none());
    }

    #[test]
    fn comic_detail_serializes_platform_and_typed_extra() {
        let detail = ComicDetail::new(
            "Example",
            "https://cdn.example/1.jpg",
            DetailExtra::Jm(JmDetailExtra {
                authors: vec!["Author".into()],
                tags: vec!["tag-a".into()],
                works: Vec::new(),
                actors: Vec::new(),
                page_count: Some(20),
                pub_date: None,
                update_date: None,
                likes: None,
                views: None,
                comment_count: None,
            }),
        );

        let value = serde_json::to_value(detail).unwrap();
        assert_eq!(value["platform"], "jm");
        assert_eq!(value["title"], "Example");
        assert_eq!(value["extra"]["authors"], json!(["Author"]));
        assert_eq!(value["extra"]["pageCount"], 20);
        // The discriminant is a sibling of `extra`, not a duplicate inside it.
        assert!(value["extra"].get("platform").is_none());
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
        assert_eq!(value["previous"]["id"], "11");
        assert!(value["next"].is_null());
        assert!(value.get("chapter_id").is_none());
    }
}
