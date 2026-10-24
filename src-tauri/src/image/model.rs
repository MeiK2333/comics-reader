//! Wire types for platform search and comic detail commands.

use serde::Serialize;

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

/// Detail response. `extra` carries platform-specific metadata.
#[derive(Debug, Serialize)]
pub(crate) struct ComicDetail {
    pub title: String,
    pub thumb: String,
    pub extra: serde_json::Value,
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
}
