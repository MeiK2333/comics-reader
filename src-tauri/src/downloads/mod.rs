//! Persistent comic downloads and the queue that fills them.
//!
//! Downloads are application data, not an image cache: the files live under
//! `app_data_dir()/downloads/` and survive any future cache cleanup. A small
//! versioned index (`downloads.json`) owns the list, while each completed
//! chapter writes a manifest next to its pages. Page writes are atomic so a
//! killed process leaves either a complete file or no file at all.
//!
//! Only one work downloads at a time. Each run gets a generation number; pause
//! and resume replace it, and the worker checks it after every await before it
//! touches the index or filesystem. Restarting the app marks abandoned
//! downloads paused instead of silently resuming network traffic.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::{AppHandle, Manager};
use tokio::sync::Semaphore;

use crate::error::{AppError, Result};
use crate::image::{ChapterRef, ReaderChapter, ReaderPage};
use crate::logging;
use crate::storage::{
    atomic_write_json, backup_corrupt as move_corrupt_file, now_millis, page_window,
};

mod local;
mod worker;

pub(crate) mod commands;

const DOWNLOADS_DIR: &str = "downloads";
const INDEX_FILE_NAME: &str = "downloads.json";
const DOCUMENT_VERSION: u32 = 1;
const MANIFEST_VERSION: u32 = 1;
const MANIFEST_FILE_NAME: &str = "manifest.json";
const MAX_CHAPTERS: usize = 5_000;
const MAX_ID_CHARS: usize = 64;
const MAX_TITLE_CHARS: usize = 2_000;
const MAX_THUMB_CHARS: usize = 4_096;
const KNOWN_PLATFORMS: [&str; 3] = ["manhuagui", "jm", "nhentai"];

type DownloadKey = (String, String);

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
    fn is_active(self) -> bool {
        matches!(self, Self::Queued | Self::Downloading)
    }
}

/// Serialized `AppError` kept with a failed chapter or work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadFailure {
    code: String,
    message: String,
    #[serde(default)]
    details: Map<String, Value>,
}

