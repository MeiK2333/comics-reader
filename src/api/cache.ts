import { invokeLocal } from "./client";
import { clearResolvedImages } from "./images";

/** Size of the disposable image cache. Mirrors Rust `CacheUsage`. */
export interface CacheUsage {
  files: number;
  bytes: number;
}

/** Measure the cached thumbnails and reader pages. */
export function getImageCacheUsage(): Promise<CacheUsage> {
  return invokeLocal("get_image_cache_usage");
}

/**
 * Delete every cached thumbnail and reader page. Downloaded works live in the
 * app data directory and are untouched by this command.
 *
 * The in-memory resolved paths are dropped as well so the next lookup does not
 * hand the webview a file that no longer exists. Returns what was freed.
 */
export async function clearImageCache(): Promise<CacheUsage> {
  const freed = await invokeLocal<CacheUsage>("clear_image_cache");
  clearResolvedImages();
  return freed;
}
