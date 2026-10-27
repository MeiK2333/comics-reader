//! Opt-in application logging.
//!
//! The preference lives in the webview's persisted settings and is pushed to
//! Rust through [`set_logging_enabled`] on startup and whenever the user
//! toggles it. While enabled, commands append one JSON object per line to
//! `<app_log_dir>/app.log`; [`read_logs`] returns the newest entries for the
//! settings log viewer. The file is capped and rotated so diagnostics cannot
//! grow without bound.
//!
//! Rust owns the file so network, search and ECH failures can be recorded even
//! when the webview is busy. Logging is intentionally silent when disabled:
//! the hot-path helpers return before resolving the file path.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, Result};

const LOG_FILE_NAME: &str = "app.log";
const MAX_LOG_BYTES: u64 = 1024 * 1024;
const DEFAULT_TAIL_ENTRIES: usize = 500;

static ENABLED: AtomicBool = AtomicBool::new(false);
static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();
static WRITE_LOCK: Mutex<()> = Mutex::new(());

/// Severity shown next to an entry in the viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LogLevel {
    Info,
    Warn,
    Error,
}

/// One line of the JSONL log file.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LogEntry {
    /// Unix timestamp in milliseconds.
    timestamp: u64,
    level: LogLevel,
    target: String,
    message: String,
}

impl LogEntry {
    fn new(level: LogLevel, target: &str, message: impl Into<String>) -> Self {
        Self {
            timestamp: now_millis(),
            level,
            target: target.to_owned(),
            message: message.into(),
        }
    }
}

/// Everything the viewer needs in a single round trip.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LogSnapshot {
    enabled: bool,
    entries: Vec<LogEntry>,
    /// `true` when older entries were dropped by the tail limit.
    truncated: bool,
}

/// Resolve and prepare the log directory once at startup.
///
/// Path resolution is allowed to fail without aborting the app: logging is a
/// diagnostic aid, not a startup dependency. Commands resolve the path again
/// if setup did not get that far.
pub(crate) fn init(app: &AppHandle) -> Result<PathBuf> {
    let path = log_path(app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(path)
}

pub(crate) fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

pub(crate) fn info(target: &str, message: impl Into<String>) {
    write_if_enabled(LogLevel::Info, target, message);
}

pub(crate) fn warn(target: &str, message: impl Into<String>) {
    write_if_enabled(LogLevel::Warn, target, message);
}

pub(crate) fn error(target: &str, message: impl Into<String>) {
    write_if_enabled(LogLevel::Error, target, message);
}

fn write_if_enabled(level: LogLevel, target: &str, message: impl Into<String>) {
    if !is_enabled() {
        return;
    }
    let Some(path) = LOG_PATH.get() else {
        return;
    };
    let entry = LogEntry::new(level, target, message);
    if let Err(error) = append(path, &entry) {
        // Never recurse into logging; stderr is the only remaining channel.
        eprintln!("failed to write the application log: {error}");
    }
}

/// Turn file logging on or off. Enabling writes a header entry so the viewer
/// has something to show immediately and so the file is created eagerly.
#[tauri::command]
pub(crate) fn set_logging_enabled(app: AppHandle, enabled: bool) -> Result<()> {
    let path = log_path(&app)?;

    if enabled {
        if is_enabled() {
            return Ok(());
        }
        append(
            &path,
            &LogEntry::new(
                LogLevel::Info,
                "logging",
                format!(
                    "logging enabled (Comics Reader {})",
                    env!("CARGO_PKG_VERSION")
                ),
            ),
        )?;
        ENABLED.store(true, Ordering::Relaxed);
    } else if ENABLED.swap(false, Ordering::Relaxed) {
        // Record the final event while appending is still allowed. A failed
        // write must not keep the switch on: the user asked to stop logging.
        let entry = LogEntry::new(LogLevel::Info, "logging", "logging disabled");
        if let Err(error) = append(&path, &entry) {
            eprintln!("failed to write the application log: {error}");
        }
    }

    Ok(())
}

/// Return the newest entries for the viewer. Reading is deliberately bounded,
/// so a large file (from an older build) cannot freeze the settings page.
#[tauri::command]
pub(crate) fn read_logs(app: AppHandle) -> Result<LogSnapshot> {
    let path = log_path(&app)?;
    let (entries, truncated) = read_tail(&path, DEFAULT_TAIL_ENTRIES)?;

    Ok(LogSnapshot {
        enabled: is_enabled(),
        entries,
        truncated,
    })
}

/// Copy the rotated and active log files to a destination chosen by the user.
/// Reading both before writing also makes choosing the active file harmless.
#[tauri::command]
pub(crate) fn export_logs(app: AppHandle, destination: String) -> Result<()> {
    let destination = PathBuf::from(destination.trim());
    if !destination.is_absolute() {
        return Err(AppError::InvalidInput {
            field: "destination",
            reason: "must be an absolute path".into(),
        });
    }

    let path = log_path(&app)?;
    let contents = collect_log_contents(&path)?;
    fs::write(&destination, contents)?;
    info(
        "logging",
        format!("logs exported to {}", destination.to_string_lossy()),
    );
    Ok(())
}

/// Remove both the active and rotated files. Logging stays enabled and the
/// next event recreates the active file.
#[tauri::command]
pub(crate) fn clear_logs(app: AppHandle) -> Result<()> {
    let path = log_path(&app)?;
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| AppError::Unexpected("log write lock was poisoned".into()))?;

    for candidate in [&path, &rotated_path(&path)] {
        match fs::remove_file(candidate) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn log_path(app: &AppHandle) -> Result<PathBuf> {
    if let Some(path) = LOG_PATH.get() {
        return Ok(path.clone());
    }

    let path = app.path().app_log_dir()?.join(LOG_FILE_NAME);
    let _ = LOG_PATH.set(path.clone());
    Ok(path)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn append(path: &Path, entry: &LogEntry) -> Result<()> {
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| AppError::Unexpected("log write lock was poisoned".into()))?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    rotate_if_needed(path)?;

    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    let line = serde_json::to_string(entry)?;
    file.write_all(line.as_bytes())?;
    file.write_all(b"\n")?;
    Ok(())
}

/// Keep one previous file. Rotation runs under the write lock, so a reader can
/// only ever see a complete active file.
fn rotate_if_needed(path: &Path) -> Result<()> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() < MAX_LOG_BYTES {
        return Ok(());
    }

    let rotated = rotated_path(path);
    match fs::remove_file(&rotated) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    fs::rename(path, rotated)?;
    Ok(())
}

fn rotated_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".1");
    path.with_file_name(name)
}

