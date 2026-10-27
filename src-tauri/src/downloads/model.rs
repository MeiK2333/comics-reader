//! Download wire types and the persisted record shape.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::AppError;
use crate::storage::now_millis;

use super::*;

/// State shown in the downloads list and on the detail page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    Completed,
    Partial,
    Failed,
}

impl DownloadStatus {
    pub(super) fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Downloading)
    }
}

/// Serialized `AppError` kept with a failed chapter or work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadFailure {
    pub(super) code: String,
    pub(super) message: String,
    #[serde(default)]
    pub(super) details: Map<String, Value>,
}

impl DownloadFailure {
    pub(super) fn from_app(error: &AppError) -> Self {
        Self {
            code: error.code().as_str().to_owned(),
            message: error.to_string(),
            details: error.details(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadChapterInput {
    pub(super) id: Option<String>,
    pub(super) title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadInput {
    pub(super) platform: String,
    pub(super) id: String,
    pub(super) title: String,
    pub(super) thumb: Option<String>,
    pub(super) chapters: Vec<DownloadChapterInput>,
    /// Full detail snapshot captured when the download starts. The chapter
    /// lists are stripped before it is stored; the download chapters own them.
    #[serde(default)]
    pub(super) detail: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadChapter {
    pub(super) id: Option<String>,
    #[serde(default)]
    pub(super) title: Option<String>,
    #[serde(default)]
    pub(super) page_count: u32,
    #[serde(default)]
    pub(super) completed: bool,
    #[serde(default)]
    pub(super) error: Option<DownloadFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadRecord {
    pub(super) platform: String,
    pub(super) id: String,
    pub(super) title: String,
    #[serde(default)]
    pub(super) thumb: Option<String>,
    #[serde(default)]
    pub(super) cover_file: Option<String>,
    pub(super) status: DownloadStatus,
    #[serde(default)]
    pub(super) chapters: Vec<DownloadChapter>,
    /// Online detail captured at download time so the downloaded detail page
    /// can render the same metadata without a network request.
    #[serde(default)]
    pub(super) detail: Option<Value>,
    #[serde(default)]
    pub(super) downloaded_pages: u32,
    #[serde(default)]
    pub(super) total_bytes: u64,
    pub(super) added_at: u64,
    pub(super) updated_at: u64,
    #[serde(default)]
    pub(super) error: Option<DownloadFailure>,
    /// Runtime-only cancellation generation. Replaced on resume/pause; never
    /// persisted because workers do not survive a restart.
    #[serde(skip)]
    pub(super) run_id: u64,
}

impl DownloadRecord {
    pub(super) fn new(
        key: &DownloadKey,
        title: String,
        thumb: Option<String>,
        chapters: Vec<DownloadChapter>,
    ) -> Self {
        let now = now_millis();
        Self {
            platform: key.0.clone(),
            id: key.1.clone(),
            title,
            thumb,
            cover_file: None,
            status: DownloadStatus::Queued,
            chapters,
            detail: None,
            downloaded_pages: 0,
            total_bytes: 0,
            added_at: now,
            updated_at: now,
            error: None,
            run_id: 0,
        }
    }

    pub(super) fn key(&self) -> DownloadKey {
        (self.platform.clone(), self.id.clone())
    }

    pub(super) fn sanitized(mut self) -> Option<Self> {
        let key = download_key(&self.platform, &self.id).ok()?;
        self.platform = key.0;
        self.id = key.1;

        self.title = self.title.trim().to_owned();
        if self.title.is_empty() || self.title.chars().count() > MAX_TITLE_CHARS {
            return None;
        }
        self.thumb = self
            .thumb
            .and_then(non_empty)
            .filter(|value| value.chars().count() <= MAX_THUMB_CHARS && value.starts_with("http"));
        self.cover_file = self
            .cover_file
            .filter(|name| is_safe_file_name(name))
            .and_then(non_empty);

        let snapshot_platform = self.platform.clone();
        self.detail = self.detail.take().filter(|detail| {
            detail.get("platform").and_then(Value::as_str) == Some(snapshot_platform.as_str())
                && detail.get("title").is_some_and(Value::is_string)
                && detail.get("thumb").is_some_and(Value::is_string)
                && detail.get("chapters").is_some_and(Value::is_array)
                && detail.get("extra").is_some_and(Value::is_object)
        });

        let single_chapter = crate::platforms::find_catalog(&self.platform)
            .is_some_and(|catalog| catalog.single_chapter());
        self.chapters.retain_mut(|chapter| {
            if single_chapter {
                chapter.id = None;
            } else {
                let Some(id) = chapter.id.as_deref() else {
                    return false;
                };
                let Ok(normalized) = numeric_id(id, "chapterId") else {
                    return false;
                };
                chapter.id = Some(normalized);
            }
            chapter.title = chapter.title.take().and_then(non_empty);
            true
        });
        let mut seen = HashSet::new();
        self.chapters
            .retain(|chapter| seen.insert(chapter.id.clone()));
        if self.chapters.is_empty() {
            return None;
        }

        self.added_at = self.added_at.max(1);
        self.updated_at = self.updated_at.max(self.added_at);
        self.run_id = 0;
        Some(self)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadSummary {
    pub(super) platform: String,
    pub(super) id: String,
    pub(super) title: String,
    pub(super) thumb: Option<String>,
    /// Absolute path of the local cover, when one was downloaded.
    pub(super) cover_path: Option<String>,
    pub(super) status: DownloadStatus,
    pub(super) total_chapters: usize,
    pub(super) completed_chapters: usize,
    pub(super) failed_chapters: usize,
    pub(super) downloaded_pages: u32,
    pub(super) total_bytes: u64,
    /// First completed chapter; `null` for nhentai's implicit chapter.
    pub(super) read_chapter_id: Option<String>,
    pub(super) added_at: u64,
    pub(super) updated_at: u64,
    pub(super) error: Option<DownloadFailure>,
    pub(super) progress: Option<DownloadProgress>,
}

/// One chapter in the detail payload. The offline detail page keeps the
/// original order so fallback numbering stays stable even when only a subset
/// was downloaded.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadChapterSummary {
    pub(super) id: Option<String>,
    pub(super) title: Option<String>,
    pub(super) page_count: u32,
    pub(super) completed: bool,
}

/// `get_download` payload: the list summary, every stored chapter, and the
/// online detail snapshot captured when the download started.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadDetail {
    #[serde(flatten)]
    pub(super) summary: DownloadSummary,
    pub(super) chapters: Vec<DownloadChapterSummary>,
    /// Detail snapshot stored with the download; absent for old records or
    /// when the payload could not be validated.
    pub(super) detail: Option<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadProgress {
    pub(super) chapter_id: Option<String>,
    pub(super) chapter_title: Option<String>,
    pub(super) chapter_index: u32,
    pub(super) chapter_count: u32,
    pub(super) page: u32,
    pub(super) page_count: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadPage {
    pub(super) total: usize,
    pub(super) entries: Vec<DownloadSummary>,
}
