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

use tauri::{AppHandle, Manager};
use tokio::sync::Semaphore;

use crate::error::{AppError, Result};
use crate::logging;
use crate::platforms::find_catalog;
use crate::platforms::model::{ChapterRef, ReaderChapter, ReaderPage};
use crate::storage::{
    atomic_write_json, backup_corrupt as move_corrupt_file, now_millis, page_window,
};

mod index;
mod local;
mod model;
mod store;
mod worker;

pub(crate) mod commands;

pub(crate) use index::{DownloadDocument, DownloadManifest, ManifestPage};
pub(crate) use model::*;
#[cfg(test)]
pub(crate) use store::DownloadsData;
pub(crate) use store::DownloadsStore;

const DOWNLOADS_DIR: &str = "downloads";
const INDEX_FILE_NAME: &str = "downloads.json";
const DOCUMENT_VERSION: u32 = 1;
const MANIFEST_VERSION: u32 = 1;
const MANIFEST_FILE_NAME: &str = "manifest.json";
const MAX_CHAPTERS: usize = 5_000;
const MAX_ID_CHARS: usize = 64;
const MAX_TITLE_CHARS: usize = 2_000;
const MAX_THUMB_CHARS: usize = 4_096;

type DownloadKey = (String, String);

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
    if find_catalog(platform).is_some_and(|catalog| catalog.single_chapter()) {
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
    let Some(catalog) = find_catalog(platform) else {
        return Err(AppError::InvalidInput {
            field: "platform",
            reason: "must be a supported platform".into(),
        });
    };

    let id = catalog.normalize_work_id(id)?;
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
    fn download_detail_matches_the_shared_wire_contract() {
        let summary = DownloadSummary {
            platform: "manhuagui".into(),
            id: "42".into(),
            title: "Example Download".into(),
            thumb: None,
            cover_path: Some("/tmp/downloads/manhuagui/42/cover.jpg".into()),
            status: DownloadStatus::Partial,
            total_chapters: 2,
            completed_chapters: 1,
            failed_chapters: 1,
            downloaded_pages: 12,
            total_bytes: 34567,
            read_chapter_id: Some("1".into()),
            added_at: 1_700_000_000_000,
            updated_at: 1_700_000_001_000,
            error: None,
            progress: Some(DownloadProgress {
                chapter_id: Some("2".into()),
                chapter_title: Some("Chapter 2".into()),
                chapter_index: 1,
                chapter_count: 2,
                page: 5,
                page_count: 8,
            }),
        };
        let detail = DownloadDetail {
            summary,
            chapters: vec![
                DownloadChapterSummary {
                    id: Some("1".into()),
                    title: Some("Chapter 1".into()),
                    page_count: 10,
                    completed: true,
                },
                DownloadChapterSummary {
                    id: Some("2".into()),
                    title: Some("Chapter 2".into()),
                    page_count: 8,
                    completed: false,
                },
            ],
        };

        let actual = serde_json::to_value(&detail).unwrap();
        let expected: serde_json::Value =
            serde_json::from_str(include_str!("../../../contracts/download-detail.json")).unwrap();
        assert_eq!(actual, expected);
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
