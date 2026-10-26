import { invoke } from "@tauri-apps/api/core";
import type { ErrorPayload } from "../errors";
import type { ComicPlatform } from "./comics";
import { waitForEchStartup } from "./ech";
import type { ReaderChapter } from "./reader";

export type DownloadStatus =
  | "queued"
  | "downloading"
  | "paused"
  | "completed"
  | "partial"
  | "failed";

/** Live page counters for the work currently being written. */
export interface DownloadProgress {
  chapterId: string | null;
  chapterTitle: string | null;
  chapterIndex: number;
  chapterCount: number;
  page: number;
  pageCount: number;
}

/** One row in the downloads list, also used by the detail-page button. */
export interface DownloadSummary {
  platform: ComicPlatform;
  id: string;
  title: string;
  thumb: string | null;
  /** Absolute path of the local cover, when one was stored. */
  coverPath: string | null;
  status: DownloadStatus;
  totalChapters: number;
  completedChapters: number;
  failedChapters: number;
  downloadedPages: number;
  totalBytes: number;
  /** First completed chapter; `null` for nhentai's implicit chapter. */
  readChapterId: string | null;
  addedAt: number;
  updatedAt: number;
  error: ErrorPayload | null;
  progress: DownloadProgress | null;
}

export interface DownloadPage {
  total: number;
  entries: DownloadSummary[];
}

export interface DownloadChapterInput {
  id: string | null;
  title: string | null;
}

export interface DownloadInput {
  platform: ComicPlatform;
  id: string;
  title: string;
  thumb: string | null;
  chapters: DownloadChapterInput[];
}

export function isDownloadActive(status: DownloadStatus): boolean {
  return status === "queued" || status === "downloading";
}

/** Queue a whole work. Existing completed chapters are preserved. */
export async function startDownload(
  input: DownloadInput,
): Promise<DownloadSummary> {
  await waitForEchStartup();
  return invoke<DownloadSummary>("start_download", { input });
}

/** Pause a queued or running work. */
export function pauseDownload(
  platform: ComicPlatform,
  id: string,
): Promise<DownloadSummary> {
  return invoke<DownloadSummary>("pause_download", { platform, id });
}

/** Resume the stored chapter list of an interrupted work. */
export async function resumeDownload(
  platform: ComicPlatform,
  id: string,
): Promise<DownloadSummary> {
  await waitForEchStartup();
  return invoke<DownloadSummary>("resume_download", { platform, id });
}

export function deleteDownload(
  platform: ComicPlatform,
  id: string,
): Promise<void> {
  return invoke("delete_download", { platform, id });
}

export function listDownloads(
  offset = 0,
  limit = 50,
): Promise<DownloadPage> {
  return invoke<DownloadPage>("list_downloads", { offset, limit });
}

export function getDownload(
  platform: ComicPlatform,
  id: string,
): Promise<DownloadSummary | null> {
  return invoke<DownloadSummary | null>("get_download", { platform, id });
}

/**
 * Resolve a chapter from the download store. This deliberately does not wait
 * for ECH: a downloaded chapter must open even when startup networking is
 * still being configured.
 */
export function getDownloadedChapter(
  platform: ComicPlatform,
  id: string,
  chapterId?: string,
): Promise<ReaderChapter | null> {
  return invoke<ReaderChapter | null>("get_downloaded_chapter", {
    platform,
    id,
    chapterId: chapterId ?? null,
  });
}
