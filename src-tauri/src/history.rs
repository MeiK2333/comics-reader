//! Reading history persisted as an append-only JSONL event log.
//!
//! The file lives in the app data directory and holds at most
//! [`MAX_ENTRIES`] works. Saving progress appends one small line instead of
//! rewriting the whole list, which matters when the list approaches the cap.
//! The log is compacted into one record per work when it grows past twice the
//! cap; loading replays the remaining lines into an in-memory index so paging
//! and upserts stay cheap.
//!
//! History is auxiliary data: a malformed file is logged and treated as empty,
//! and the reader-facing commands never block reading when a write fails.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, Result};
use crate::logging;

const HISTORY_FILE_NAME: &str = "history.jsonl";
const DEFAULT_PAGE_SIZE: u32 = 50;
const MAX_PAGE_SIZE: u32 = 200;
pub(crate) const MAX_ENTRIES: usize = 10_000;
/// Compact once the log has this many lines. At the cap this bounds the file
/// to roughly twice the size of a compacted checkpoint.
const COMPACT_AFTER_EVENTS: usize = MAX_ENTRIES * 2;
const KNOWN_PLATFORMS: [&str; 3] = ["manhuagui", "jm", "nhentai"];

type HistoryKey = (String, String);

/// One reading-history row. `updated_at` is assigned by Rust so a client clock
/// cannot reorder the list.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryEntry {
    platform: String,
    id: String,
    title: String,
    thumb: Option<String>,
    chapter_id: Option<String>,
    chapter_title: Option<String>,
    page: u32,
    total_pages: u32,
    updated_at: u64,
}

impl HistoryEntry {
    fn key(&self) -> HistoryKey {
        (self.platform.clone(), self.id.clone())
    }

    /// Drop junk that a hand-edited or partially written file could contain.
    fn sanitized(mut self) -> Option<Self> {
        if !KNOWN_PLATFORMS.contains(&self.platform.as_str()) {
            return None;
        }

        self.id = self.id.trim().to_owned();
        self.title = self.title.trim().to_owned();
        if self.id.is_empty() || self.title.is_empty() || self.page == 0 || self.total_pages == 0 {
            return None;
        }

        self.page = self.page.min(self.total_pages);
        self.thumb = self.thumb.and_then(non_empty);
        self.chapter_id = self.chapter_id.and_then(non_empty);
        self.chapter_title = self.chapter_title.and_then(non_empty);
        Some(self)
    }
}

/// Payload sent by the reader after a chapter loads or the page changes.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryInput {
    platform: String,
    id: String,
    title: String,
    thumb: Option<String>,
    chapter_id: Option<String>,
    chapter_title: Option<String>,
    page: u32,
    total_pages: u32,
}

impl HistoryInput {
    fn into_entry(self, updated_at: u64) -> Result<HistoryEntry> {
        let platform = self.platform.trim();
        if !KNOWN_PLATFORMS.contains(&platform) {
            return Err(invalid("platform", "must be a supported platform"));
        }

        let id = self.id.trim();
        if id.is_empty() {
            return Err(invalid("id", "must not be empty"));
        }

        let title = self.title.trim();
        if title.is_empty() {
            return Err(invalid("title", "must not be empty"));
        }

        if self.total_pages == 0 {
            return Err(invalid("totalPages", "must be 1 or greater"));
        }
        if self.page == 0 {
            return Err(invalid("page", "must be 1 or greater"));
        }

        Ok(HistoryEntry {
            platform: platform.to_owned(),
            id: id.to_owned(),
            title: title.to_owned(),
            thumb: self.thumb.and_then(non_empty),
            chapter_id: self.chapter_id.and_then(non_empty),
            chapter_title: self.chapter_title.and_then(non_empty),
            page: self.page.min(self.total_pages),
            total_pages: self.total_pages,
            updated_at,
        })
    }
}

/// One line of the event log. Record events carry the whole entry, so replay
/// does not need to consult any other state.
#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum HistoryEvent {
    Record { entry: HistoryEntry },
    Remove { platform: String, id: String },
}

/// One page of history plus the total number of works, so the UI can paginate
/// without loading all entries over IPC.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HistoryPage {
    total: usize,
    entries: Vec<HistoryEntry>,
}

/// Tauri-managed store. The file is loaded on the first command and then
/// indexed in memory; every operation runs under the same lock.
#[derive(Default)]
pub(crate) struct HistoryStore {
    data: Mutex<HistoryData>,
}

