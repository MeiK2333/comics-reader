import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";

/** Severity written by the Rust logger. */
export type LogLevel = "info" | "warn" | "error";

/** One entry returned by `read_logs`. Mirrors `logging::LogEntry`. */
export interface LogEntry {
  /** Unix timestamp in milliseconds. */
  timestamp: number;
  level: LogLevel;
  target: string;
  message: string;
}

/** Latest slice of the opt-in JSONL log file. */
export interface LogSnapshot {
  enabled: boolean;
  entries: LogEntry[];
  /** `true` when older entries were dropped by the backend tail limit. */
  truncated: boolean;
}

/** Toggle Rust file logging; the preference itself stays in the settings store. */
export function setLoggingEnabled(enabled: boolean): Promise<void> {
  return invoke("set_logging_enabled", { enabled });
}

/** Read the newest log entries for display. */
export function getLogs(): Promise<LogSnapshot> {
  return invoke("read_logs");
}

/**
 * Ask for a destination with the native save dialog, then copy the rotated and
 * active logs there. Returns `null` when the user cancels the dialog.
 */
export async function exportLogs(): Promise<string | null> {
  const date = new Date().toISOString().slice(0, 10);
  const destination = await save({
    defaultPath: `comics-reader-logs-${date}.log`,
    filters: [{ name: "Log", extensions: ["log"] }],
  });
  if (!destination) {
    return null;
  }

  await invoke("export_logs", { destination });
  return destination;
}

/** Delete the active and rotated log files without changing the switch. */
export function clearLogs(): Promise<void> {
  return invoke("clear_logs");
}
