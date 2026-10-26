import { invoke } from "@tauri-apps/api/core";
import type { ComicPlatform } from "./comics";
import { getDownloadedChapter } from "./downloads";
import { waitForEchStartup } from "./ech";

export interface ReaderPage {
  url: string;
  width: number | null;
  height: number | null;
  /** Absolute downloaded file path; local pages skip the network entirely. */
  localPath: string | null;
}

/** Navigation target for chaptered platforms; empty for single-work galleries. */
export interface ReaderChapterRef {
  id: string;
  title: string;
}

export interface ReaderChapter {
  /** Display title of the work, used for the native window title. */
  title: string;
  /** Cover thumbnail, reused by the reading-history list. */
  thumb: string;
  chapterId: string | null;
  chapterTitle: string | null;
  pages: ReaderPage[];
  previous: ReaderChapterRef | null;
  next: ReaderChapterRef | null;
}

// The next chapter's page list is fetched while the reader sits on the last
// page. Keeping both the in-flight promise and the result lets a fast page turn
// join the request instead of issuing it a second time.
const CHAPTER_CACHE_LIMIT = 4;
const cachedChapters = new Map<string, ReaderChapter>();
const pendingChapters = new Map<string, Promise<ReaderChapter>>();

function chapterKey(
  platform: ComicPlatform,
  id: string,
  chapterId: string,
): string {
  return `${platform}\u0000${id}\u0000${chapterId}`;
}

function rememberChapter(key: string, chapter: ReaderChapter) {
  cachedChapters.delete(key);
  cachedChapters.set(key, chapter);
  while (cachedChapters.size > CHAPTER_CACHE_LIMIT) {
    const oldest = cachedChapters.keys().next().value;
    if (oldest === undefined) {
      break;
    }
    cachedChapters.delete(oldest);
  }
}

/** Return a chapter page list that was already fetched by a prefetch. */
export function getCachedReaderChapter(
  platform: ComicPlatform,
  id: string,
  chapterId?: string,
): ReaderChapter | null {
  if (!chapterId) {
    return null;
  }
  return cachedChapters.get(chapterKey(platform, id, chapterId)) ?? null;
}

/** Return an in-flight prefetch for this chapter, if one exists. */
export function getPendingReaderChapter(
  platform: ComicPlatform,
  id: string,
  chapterId?: string,
): Promise<ReaderChapter> | null {
  if (!chapterId) {
    return null;
  }
  return pendingChapters.get(chapterKey(platform, id, chapterId)) ?? null;
}

/**
 * Fetch and cache a chapter page list for preloading. If the same chapter is
 * being loaded by the reader, both callers share one request.
 */
export function prefetchReaderChapter(
  platform: ComicPlatform,
  id: string,
  chapterId: string,
): Promise<ReaderChapter> {
  const key = chapterKey(platform, id, chapterId);
  const cached = cachedChapters.get(key);
  if (cached) {
    return Promise.resolve(cached);
  }
  const pending = pendingChapters.get(key);
  if (pending) {
    return pending;
  }

  const request = getReaderChapter(platform, id, chapterId).then((chapter) => {
    rememberChapter(key, chapter);
    return chapter;
  });
  const cleanup = () => {
    if (pendingChapters.get(key) === request) {
      pendingChapters.delete(key);
    }
  };
  request.then(cleanup, cleanup);
  pendingChapters.set(key, request);
  return request;
}

/**
 * Fetch the page list for a chapter. Downloads win over the network so an
 * offline reader never waits for ECH or the platform. nhentai galleries are a
 * single implicit chapter, so `chapterId` is omitted there.
 */
export async function getReaderChapter(
  platform: ComicPlatform,
  id: string,
  chapterId?: string,
): Promise<ReaderChapter> {
  try {
    const local = await getDownloadedChapter(platform, id, chapterId);
    if (local) {
      return local;
    }
  } catch {
    // Download lookup is an optimization. A broken index must not prevent the
    // online reader from loading the same chapter.
  }

  await waitForEchStartup();
  return invoke<ReaderChapter>("get_reader_chapter", {
    platform,
    id,
    chapterId: chapterId ?? null,
  });
}

/**
 * Drop resolved chapter page lists after their download was deleted. The
 * reader would otherwise keep pointing at files that no longer exist.
 */
export function clearReaderChapterCache() {
  cachedChapters.clear();
}