#[derive(Default)]
struct HistoryData {
    loaded: bool,
    entries: HashMap<HistoryKey, HistoryEntry>,
    /// Lines currently in the log, used to decide when to compact.
    events: usize,
}

impl HistoryStore {
    fn with_data<T>(
        &self,
        path: &Path,
        operation: impl FnOnce(&mut HistoryData, &Path) -> Result<T>,
    ) -> Result<T> {
        let mut data = self
            .data
            .lock()
            .map_err(|_| AppError::Unexpected("history lock was poisoned".into()))?;
        if !data.loaded {
            data.load(path)?;
            data.loaded = true;
        }
        operation(&mut data, path)
    }
}

impl HistoryData {
    fn load(&mut self, path: &Path) -> Result<()> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        let contents = String::from_utf8_lossy(&bytes);

        // A truncated final line is expected after a crash; lossy decoding
        // also tolerates a split multi-byte character. Skipping malformed
        // lines keeps the remaining history readable.
        for line in contents.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            self.events += 1;
            match serde_json::from_str::<HistoryEvent>(line) {
                Ok(event) => self.apply(event),
                Err(error) => logging::warn(
                    "history",
                    format!("skipping malformed history line: {error}"),
                ),
            }
        }

        let trimmed = self.trim_to(MAX_ENTRIES);
        if (trimmed || self.events > COMPACT_AFTER_EVENTS)
            && let Err(error) = self.compact(path)
        {
            logging::warn("history", format!("failed to compact history: {error}"));
        }
        Ok(())
    }

    fn apply(&mut self, event: HistoryEvent) {
        match event {
            HistoryEvent::Record { entry } => {
                if let Some(entry) = entry.sanitized() {
                    self.entries.insert(entry.key(), entry);
                }
            }
            HistoryEvent::Remove { platform, id } => {
                self.entries.remove(&(platform, id));
            }
        }
    }

    fn record(&mut self, path: &Path, input: HistoryInput) -> Result<()> {
        let entry = input.into_entry(now_millis())?;
        append_event(
            path,
            &HistoryEvent::Record {
                entry: entry.clone(),
            },
        )?;
        self.events += 1;
        self.entries.insert(entry.key(), entry);
        // Evicted entries are not tombstoned: replaying the stale record adds
        // the oldest work back, and `trim_to` drops it again at load time.
        self.trim_to(MAX_ENTRIES);
        self.maybe_compact(path);
        Ok(())
    }

    fn remove(&mut self, path: &Path, platform: &str, id: &str) -> Result<()> {
        let key = (platform.to_owned(), id.to_owned());
        if !self.entries.contains_key(&key) {
            return Ok(());
        }

        append_event(
            path,
            &HistoryEvent::Remove {
                platform: key.0.clone(),
                id: key.1.clone(),
            },
        )?;
        self.entries.remove(&key);
        self.events += 1;
        self.maybe_compact(path);
        Ok(())
    }

    fn clear(&mut self, path: &Path) -> Result<()> {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        self.entries.clear();
        self.events = 0;
        Ok(())
    }

    fn list(&self, offset: usize, limit: usize) -> HistoryPage {
        let mut entries: Vec<HistoryEntry> = self.entries.values().cloned().collect();
        entries.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| left.platform.cmp(&right.platform))
                .then_with(|| left.id.cmp(&right.id))
        });

        let total = entries.len();
        let entries = entries.into_iter().skip(offset).take(limit).collect();
        HistoryPage { total, entries }
    }

    /// Keep the newest `max` entries and report whether anything was dropped.
    fn trim_to(&mut self, max: usize) -> bool {
        let mut trimmed = false;
        while self.entries.len() > max {
            let oldest = self
                .entries
                .values()
                .min_by(|left, right| {
                    left.updated_at
                        .cmp(&right.updated_at)
                        .then_with(|| left.platform.cmp(&right.platform))
                        .then_with(|| left.id.cmp(&right.id))
                })
                .map(|entry| entry.key());

            match oldest {
                Some(key) => {
                    self.entries.remove(&key);
                    trimmed = true;
                }
                None => break,
            }
        }
        trimmed
    }

    fn maybe_compact(&mut self, path: &Path) {
        if self.events <= COMPACT_AFTER_EVENTS {
            return;
        }
        if let Err(error) = self.compact(path) {
            logging::warn("history", format!("failed to compact history: {error}"));
        }
    }

    /// Rewrite the current index as a clean checkpoint. The temporary file
    /// lives next to the target so the final rename stays atomic.
    fn compact(&mut self, path: &Path) -> Result<()> {
        let temporary = temporary_path(path);
        let mut entries: Vec<HistoryEntry> = self.entries.values().cloned().collect();
        entries.sort_by_key(|entry| entry.updated_at);

        let result = (|| -> Result<()> {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let file = File::create(&temporary)?;
            let mut writer = BufWriter::new(file);
            for entry in entries {
                let event = HistoryEvent::Record { entry };
                serde_json::to_writer(&mut writer, &event)?;
                writer.write_all(b"\n")?;
            }
            writer.flush()?;
            writer.get_ref().sync_all()?;
            Ok(())
        })();

        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }

        fs::rename(&temporary, path)?;
        self.events = self.entries.len();
        Ok(())
    }
}

