import { invokeCommand } from "./client";
import type { ComicPlatform } from "../platforms";

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

// Preloaded pages also keep their resolved path. That lets the next chapter
// render with the asset URL immediately instead of showing the page spinner
// while the same cache entry is looked up again.
const RESOLVED_CACHE_LIMIT = 64;
const resolvedImages = new Map<string, CachedImage>();

// Bumped when the on-disk cache is cleared. A response that started before the
// clear must not put its now-deleted path back into the map.
let generation = 0;

function requestKey(platform: ComicPlatform, url: string): string {
  return `${platform}\u0000${url}`;
}

function rememberImage(key: string, image: CachedImage) {
  resolvedImages.delete(key);
  resolvedImages.set(key, image);
  while (resolvedImages.size > RESOLVED_CACHE_LIMIT) {
    const oldest = resolvedImages.keys().next().value;
    if (oldest === undefined) {
      break;
    }
    resolvedImages.delete(oldest);
  }
}

/** Return a path that was already resolved for this image, if any. */
export function getCachedImage(
  platform: ComicPlatform,
  url: string,
): CachedImage | null {
  return resolvedImages.get(requestKey(platform, url)) ?? null;
}

/** Drop a stale resolved path after an image decode failure. */
export function forgetCachedImage(platform: ComicPlatform, url: string) {
  resolvedImages.delete(requestKey(platform, url));
}

/**
 * Drop every resolved path after the on-disk cache was cleared. Requests that
 * were already in flight settle normally but are not remembered.
 */
export function clearResolvedImages() {
  generation += 1;
  resolvedImages.clear();
  inFlight.clear();
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
  const resolved = resolvedImages.get(key);
  if (resolved) {
    return Promise.resolve(resolved);
  }
  const existing = inFlight.get(key);
  if (existing) {
    return existing;
  }

  const requestedGeneration = generation;
  const request = (async () => {
    const image = await invokeCommand<CachedImage>("fetch_image", {
      url,
      platform,
    });
    if (requestedGeneration === generation) {
      rememberImage(key, image);
    }
    return image;
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
