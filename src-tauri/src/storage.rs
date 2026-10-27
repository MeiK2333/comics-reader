//! Small persistence helpers shared by the favorites and downloads indexes.
//!
//! Both documents are rewritten as a whole and must survive a crash, so they
//! use the same temporary-file + `sync_all` + rename sequence. History keeps
//! its append-only JSONL format; it only borrows the timestamp and paging
//! helpers.

use std::fs::{self, File};
use std::io::{BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::error::Result;

pub(crate) const DEFAULT_PAGE_SIZE: u32 = 50;
pub(crate) const MAX_PAGE_SIZE: u32 = 200;

/// Clamp an IPC paging request to the limits every list command shares.
pub(crate) fn page_window(offset: Option<u32>, limit: Option<u32>) -> (usize, usize) {
    let offset = offset.unwrap_or(0) as usize;
    let limit = limit.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE) as usize;
    (offset, limit)
}

pub(crate) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
        .max(1)
}

pub(crate) fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

pub(crate) fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bak");
    path.with_file_name(name)
}

/// Write `value` as newline-terminated JSON next to `path`, then rename it over
/// the target. The temporary file lives in the same directory so the rename is
/// atomic.
pub(crate) fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    let temporary = temporary_path(path);
    let result = (|| -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = File::create(&temporary)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, value)?;
        writer.write_all(b"\n")?;
        writer.flush()?;
        writer.get_ref().sync_all()?;
        Ok(())
    })();

    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }

    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }

    Ok(())
}

/// Preserve an unreadable document instead of overwriting it on the next save.
/// Returns the backup path when a file was moved; `None` means there was
/// nothing to back up.
pub(crate) fn backup_corrupt(path: &Path) -> Result<Option<PathBuf>> {
    let backup = backup_path(path);
    // `fs::rename` does not replace an existing backup on every platform, so
    // drop the previous one before moving the newest unreadable file into place.
    let _ = fs::remove_file(&backup);
    match fs::rename(path, &backup) {
        Ok(()) => Ok(Some(backup)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    use serde_json::json;

    fn temp_dir(label: &str) -> PathBuf {
        static ID: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "comics-reader-storage-{label}-{}-{}",
            std::process::id(),
            ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn atomic_write_replaces_the_document() {
        let dir = temp_dir("atomic");
        let path = dir.join("state.json");

        atomic_write_json(&path, &json!({ "version": 1, "entries": [] })).unwrap();
        atomic_write_json(&path, &json!({ "version": 2, "entries": [1] })).unwrap();

        let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["version"], 2);
        assert_eq!(value["entries"], json!([1]));
        assert!(!temporary_path(&path).exists());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn backup_corrupt_moves_the_file_once() {
        let dir = temp_dir("backup");
        let path = dir.join("state.json");
        atomic_write_json(&path, &json!({ "version": 1 })).unwrap();

        let backup = backup_corrupt(&path).unwrap().expect("file should exist");
        assert_eq!(backup, backup_path(&path));
        assert!(backup.exists());
        assert!(!path.exists());
        assert!(backup_corrupt(&path).unwrap().is_none());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn page_window_clamps_offset_and_limit() {
        assert_eq!(page_window(None, None), (0, 50));
        assert_eq!(page_window(Some(20), Some(10)), (20, 10));
        assert_eq!(page_window(Some(0), Some(0)), (0, 1));
        assert_eq!(page_window(Some(0), Some(10_000)), (0, 200));
    }

    #[test]
    fn timestamps_are_never_zero() {
        assert!(now_millis() > 0);
    }
}
