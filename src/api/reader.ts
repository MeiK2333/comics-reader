import { invoke } from "@tauri-apps/api/core";
import type { ComicPlatform } from "./comics";
import { waitForEchStartup } from "./ech";

export interface ReaderPage {
  url: string;
  width: number | null;
  height: number | null;
}

/** Navigation target for chaptered platforms; empty for single-work galleries. */
export interface ReaderChapterRef {
  id: string;
  title: string;
}

export interface ReaderChapter {
  /** Display title of the work, used for the native window title. */
  title: string;
  chapterId: string | null;
  chapterTitle: string | null;
  pages: ReaderPage[];
  previous: ReaderChapterRef | null;
  next: ReaderChapterRef | null;
}

/**
 * Fetch the page list for a chapter. nhentai galleries are a single implicit
 * chapter, so `chapterId` is omitted there.
 */
export async function getReaderChapter(
  platform: ComicPlatform,
  id: string,
  chapterId?: string,
): Promise<ReaderChapter> {
  await waitForEchStartup();
  return invoke<ReaderChapter>("get_reader_chapter", {
    platform,
    id,
    chapterId: chapterId ?? null,
  });
}