/// Concatenate the rotated file before the active one so an export reads as a
/// single chronological log.
fn collect_log_contents(path: &Path) -> Result<Vec<u8>> {
    let mut contents = Vec::new();
    for candidate in [rotated_path(path), path.to_path_buf()] {
        match fs::read(&candidate) {
            Ok(bytes) => contents.extend_from_slice(&bytes),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(contents)
}

fn read_tail(path: &Path, limit: usize) -> Result<(Vec<LogEntry>, bool)> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok((Vec::new(), false)),
        Err(error) => return Err(error.into()),
    };
    let contents = String::from_utf8_lossy(&bytes);

    // A truncated final line can remain if the process was killed mid-write;
    // lossy decoding also tolerates a split multi-byte character. Skipping
    // malformed lines keeps the rest of the file readable.
    let mut entries: Vec<LogEntry> = contents
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    let total = entries.len();
    if total > limit {
        entries.drain(..total - limit);
        Ok((entries, true))
    } else {
        Ok((entries, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "comics-reader-log-{label}-{}-{}.log",
            std::process::id(),
            now_millis()
        ))
    }

    fn entry(level: LogLevel, message: &str) -> LogEntry {
        LogEntry {
            timestamp: 1_700_000_000_000,
            level,
            target: "test".into(),
            message: message.into(),
        }
    }

    #[test]
    fn serializes_entries_with_a_stable_wire_shape() {
        let value = serde_json::to_value(entry(LogLevel::Warn, "careful")).unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "timestamp": 1_700_000_000_000_u64,
                "level": "warn",
                "target": "test",
                "message": "careful"
            })
        );
        assert_eq!(
            serde_json::from_value::<LogEntry>(value).unwrap(),
            entry(LogLevel::Warn, "careful")
        );
    }

    #[test]
    fn collect_places_the_rotated_file_before_the_active_file() {
        let path = temp_path("export");
        fs::write(rotated_path(&path), b"old\n").unwrap();
        fs::write(&path, b"new\n").unwrap();

        assert_eq!(collect_log_contents(&path).unwrap(), b"old\nnew\n");

        fs::remove_file(rotated_path(&path)).unwrap();
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn tail_keeps_only_the_newest_entries() {
        let path = temp_path("tail");
        let contents = (0..5)
            .map(|index| serde_json::to_string(&entry(LogLevel::Info, &index.to_string())).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, format!("{contents}\n")).unwrap();

        let (entries, truncated) = read_tail(&path, 3).unwrap();

        assert!(truncated);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.message.as_str())
                .collect::<Vec<_>>(),
            ["2", "3", "4"]
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn skips_malformed_lines_and_missing_files() {
        let path = temp_path("malformed");
        let valid = serde_json::to_string(&entry(LogLevel::Error, "still readable")).unwrap();
        let mut contents = vec![0xff, b'\n'];
        contents.extend_from_slice(format!("not json\n{valid}\n").as_bytes());
        fs::write(&path, contents).unwrap();

        let (entries, truncated) = read_tail(&path, 10).unwrap();

        assert!(!truncated);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "still readable");
        fs::remove_file(&path).unwrap();

        let missing = read_tail(&path, 10).unwrap();
        assert!(missing.0.is_empty());
        assert!(!missing.1);
    }
    #[test]
    fn serializes_the_log_snapshot_contract() {
        let snapshot = LogSnapshot {
            enabled: true,
            entries: vec![
                LogEntry {
                    timestamp: 1_700_000_000_000,
                    level: LogLevel::Info,
                    target: "image".into(),
                    message: "loaded".into(),
                },
                LogEntry {
                    timestamp: 1_700_000_001_000,
                    level: LogLevel::Error,
                    target: "downloads".into(),
                    message: "failed".into(),
                },
            ],
            truncated: true,
        };

        let expected: serde_json::Value =
            serde_json::from_str(include_str!("../../contracts/log-snapshot.json")).unwrap();
        assert_eq!(serde_json::to_value(snapshot).unwrap(), expected);
    }
}