fn append_event(path: &Path, event: &HistoryEvent) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let line = serde_json::to_string(event)?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

fn non_empty(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn invalid(field: &'static str, reason: &str) -> AppError {
    AppError::InvalidInput {
        field,
        reason: reason.to_owned(),
    }
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn history_path(app: &AppHandle) -> Result<PathBuf> {
    Ok(app.path().app_data_dir()?.join(HISTORY_FILE_NAME))
}

/// Return one page of history, newest first.
#[tauri::command]
pub(crate) async fn list_history(
    app: AppHandle,
    offset: Option<u32>,
    limit: Option<u32>,
) -> Result<HistoryPage> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = history_path(&app)?;
        let store = app.state::<HistoryStore>();
        let offset = offset.unwrap_or(0) as usize;
        let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE) as usize;
        store.with_data(&path, |data, _| Ok(data.list(offset, limit)))
    })
    .await
    .map_err(|error| AppError::Unexpected(format!("history task failed: {error}")))?
}

/// Upsert the current progress for one work.
#[tauri::command]
pub(crate) async fn record_history(app: AppHandle, entry: HistoryInput) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = history_path(&app)?;
        let store = app.state::<HistoryStore>();
        store.with_data(&path, |data, path| data.record(path, entry))
    })
    .await
    .map_err(|error| AppError::Unexpected(format!("history task failed: {error}")))?
}

/// Remove one work from history. Removing an absent work is a no-op.
#[tauri::command]
pub(crate) async fn remove_history(app: AppHandle, platform: String, id: String) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = history_path(&app)?;
        let store = app.state::<HistoryStore>();
        store.with_data(&path, |data, path| data.remove(path, &platform, &id))
    })
    .await
    .map_err(|error| AppError::Unexpected(format!("history task failed: {error}")))?
}

