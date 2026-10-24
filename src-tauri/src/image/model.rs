//! Wire types for platform search and comic detail commands.

use serde::Serialize;

/// Paginated search response. `extra` on each item is platform-specific.
#[derive(Debug, Serialize)]
pub(crate) struct SearchResponse {
    pub total: u64,
    pub page_size: u32,
    pub result: Vec<SearchItem>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SearchItem {
    pub id: String,
    pub title: String,
    pub thumb: String,
    pub extra: serde_json::Value,
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
}