impl DownloadFailure {
    fn from_app(error: &AppError) -> Self {
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
    id: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadInput {
    platform: String,
    id: String,
    title: String,
    thumb: Option<String>,
    chapters: Vec<DownloadChapterInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadChapter {
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    page_count: u32,
    #[serde(default)]
    completed: bool,
    #[serde(default)]
    error: Option<DownloadFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadRecord {
    platform: String,
    id: String,
    title: String,
    #[serde(default)]
    thumb: Option<String>,
    #[serde(default)]
    cover_file: Option<String>,
    status: DownloadStatus,
    #[serde(default)]
    chapters: Vec<DownloadChapter>,
    #[serde(default)]
    downloaded_pages: u32,
    #[serde(default)]
    total_bytes: u64,
    added_at: u64,
    updated_at: u64,
    #[serde(default)]
    error: Option<DownloadFailure>,
    /// Runtime-only cancellation generation. Replaced on resume/pause; never
    /// persisted because workers do not survive a restart.
    #[serde(skip)]
    run_id: u64,
}

impl DownloadRecord {
    fn new(
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
            downloaded_pages: 0,
            total_bytes: 0,
            added_at: now,
            updated_at: now,
            error: None,
            run_id: 0,
        }
    }

    fn key(&self) -> DownloadKey {
        (self.platform.clone(), self.id.clone())
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadSummary {
    platform: String,
    id: String,
    title: String,
    thumb: Option<String>,
    /// Absolute path of the local cover, when one was downloaded.
    cover_path: Option<String>,
    status: DownloadStatus,
    total_chapters: usize,
    completed_chapters: usize,
    failed_chapters: usize,
    downloaded_pages: u32,
    total_bytes: u64,
    /// First completed chapter; `null` for nhentai's implicit chapter.
    read_chapter_id: Option<String>,
    added_at: u64,
    updated_at: u64,
    error: Option<DownloadFailure>,
    progress: Option<DownloadProgress>,
}

/// One chapter in the detail payload. The offline detail page keeps the
/// original order so fallback numbering stays stable even when only a subset
/// was downloaded.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadChapterSummary {
    id: Option<String>,
    title: Option<String>,
    page_count: u32,
    completed: bool,
}

/// `get_download` payload: the list summary plus every stored chapter. The
/// detail page can render downloaded chapters when the online lookup fails.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadDetail {
    #[serde(flatten)]
    summary: DownloadSummary,
    chapters: Vec<DownloadChapterSummary>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadProgress {
    chapter_id: Option<String>,
    chapter_title: Option<String>,
    chapter_index: u32,
    chapter_count: u32,
    page: u32,
    page_count: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadPage {
    total: usize,
    entries: Vec<DownloadSummary>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadDocument {
    version: u32,
    entries: Vec<DownloadRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadManifest {
    version: u32,
    chapter_id: Option<String>,
    chapter_title: Option<String>,
    pages: Vec<ManifestPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestPage {
    url: String,
    file: String,
    width: Option<u32>,
    height: Option<u32>,
}

#[derive(Default)]
struct DownloadsData {
    loaded: bool,
    entries: HashMap<DownloadKey, DownloadRecord>,
    corrupt: bool,
}

/// Tauri-managed store. The index is loaded once and then kept in memory; the
/// semaphore serializes works and each run's id invalidates the previous
/// worker when the user pauses or resumes.
pub(crate) struct DownloadsStore {
    data: Mutex<DownloadsData>,
    queue: Semaphore,
    progress: Mutex<HashMap<DownloadKey, DownloadProgress>>,
    /// Monotonic run ids. A deleted work can be queued again, so the counter
    /// must never restart from the record; that would let an old worker pass
    /// `is_current` for the new run.
    run_seq: AtomicU64,
}

impl Default for DownloadsStore {
    fn default() -> Self {
        Self {
            data: Mutex::new(DownloadsData::default()),
            queue: Semaphore::new(1),
            progress: Mutex::new(HashMap::new()),
            run_seq: AtomicU64::new(0),
        }
    }
}

impl DownloadsStore {
    fn next_run_id(&self) -> u64 {
        self.run_seq.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn with_data<T>(
        &self,
        path: &Path,
        operation: impl FnOnce(&mut DownloadsData, &Path) -> Result<T>,
    ) -> Result<T> {
        let mut data = self.lock_data()?;
        if !data.loaded {
            data.load(path)?;
            data.loaded = true;
        }
        operation(&mut data, path)
    }

    fn lock_data(&self) -> Result<MutexGuard<'_, DownloadsData>> {
        self.data
            .lock()
            .map_err(|_| AppError::Unexpected("downloads lock was poisoned".into()))
    }

    fn lock_progress(&self) -> Result<MutexGuard<'_, HashMap<DownloadKey, DownloadProgress>>> {
        self.progress
            .lock()
            .map_err(|_| AppError::Unexpected("download progress lock was poisoned".into()))
    }

    fn start(
        &self,
        path: &Path,
        root: &Path,
        input: DownloadInput,
    ) -> Result<(DownloadSummary, Option<u64>)> {
        let platform = input.platform.trim().to_owned();
        let key = download_key(&platform, &input.id)?;
        let title = validate_title(&input.title)?;
        let thumb = validate_thumb(input.thumb)?;
        let chapters = normalize_input_chapters(&platform, input.chapters)?;

        let record = self.with_data(path, |data, path| {
            let mut record = match data.entries.remove(&key) {
                // An active record already owns the work. Starting it again is
                // a no-op; the caller gets the current progress instead of a
                // duplicate queue entry. A brand-new record must not take this
                // branch: it starts as `Queued` and still needs a run id.
                Some(record) if record.status.is_active() => {
                    data.entries.insert(key.clone(), record.clone());
                    return Ok(record);
                }
                Some(record) => record,
                None => DownloadRecord::new(&key, title.clone(), thumb.clone(), chapters.clone()),
            };

            record.title = title;
            record.thumb = thumb;
            record.chapters = merge_chapters(&record.chapters, chapters);
            record.updated_at = now_millis();
            record.error = None;

            if record.chapters.iter().all(|chapter| chapter.completed) {
                record.status = DownloadStatus::Completed;
            } else {
                record.run_id = self.next_run_id();
                record.status = DownloadStatus::Queued;
                for chapter in &mut record.chapters {
                    if !chapter.completed {
                        chapter.error = None;
                    }
                }
            }

            let saved = record.clone();
            data.entries.insert(key.clone(), record);
            data.save(path)?;
            Ok(saved)
        })?;

        let spawn = (record.status == DownloadStatus::Queued).then_some(record.run_id);
        let summary = self.summary(root, &record)?;
        Ok((summary, spawn))
    }

    fn resume(
        &self,
        path: &Path,
        root: &Path,
        platform: &str,
        id: &str,
    ) -> Result<(DownloadSummary, Option<u64>)> {
        let key = download_key(platform, id)?;
        let record = self.with_data(path, |data, path| {
            let Some(mut record) = data.entries.remove(&key) else {
                return Err(AppError::NotFound {
                    resource: "download".into(),
                });
            };

            if record.status == DownloadStatus::Completed || record.status.is_active() {
                data.entries.insert(key.clone(), record.clone());
                return Ok(record);
            }

            record.run_id = self.next_run_id();
            record.status = DownloadStatus::Queued;
            record.error = None;
            record.updated_at = now_millis();
            for chapter in &mut record.chapters {
                if !chapter.completed {
                    chapter.error = None;
                }
            }

            let saved = record.clone();
            data.entries.insert(key.clone(), record);
            data.save(path)?;
            Ok(saved)
        })?;

        let spawn = (record.status == DownloadStatus::Queued).then_some(record.run_id);
        let summary = self.summary(root, &record)?;
        Ok((summary, spawn))
    }

    fn pause(&self, path: &Path, root: &Path, platform: &str, id: &str) -> Result<DownloadSummary> {
        let key = download_key(platform, id)?;
        let record = self.with_data(path, |data, path| {
            let Some(mut record) = data.entries.remove(&key) else {
                return Err(AppError::NotFound {
                    resource: "download".into(),
                });
            };

            if record.status.is_active() {
                record.run_id = self.next_run_id();
                record.status = DownloadStatus::Paused;
                record.updated_at = now_millis();
                let saved = record.clone();
                data.entries.insert(key.clone(), record);
                data.save(path)?;
                return Ok(saved);
            }

            data.entries.insert(key.clone(), record.clone());
            Ok(record)
        })?;

        self.summary(root, &record)
    }

    fn delete(&self, path: &Path, root: &Path, platform: &str, id: &str) -> Result<()> {
        let key = download_key(platform, id)?;

        self.with_data(path, |data, path| {
            let Some(record) = data.entries.get(&key) else {
                return Ok(());
            };
            if record.status.is_active() {
                return Err(AppError::InvalidInput {
                    field: "status",
                    reason: "pause the download before deleting it".into(),
                });
            }

            let directory = work_dir(root, record);
            if directory.exists() {
                fs::remove_dir_all(&directory)?;
            }
            data.entries.remove(&key);
            data.save(path)
        })
    }

    fn get(
        &self,
        path: &Path,
        root: &Path,
        platform: &str,
        id: &str,
    ) -> Result<Option<DownloadDetail>> {
        let key = download_key(platform, id)?;
        let record = self.with_data(path, |data, _| Ok(data.entries.get(&key).cloned()))?;
        let Some(record) = record else {
            return Ok(None);
        };

        let summary = self.summary(root, &record)?;
        let chapters = record
            .chapters
            .iter()
            .map(|chapter| DownloadChapterSummary {
                id: chapter.id.clone(),
                title: chapter.title.clone(),
                page_count: chapter.page_count,
                completed: chapter.completed,
            })
            .collect();
        Ok(Some(DownloadDetail { summary, chapters }))
    }

    fn list(&self, path: &Path, root: &Path, offset: usize, limit: usize) -> Result<DownloadPage> {
        let records = self.with_data(path, |data, _| {
            let mut records: Vec<DownloadRecord> = data.entries.values().cloned().collect();
            records.sort_by(|left, right| {
                right
                    .added_at
                    .cmp(&left.added_at)
                    .then_with(|| left.platform.cmp(&right.platform))
                    .then_with(|| left.id.cmp(&right.id))
            });
            Ok(records)
        })?;

        let total = records.len();
        let entries = records
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|record| self.summary(root, &record))
            .collect::<Result<Vec<_>>>()?;

        Ok(DownloadPage { total, entries })
    }

    fn summary(&self, root: &Path, record: &DownloadRecord) -> Result<DownloadSummary> {
        let total_chapters = record.chapters.len();
        let completed_chapters = record.chapters.iter().filter(|item| item.completed).count();
        let failed_chapters = record
            .chapters
            .iter()
            .filter(|item| !item.completed && item.error.is_some())
            .count();
        let read_chapter_id = record
            .chapters
            .iter()
            .find(|item| item.completed)
            .and_then(|item| item.id.clone());
        let cover_path = record
            .cover_file
            .as_deref()
            .filter(|name| is_safe_file_name(name))
            .map(|name| path_to_string(&work_dir(root, record).join(name)))
            .transpose()?;
        let progress = if record.status == DownloadStatus::Downloading {
            self.lock_progress()?.get(&record.key()).cloned()
        } else {
            None
        };

        Ok(DownloadSummary {
            platform: record.platform.clone(),
            id: record.id.clone(),
            title: record.title.clone(),
            thumb: record.thumb.clone(),
            cover_path,
            status: record.status,
            total_chapters,
            completed_chapters,
            failed_chapters,
            downloaded_pages: record.downloaded_pages,
            total_bytes: record.total_bytes,
            read_chapter_id,
            added_at: record.added_at,
            updated_at: record.updated_at,
            error: record.error.clone(),
            progress,
        })
    }

    fn record_snapshot(&self, path: &Path, key: &DownloadKey) -> Result<Option<DownloadRecord>> {
        self.with_data(path, |data, _| Ok(data.entries.get(key).cloned()))
    }

    fn is_current(&self, path: &Path, key: &DownloadKey, run_id: u64) -> Result<bool> {
        self.with_data(path, |data, _| {
            Ok(data.entries.get(key).is_some_and(|record| {
                record.run_id == run_id && record.status == DownloadStatus::Downloading
            }))
        })
    }

    fn begin_worker(&self, path: &Path, key: &DownloadKey, run_id: u64) -> Result<bool> {
        self.with_data(path, |data, path| {
            let Some(record) = data.entries.get_mut(key) else {
                return Ok(false);
            };
            if record.run_id != run_id || record.status != DownloadStatus::Queued {
                return Ok(false);
            }
            record.status = DownloadStatus::Downloading;
            record.updated_at = now_millis();
            data.save(path)?;
            Ok(true)
        })
    }

    fn set_cover(
        &self,
        path: &Path,
        key: &DownloadKey,
        run_id: u64,
        cover_file: String,
    ) -> Result<bool> {
        self.with_data(path, |data, path| {
            let Some(record) = data.entries.get_mut(key) else {
                return Ok(false);
            };
            if record.run_id != run_id || record.status != DownloadStatus::Downloading {
                return Ok(false);
            }
            record.cover_file = Some(cover_file);
            record.updated_at = now_millis();
            data.save(path)?;
            Ok(true)
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn note_chapter_started(
        &self,
        path: &Path,
        key: &DownloadKey,
        run_id: u64,
        chapter_index: u32,
        chapter_count: u32,
        page_count: u32,
        source_title: Option<String>,
    ) -> Result<bool> {
        self.with_data(path, |data, path| {
            let Some(record) = data.entries.get_mut(key) else {
                return Ok(false);
            };
            if record.run_id != run_id || record.status != DownloadStatus::Downloading {
                return Ok(false);
            }
            let Some(chapter) = record.chapters.get_mut(chapter_index as usize) else {
                return Ok(false);
            };
            chapter.page_count = page_count;
            chapter.error = None;
            if chapter.title.is_none() {
                chapter.title = source_title;
            }
            record.updated_at = now_millis();

            self.lock_progress()?.insert(
                key.clone(),
                DownloadProgress {
                    chapter_id: chapter.id.clone(),
                    chapter_title: chapter.title.clone(),
                    // The wire field is 1-based for display; the method
                    // argument remains the 0-based chapter slot.
                    chapter_index: chapter_index.saturating_add(1),
                    chapter_count,
                    page: 0,
                    page_count,
                },
            );
            data.save(path)?;
            Ok(true)
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn note_page(
        &self,
        path: &Path,
        key: &DownloadKey,
        run_id: u64,
        page: u32,
        page_count: u32,
        bytes: u64,
        counted: bool,
    ) -> Result<bool> {
        self.with_data(path, |data, _| {
            let Some(record) = data.entries.get_mut(key) else {
                return Ok(false);
            };
            if record.run_id != run_id || record.status != DownloadStatus::Downloading {
                return Ok(false);
            }
            if counted {
                record.downloaded_pages = record.downloaded_pages.saturating_add(1);
                record.total_bytes = record.total_bytes.saturating_add(bytes);
            }

            if let Some(progress) = self.lock_progress()?.get_mut(key) {
                progress.page = page;
                progress.page_count = page_count;
            }
            Ok(true)
        })
    }

    fn complete_chapter(
        &self,
        path: &Path,
        key: &DownloadKey,
        run_id: u64,
        chapter_index: u32,
        page_count: u32,
    ) -> Result<bool> {
        self.with_data(path, |data, path| {
            let Some(record) = data.entries.get_mut(key) else {
                return Ok(false);
            };
            if record.run_id != run_id || record.status != DownloadStatus::Downloading {
                return Ok(false);
            }
            let Some(chapter) = record.chapters.get_mut(chapter_index as usize) else {
                return Ok(false);
            };
            chapter.completed = true;
            chapter.page_count = page_count;
            chapter.error = None;
            record.updated_at = now_millis();
            if let Some(progress) = self.lock_progress()?.get_mut(key) {
                progress.page = page_count;
                progress.page_count = page_count;
            }
            data.save(path)?;
            Ok(true)
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn fail_chapter(
        &self,
        path: &Path,
        key: &DownloadKey,
        run_id: u64,
        chapter_index: u32,
        page_count: u32,
        failure: DownloadFailure,
    ) -> Result<bool> {
        self.with_data(path, |data, path| {
            let Some(record) = data.entries.get_mut(key) else {
                return Ok(false);
            };
            if record.run_id != run_id || record.status != DownloadStatus::Downloading {
                return Ok(false);
            }
            let Some(chapter) = record.chapters.get_mut(chapter_index as usize) else {
                return Ok(false);
            };
            chapter.completed = false;
            chapter.page_count = page_count;
            chapter.error = Some(failure);
            record.updated_at = now_millis();
            data.save(path)?;
            Ok(true)
        })
    }

    fn finish_worker(&self, path: &Path, key: &DownloadKey, run_id: u64, result: Result<()>) {
        let outcome = self.with_data(path, |data, path| {
            let Some(record) = data.entries.get_mut(key) else {
                return Ok(None);
            };
            if record.run_id != run_id {
                return Ok(None);
            }

            let failure = result.err().map(|error| DownloadFailure::from_app(&error));
            let completed = record.chapters.iter().filter(|item| item.completed).count();
            let all_completed = !record.chapters.is_empty() && completed == record.chapters.len();
            record.status = if all_completed {
                DownloadStatus::Completed
            } else if completed > 0 {
                DownloadStatus::Partial
            } else {
                DownloadStatus::Failed
            };
            record.downloaded_pages = record
                .chapters
                .iter()
                .filter(|item| item.completed)
                .map(|item| item.page_count)
                .sum();
            record.error = if record.status == DownloadStatus::Completed {
                None
            } else {
                failure.or_else(|| {
                    record
                        .chapters
                        .iter()
                        .rev()
                        .find_map(|item| item.error.clone())
                })
            };
            record.updated_at = now_millis();
            let status = record.status;
            let error = record.error.clone();
            data.save(path)?;
            Ok(Some((status, error)))
        });

        match outcome {
            Ok(Some((status, error))) => match status {
                DownloadStatus::Completed => logging::info(
                    "download",
                    format!("download completed: platform={} id={}", key.0, key.1),
                ),
                _ => logging::warn(
                    "download",
                    format!(
                        "download finished with status {:?}: platform={} id={} error={:?}",
                        status, key.0, key.1, error
                    ),
                ),
            },
            Ok(None) => {}
            Err(error) => logging::error(
                "download",
                format!(
                    "failed to finalize download: platform={} id={} error={error}",
                    key.0, key.1
                ),
            ),
        }
    }

    fn read_local_chapter(
        &self,
        path: &Path,
        root: &Path,
        platform: &str,
        id: &str,
        chapter_id: Option<&str>,
    ) -> Result<Option<ReaderChapter>> {
        local::read_local_chapter(self, path, root, platform, id, chapter_id)
    }
}

impl DownloadsData {
    fn load(&mut self, path: &Path) -> Result<()> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };

        let document = match serde_json::from_slice::<DownloadDocument>(&bytes) {
            Ok(document) => document,
            Err(error) => {
                logging::warn(
                    "download",
                    format!("failed to parse downloads; treating it as empty: {error}"),
                );
                self.corrupt = true;
                return Ok(());
            }
        };
        if document.version != DOCUMENT_VERSION {
            return Err(AppError::Unexpected(format!(
                "unsupported downloads document version {}",
                document.version
            )));
        }

        self.entries.clear();
        let mut needs_save = false;
        for record in document.entries {
            let Some(mut record) = record.sanitized() else {
                needs_save = true;
                logging::warn("download", "skipping an invalid download entry");
                continue;
            };
            if record.status.is_active() {
                record.status = DownloadStatus::Paused;
                needs_save = true;
            }
            if self.entries.insert(record.key(), record).is_some() {
                needs_save = true;
            }
        }

        if needs_save && let Err(error) = self.save(path) {
            logging::warn(
                "download",
                format!("failed to normalize downloads: {error}"),
            );
        }
        Ok(())
    }

    fn save(&mut self, path: &Path) -> Result<()> {
        if self.corrupt {
            self.backup_corrupt(path);
        }

        let mut entries: Vec<DownloadRecord> = self.entries.values().cloned().collect();
        entries.sort_by(|left, right| {
            left.added_at
                .cmp(&right.added_at)
                .then_with(|| left.platform.cmp(&right.platform))
                .then_with(|| left.id.cmp(&right.id))
        });

        write_document(
            path,
            &DownloadDocument {
                version: DOCUMENT_VERSION,
                entries,
            },
        )?;
        self.corrupt = false;
        Ok(())
    }

    fn backup_corrupt(&mut self, path: &Path) {
        match move_corrupt_file(path) {
            Ok(Some(backup)) => logging::warn(
                "download",
                format!(
                    "backed up an unreadable downloads file to {}",
                    backup.display()
                ),
            ),
            Ok(None) => {}
            Err(error) => logging::warn(
                "download",
                format!("failed to back up the unreadable downloads file: {error}"),
            ),
        }
    }
}

impl DownloadRecord {
    fn sanitized(mut self) -> Option<Self> {
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

        self.chapters.retain_mut(|chapter| {
            if self.platform == "nhentai" {
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

fn chapter_ref(chapter: &DownloadChapter) -> Option<ChapterRef> {
    chapter.id.as_ref().map(|id| ChapterRef {
        id: id.clone(),
        title: chapter.title.clone().unwrap_or_default(),
    })
}

fn merge_chapters(
    existing: &[DownloadChapter],
    incoming: Vec<DownloadChapter>,
) -> Vec<DownloadChapter> {
    let mut old: HashMap<Option<String>, DownloadChapter> = existing
        .iter()
        .cloned()
        .map(|chapter| (chapter.id.clone(), chapter))
        .collect();
    let mut merged = Vec::new();

    for mut chapter in incoming {
        if let Some(previous) = old.remove(&chapter.id) {
            chapter.page_count = previous.page_count;
            chapter.completed = previous.completed;
            chapter.error = previous.error;
            if chapter.title.is_none() {
                chapter.title = previous.title;
            }
        }
        merged.push(chapter);
    }

    // Keep downloaded chapters that disappeared from the current detail page;
    // dropping them would silently delete content the user already has.
    for chapter in existing {
        if old.remove(&chapter.id).is_some() {
            merged.push(chapter.clone());
        }
    }
    merged
}

fn normalize_input_chapters(
    platform: &str,
    chapters: Vec<DownloadChapterInput>,
) -> Result<Vec<DownloadChapter>> {
    if platform == "nhentai" {
        return Ok(vec![DownloadChapter {
            id: None,
            title: None,
            page_count: 0,
            completed: false,
            error: None,
        }]);
    }

    if chapters.is_empty() {
        return Err(AppError::InvalidInput {
            field: "chapters",
            reason: "must contain at least one chapter".into(),
        });
    }
    if chapters.len() > MAX_CHAPTERS {
        return Err(AppError::InvalidInput {
            field: "chapters",
            reason: "contains too many chapters".into(),
        });
    }

    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(chapters.len());
    for chapter in chapters {
        let Some(id) = chapter.id.as_deref() else {
            return Err(AppError::InvalidInput {
                field: "chapterId",
                reason: "must not be empty".into(),
            });
        };
        let id = numeric_id(id, "chapterId")?;
        if !seen.insert(id.clone()) {
            continue;
        }
        normalized.push(DownloadChapter {
            id: Some(id),
            title: chapter.title.and_then(non_empty),
            page_count: 0,
            completed: false,
            error: None,
        });
    }

    if normalized.is_empty() {
        return Err(AppError::InvalidInput {
            field: "chapters",
            reason: "must contain at least one chapter".into(),
        });
    }
    Ok(normalized)
}

fn download_key(platform: &str, id: &str) -> Result<DownloadKey> {
    let platform = platform.trim();
    if !KNOWN_PLATFORMS.contains(&platform) {
        return Err(AppError::InvalidInput {
            field: "platform",
            reason: "must be a supported platform".into(),
        });
    }

    let id = id.trim();
    let digits = if platform == "jm" {
        id.strip_prefix("JM")
            .or_else(|| id.strip_prefix("jm"))
            .unwrap_or(id)
    } else {
        id
    };
    let id = numeric_id(digits, "id")?;
    Ok((platform.to_owned(), id))
}

fn numeric_id(value: &str, field: &'static str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() || !value.chars().all(|character| character.is_ascii_digit()) {
        return Err(AppError::InvalidInput {
            field,
            reason: "must be numeric".into(),
        });
    }
    if value.chars().count() > MAX_ID_CHARS {
        return Err(AppError::InvalidInput {
            field,
            reason: "is too long".into(),
        });
    }
    Ok(value.to_owned())
}

fn validate_title(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::InvalidInput {
            field: "title",
            reason: "must not be empty".into(),
        });
    }
    if value.chars().count() > MAX_TITLE_CHARS {
        return Err(AppError::InvalidInput {
            field: "title",
            reason: "is too long".into(),
        });
    }
    Ok(value.to_owned())
}

fn validate_thumb(value: Option<String>) -> Result<Option<String>> {
    let value = value.and_then(non_empty);
    if let Some(value) = value.as_ref()
        && value.chars().count() > MAX_THUMB_CHARS
    {
        return Err(AppError::InvalidInput {
            field: "thumb",
            reason: "is too long".into(),
        });
    }
    Ok(value)
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn is_safe_file_name(name: &str) -> bool {
    let path = Path::new(name);
    !name.is_empty()
        && name != "."
        && name != ".."
        && path.components().count() == 1
        && path.file_name().and_then(|value| value.to_str()) == Some(name)
}

fn downloads_index_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join(INDEX_FILE_NAME))
}

fn downloads_root(app: &AppHandle) -> Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join(DOWNLOADS_DIR))
}

fn work_dir(root: &Path, record: &DownloadRecord) -> PathBuf {
    root.join(&record.platform).join(&record.id)
}

fn chapter_dir(root: &Path, record: &DownloadRecord, chapter_id: Option<&str>) -> PathBuf {
    work_dir(root, record).join(chapter_id.unwrap_or("single"))
}

fn path_to_string(path: &Path) -> Result<String> {
    path.to_str().map(str::to_owned).ok_or_else(|| {
        AppError::Unexpected(format!(
            "download path is not valid UTF-8: {}",
            path.display()
        ))
    })
}

fn write_document(path: &Path, document: &DownloadDocument) -> Result<()> {
    atomic_write_json(path, document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{backup_path, temporary_path};

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "comics-reader-downloads-{label}-{}-{}.json",
            std::process::id(),
            now_millis()
        ))
    }

    fn chapter(id: Option<&str>, completed: bool) -> DownloadChapter {
        DownloadChapter {
            id: id.map(str::to_owned),
            title: id.map(|value| format!("Chapter {value}")),
            page_count: if completed { 10 } else { 0 },
            completed,
            error: None,
        }
    }

    fn record() -> DownloadRecord {
        DownloadRecord::new(
            &("nhentai".into(), "42".into()),
            "Gallery 42".into(),
            Some("https://t.nhentai.net/galleries/1/thumb.webp".into()),
            vec![chapter(None, false)],
        )
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(temporary_path(path));
        let _ = fs::remove_file(backup_path(path));
    }

    #[test]
    fn serializes_statuses_and_summaries_camel_case() {
        assert_eq!(
            serde_json::to_value(DownloadStatus::Partial).unwrap(),
            serde_json::json!("partial")
        );

        let summary = DownloadSummary {
            platform: "nhentai".into(),
            id: "42".into(),
            title: "Gallery".into(),
            thumb: None,
            cover_path: None,
            status: DownloadStatus::Completed,
            total_chapters: 1,
            completed_chapters: 1,
            failed_chapters: 0,
            downloaded_pages: 10,
            total_bytes: 100,
            read_chapter_id: None,
            added_at: 1,
            updated_at: 2,
            error: None,
            progress: None,
        };
        let value = serde_json::to_value(&summary).unwrap();

        assert_eq!(value["readChapterId"], serde_json::Value::Null);
        assert_eq!(value["totalChapters"], 1);
        assert!(value.get("read_chapter_id").is_none());

        let detail = DownloadDetail {
            summary,
            chapters: vec![DownloadChapterSummary {
                id: Some("7".into()),
                title: Some("Chapter 7".into()),
                page_count: 10,
                completed: true,
            }],
        };
        let value = serde_json::to_value(detail).unwrap();

        assert_eq!(value["id"], "42");
        assert_eq!(value["chapters"][0]["pageCount"], 10);
        assert_eq!(value["chapters"][0]["completed"], true);
        assert!(value["chapters"][0].get("page_count").is_none());
    }

    #[test]
    fn merges_chapters_without_losing_completed_state() {
        let existing = vec![chapter(Some("1"), true), chapter(Some("3"), true)];
        let incoming = vec![chapter(Some("1"), false), chapter(Some("2"), false)];

        let merged = merge_chapters(&existing, incoming);

        assert_eq!(merged.len(), 3);
        assert_eq!(merged[0].id.as_deref(), Some("1"));
        assert!(merged[0].completed);
        assert_eq!(merged[1].id.as_deref(), Some("2"));
        assert!(!merged[1].completed);
        assert_eq!(merged[2].id.as_deref(), Some("3"));
        assert!(merged[2].completed);
    }

    #[test]
    fn normalizes_chapter_input_and_rejects_duplicates() {
        let chapters = normalize_input_chapters(
            "manhuagui",
            vec![
                DownloadChapterInput {
                    id: Some(" 1 ".into()),
                    title: Some(" First ".into()),
                },
                DownloadChapterInput {
                    id: Some("1".into()),
                    title: None,
                },
                DownloadChapterInput {
                    id: Some("2".into()),
                    title: None,
                },
            ],
        )
        .unwrap();

        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].id.as_deref(), Some("1"));
        assert_eq!(chapters[0].title.as_deref(), Some("First"));
        assert_eq!(chapters[1].id.as_deref(), Some("2"));
    }

    #[test]
    fn nhentai_always_has_one_implicit_chapter() {
        let chapters = normalize_input_chapters("nhentai", Vec::new()).unwrap();

        assert_eq!(chapters.len(), 1);
        assert!(chapters[0].id.is_none());
    }

    #[test]
    fn rejects_unsafe_download_keys_and_file_names() {
        assert!(download_key("nhentai", "../42").is_err());
        assert!(download_key("unknown", "42").is_err());
        assert_eq!(
            download_key("jm", "JM123").unwrap(),
            ("jm".into(), "123".into())
        );

        assert!(is_safe_file_name("00001.jpg"));
        assert!(!is_safe_file_name("../00001.jpg"));
        assert!(!is_safe_file_name("nested/00001.jpg"));
        assert!(!is_safe_file_name(".."));
    }

    #[test]
    fn loads_and_normalizes_an_abandoned_download() {
        let path = temp_path("normalize");
        let mut document = record();
        document.status = DownloadStatus::Downloading;
        document.chapters[0].completed = true;
        fs::write(
            &path,
            serde_json::to_vec(&DownloadDocument {
                version: DOCUMENT_VERSION,
                entries: vec![document],
            })
            .unwrap(),
        )
        .unwrap();

        let mut data = DownloadsData::default();
        data.load(&path).unwrap();

        let saved = data.entries.values().next().unwrap();
        assert_eq!(saved.status, DownloadStatus::Paused);
        assert_eq!(saved.run_id, 0);
        assert!(saved.chapters[0].completed);

        cleanup(&path);
    }

    #[test]
    fn rejects_unsupported_document_versions() {
        let path = temp_path("version");
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "version": 99,
                "entries": []
            }))
            .unwrap(),
        )
        .unwrap();

        let mut data = DownloadsData::default();
        assert_eq!(
            data.load(&path).unwrap_err().code(),
            crate::error::ErrorCode::Unexpected
        );

        cleanup(&path);
    }

    #[tokio::test]
    async fn reads_a_completed_chapter_and_links_only_downloaded_neighbours() {
        let root = temp_path("local-root").with_extension("dir");
        let directory = root.join("manhuagui").join("42").join("3");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("00001.jpg"), b"page one").unwrap();
        fs::write(directory.join("00002.webp"), b"page two").unwrap();
        fs::write(
            directory.join(MANIFEST_FILE_NAME),
            serde_json::to_vec(&DownloadManifest {
                version: MANIFEST_VERSION,
                chapter_id: Some("3".into()),
                chapter_title: Some("Chapter 3".into()),
                pages: vec![
                    ManifestPage {
                        url: "https://example.com/1.jpg".into(),
                        file: "00001.jpg".into(),
                        width: Some(800),
                        height: Some(1200),
                    },
                    ManifestPage {
                        url: "https://example.com/2.webp".into(),
                        file: "00002.webp".into(),
                        width: None,
                        height: None,
                    },
                ],
            })
            .unwrap(),
        )
        .unwrap();

        let mut record = DownloadRecord::new(
            &("manhuagui".into(), "42".into()),
            "Work".into(),
            None,
            vec![
                chapter(Some("1"), true),
                chapter(Some("2"), false),
                chapter(Some("3"), true),
            ],
        );
        record.status = DownloadStatus::Completed;
        record.chapters[0].page_count = 2;
        record.chapters[2].page_count = 2;

        let store = DownloadsStore::default();
        {
            let mut data = store.data.lock().unwrap();
            data.loaded = true;
            data.entries.insert(record.key(), record);
        }

        let local = store
            .read_local_chapter(
                &temp_path("local-index"),
                &root,
                "manhuagui",
                "42",
                Some("3"),
            )
            .unwrap()
            .expect("chapter should resolve");

        assert_eq!(local.chapter_id.as_deref(), Some("3"));
        assert_eq!(local.chapter_title.as_deref(), Some("Chapter 3"));
        assert_eq!(local.pages.len(), 2);
        assert!(
            local.pages[0]
                .local_path
                .as_deref()
                .is_some_and(|path| path.ends_with("00001.jpg"))
        );
        assert_eq!(local.previous.as_ref().unwrap().id, "1");
        assert!(local.next.is_none());

        let detail = store
            .get(&temp_path("local-index"), &root, "manhuagui", "42")
            .unwrap()
            .expect("download detail should resolve");
        assert_eq!(detail.chapters.len(), 3);
        assert_eq!(detail.summary.completed_chapters, 2);
        assert!(detail.chapters[0].completed);
        assert!(!detail.chapters[1].completed);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn backs_up_a_corrupt_index_before_overwriting_it() {
        let path = temp_path("corrupt");
        fs::write(&path, "not json").unwrap();

        let mut data = DownloadsData::default();
        data.load(&path).unwrap();
        assert!(data.corrupt);

        data.entries
            .insert(("nhentai".into(), "42".into()), record());
        data.save(&path).unwrap();

        assert!(backup_path(&path).exists());
        assert!(path.exists());

        cleanup(&path);
        let _ = fs::remove_file(backup_path(&path));
    }

    fn download_input() -> DownloadInput {
        DownloadInput {
            platform: "manhuagui".into(),
            id: "42".into(),
            title: "Work".into(),
            thumb: None,
            chapters: vec![DownloadChapterInput {
                id: Some("1".into()),
                title: Some("Chapter 1".into()),
            }],
        }
    }

    #[test]
    fn pause_and_resume_invalidate_the_previous_worker() {
        let path = temp_path("run-id");
        let root = temp_path("run-id-root").with_extension("dir");
        let store = DownloadsStore::default();
        let key: DownloadKey = ("manhuagui".into(), "42".into());

        let (queued, spawn) = store.start(&path, &root, download_input()).unwrap();
        assert_eq!(queued.status, DownloadStatus::Queued);
        let first = spawn.expect("a queued download should spawn a worker");

        assert!(store.begin_worker(&path, &key, first).unwrap());
        assert!(store.is_current(&path, &key, first).unwrap());

        let paused = store.pause(&path, &root, "manhuagui", "42").unwrap();
        assert_eq!(paused.status, DownloadStatus::Paused);
        assert!(!store.is_current(&path, &key, first).unwrap());

        let (resumed, spawn) = store.resume(&path, &root, "manhuagui", "42").unwrap();
        assert_eq!(resumed.status, DownloadStatus::Queued);
        let second = spawn.expect("resume should spawn a new worker");
        assert!(second > first);

        // The old worker must not claim the resumed queue entry.
        assert!(!store.begin_worker(&path, &key, first).unwrap());
        assert!(store.begin_worker(&path, &key, second).unwrap());
        assert!(store.is_current(&path, &key, second).unwrap());

        cleanup(&path);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_deleted_work_gets_a_new_run_id_after_restarting() {
        let path = temp_path("run-id-aba");
        let root = temp_path("run-id-aba-root").with_extension("dir");
        let store = DownloadsStore::default();
        let key: DownloadKey = ("manhuagui".into(), "42".into());

        let (_, spawn) = store.start(&path, &root, download_input()).unwrap();
        let first = spawn.expect("first start should spawn a worker");
        assert!(store.begin_worker(&path, &key, first).unwrap());

        store.pause(&path, &root, "manhuagui", "42").unwrap();
        store.delete(&path, &root, "manhuagui", "42").unwrap();

        let (_, spawn) = store.start(&path, &root, download_input()).unwrap();
        let second = spawn.expect("restarting should spawn a new worker");
        assert!(second > first, "run ids must never be reused");

        // The old worker must not be able to claim or write the new record.
        assert!(!store.begin_worker(&path, &key, first).unwrap());
        assert!(!store.is_current(&path, &key, first).unwrap());
        assert!(store.begin_worker(&path, &key, second).unwrap());
        assert!(store.is_current(&path, &key, second).unwrap());

        cleanup(&path);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_deleting_an_active_download_and_allows_it_after_pause() {
        let path = temp_path("delete-active");
        let root = temp_path("delete-active-root").with_extension("dir");
        let store = DownloadsStore::default();
        let key: DownloadKey = ("manhuagui".into(), "42".into());

        store.start(&path, &root, download_input()).unwrap();
        let error = store
            .delete(&path, &root, "manhuagui", "42")
            .expect_err("an active download must not be deleted");
        assert_eq!(error.code(), crate::error::ErrorCode::InvalidInput);

        store.pause(&path, &root, "manhuagui", "42").unwrap();
        store.delete(&path, &root, "manhuagui", "42").unwrap();
        assert!(store.record_snapshot(&path, &key).unwrap().is_none());

        cleanup(&path);
        let _ = fs::remove_dir_all(&root);
    }
}
