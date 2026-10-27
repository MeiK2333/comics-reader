//! On-disk download documents: the index and per-chapter manifests.

use serde::{Deserialize, Serialize};

use super::DownloadRecord;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadDocument {
    pub(super) version: u32,
    pub(super) entries: Vec<DownloadRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadManifest {
    pub(super) version: u32,
    pub(super) chapter_id: Option<String>,
    pub(super) chapter_title: Option<String>,
    pub(super) pages: Vec<ManifestPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ManifestPage {
    pub(super) url: String,
    pub(super) file: String,
    pub(super) width: Option<u32>,
    pub(super) height: Option<u32>,
}
