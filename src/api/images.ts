import { invoke } from "@tauri-apps/api/core";
import type { ComicPlatform } from "./comics";
import { waitForEchStartup } from "./ech";

export interface CachedImage {
  /** Absolute path inside the app cache directory. */
  path: string;
  contentType: string;
  byteLength: number;
  fromCache: boolean;
  platform: string;
}

// The reader's visible slots and its read-ahead queue can ask for the same page
// at the same time. Sharing the in-flight request avoids downloading it twice;
// the entry is removed as soon as the request settles so failures can retry.
const inFlight = new Map<string, Promise<CachedImage>>();

function requestKey(platform: ComicPlatform, url: string): string {
  return `${platform}\u0000${url}`;
}

/**
 * Resolve one remote image through the Rust cache. The shared promise is only
 * an in-process dedupe layer; Rust still owns headers, caching and storage.
 */
export function fetchCachedImage(
  platform: ComicPlatform,
  url: string,
): Promise<CachedImage> {
  const key = requestKey(platform, url);
  const existing = inFlight.get(key);
  if (existing) {
    return existing;
  }

  const request = (async () => {
    await waitForEchStartup();
    return invoke<CachedImage>("fetch_image", { url, platform });
  })();
  inFlight.set(key, request);

  const cleanup = () => {
    if (inFlight.get(key) === request) {
      inFlight.delete(key);
    }
  };
  request.then(cleanup, cleanup);

  return request;
}

/** Warm the cache for a page the reader has not displayed yet. */
export async function prefetchCachedImage(
  platform: ComicPlatform,
  url: string,
): Promise<void> {
  await fetchCachedImage(platform, url);
}

/**
 * Warm several images with a small concurrency cap. Used when the reader
 * reaches the last page and prepares the next chapter without blocking it.
 */
export async function prefetchCachedImages(
  platform: ComicPlatform,
  urls: readonly string[],
  concurrency = 2,
): Promise<void> {
  let next = 0;
  const workers = Array.from(
    { length: Math.max(1, concurrency) },
    async () => {
      while (true) {
        const index = next;
        next += 1;
        if (index >= urls.length) {
          return;
        }
        try {
          await prefetchCachedImage(platform, urls[index]);
        } catch {
          // Best effort; the chapter still loads through the normal path.
        }
      }
    },
  );
  await Promise.all(workers);
}