/// Delete the history file and clear the in-memory index.
#[tauri::command]
pub(crate) async fn clear_history(app: AppHandle) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let path = history_path(&app)?;
        let store = app.state::<HistoryStore>();
        store.with_data(&path, |data, path| data.clear(path))
    })
    .await
    .map_err(|error| AppError::Unexpected(format!("history task failed: {error}")))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorCode;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "comics-reader-history-{label}-{}-{}.jsonl",
            std::process::id(),
            now_millis()
        ))
    }

    fn entry(id: &str, updated_at: u64) -> HistoryEntry {
        HistoryEntry {
            platform: "nhentai".into(),
            id: id.into(),
            title: format!("Gallery {id}"),
            thumb: Some("https://t.nhentai.net/galleries/1/thumb.webp".into()),
            chapter_id: None,
            chapter_title: None,
            page: 3,
            total_pages: 30,
            updated_at,
        }
    }

    fn input(id: &str) -> HistoryInput {
        HistoryInput {
            platform: "nhentai".into(),
            id: id.into(),
            title: format!("Gallery {id}"),
            thumb: Some("https://t.nhentai.net/galleries/1/thumb.webp".into()),
            chapter_id: None,
            chapter_title: None,
            page: 3,
            total_pages: 30,
        }
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(temporary_path(path));
    }

    #[test]
    fn serializes_entries_and_events_with_camel_case() {
        let value = serde_json::to_value(HistoryEvent::Record {
            entry: entry("42", 1_700_000_000_000),
        })
        .unwrap();

        assert_eq!(value["type"], "record");
        assert_eq!(value["entry"]["platform"], "nhentai");
        assert_eq!(value["entry"]["totalPages"], 30);
        assert_eq!(value["entry"]["updatedAt"], 1_700_000_000_000_u64);
        assert_eq!(value["entry"]["chapterId"], serde_json::Value::Null);
        assert!(value["entry"].get("total_pages").is_none());

        let remove = serde_json::to_value(HistoryEvent::Remove {
            platform: "nhentai".into(),
            id: "42".into(),
        })
        .unwrap();
        assert_eq!(
            remove,
            serde_json::json!({ "type": "remove", "platform": "nhentai", "id": "42" })
        );
    }

    #[test]
    fn records_upsert_by_platform_and_id() {
        let path = temp_path("upsert");
        let mut data = HistoryData::default();

        data.record(&path, input("1")).unwrap();
        data.record(&path, input("1")).unwrap();
        data.record(&path, input("2")).unwrap();

        assert_eq!(data.entries.len(), 2);
        cleanup(&path);
    }

    #[test]
    fn keeps_the_newest_entries_when_trimming() {
        let mut data = HistoryData::default();
        for (index, id) in ["a", "b", "c"].iter().enumerate() {
            let entry = entry(id, 100 + index as u64);
            data.entries.insert(entry.key(), entry);
        }

        assert!(data.trim_to(2));

        assert_eq!(data.entries.len(), 2);
        assert!(!data.entries.contains_key(&("nhentai".into(), "a".into())));
        assert!(data.entries.contains_key(&("nhentai".into(), "c".into())));
    }

    #[test]
    fn removes_and_clears_entries() {
        let path = temp_path("remove");
        let mut data = HistoryData::default();

        data.record(&path, input("1")).unwrap();
        data.remove(&path, "nhentai", "1").unwrap();
        assert!(data.entries.is_empty());

        let mut reloaded = HistoryData::default();
        reloaded.load(&path).unwrap();
        assert!(reloaded.entries.is_empty());

        data.record(&path, input("2")).unwrap();
        data.clear(&path).unwrap();
        assert!(data.entries.is_empty());
        assert!(!path.exists());

        cleanup(&path);
    }

    #[test]
    fn replays_the_log_and_skips_malformed_lines() {
        let path = temp_path("replay");
        let valid = serde_json::to_string(&HistoryEvent::Record {
            entry: entry("1", 1_700_000_000_000),
        })
        .unwrap();
        fs::write(
            &path,
            format!("not json\n{valid}\n{{\"type\":\"record\"}}\n"),
        )
        .unwrap();

        let mut data = HistoryData::default();
        data.load(&path).unwrap();

        assert_eq!(data.entries.len(), 1);
        assert!(data.entries.contains_key(&("nhentai".into(), "1".into())));

        cleanup(&path);
    }

    #[test]
    fn compacts_the_log_to_one_record_per_entry() {
        let path = temp_path("compact");
        let mut data = HistoryData::default();

        data.record(&path, input("1")).unwrap();
        data.record(&path, input("2")).unwrap();
        data.compact(&path).unwrap();

        let contents = fs::read_to_string(&path).unwrap();
        assert_eq!(contents.lines().count(), 2);
        assert!(contents.lines().all(|line| {
            serde_json::from_str::<HistoryEvent>(line)
                .is_ok_and(|event| matches!(event, HistoryEvent::Record { .. }))
        }));

        let mut reloaded = HistoryData::default();
        reloaded.load(&path).unwrap();
        assert_eq!(reloaded.entries.len(), 2);

        cleanup(&path);
    }

    #[test]
    fn pages_entries_newest_first() {
        let mut data = HistoryData::default();
        for (id, updated_at) in [("a", 100), ("b", 300), ("c", 200)] {
            let entry = entry(id, updated_at);
            data.entries.insert(entry.key(), entry);
        }

        let page = data.list(1, 1);

        assert_eq!(page.total, 3);
        assert_eq!(page.entries.len(), 1);
        assert_eq!(page.entries[0].id, "c");
    }

    #[test]
    fn rejects_invalid_records_and_clamps_the_page() {
        let mut value = input("1");
        value.platform = "unknown".into();
        assert_eq!(
            value.into_entry(0).unwrap_err().code(),
            ErrorCode::InvalidInput
        );

        let mut value = input("1");
        value.id = "  ".into();
        assert_eq!(
            value.into_entry(0).unwrap_err().code(),
            ErrorCode::InvalidInput
        );

        let mut value = input("1");
        value.page = 0;
        assert_eq!(
            value.into_entry(0).unwrap_err().code(),
            ErrorCode::InvalidInput
        );

        let mut value = input("1");
        value.total_pages = 0;
        assert_eq!(
            value.into_entry(0).unwrap_err().code(),
            ErrorCode::InvalidInput
        );

        let mut value = input("1");
        value.page = 99;
        assert_eq!(value.into_entry(0).unwrap().page, 30);
    }
}
