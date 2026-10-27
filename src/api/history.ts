import { invokeLocal } from "./client";
import type { ComicPlatform } from "../platforms";

/** One reading-history row. Mirrors `history::HistoryEntry`. */
export interface HistoryEntry {
  platform: ComicPlatform;
  id: string;
  title: string;
  thumb: string | null;
  chapterId: string | null;
  chapterTitle: string | null;
  page: number;
  totalPages: number;
  /** Unix timestamp in milliseconds, assigned by Rust. */
  updatedAt: number;
}

/** Payload the reader sends after a chapter loads or the page changes. */
export interface HistoryInput {
  platform: ComicPlatform;
  id: string;
  title: string;
  thumb: string | null;
  chapterId: string | null;
  chapterTitle: string | null;
  page: number;
  totalPages: number;
}

/** One page of history plus the total number of works. */
export interface HistoryPage {
  total: number;
  entries: HistoryEntry[];
}

/** Read one page of history, newest first. */
export function listHistory(offset = 0, limit = 50): Promise<HistoryPage> {
  return invokeLocal<HistoryPage>("list_history", { offset, limit });
}

/** Read the saved progress for one work, or `null` if it was never read. */
export function getHistoryEntry(
  platform: ComicPlatform,
  id: string,
): Promise<HistoryEntry | null> {
  return invokeLocal<HistoryEntry | null>("get_history_entry", { platform, id });
}

/** Insert or update the current progress for one work. */
export function recordHistory(entry: HistoryInput): Promise<void> {
  return invokeLocal("record_history", { entry });
}
