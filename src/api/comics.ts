import { invoke } from "@tauri-apps/api/core";

/** Platforms that implement keyword search and exact-id detail lookups. */
export type ComicPlatform = "manhuagui" | "jm" | "nhentai";

export interface ComicSearchItem {
  id: string;
  title: string;
  thumb: string;
  /** Platform-specific fields; narrow this at the call site. */
  extra: Record<string, unknown>;
}

export interface ComicSearchResult {
  total: number;
  page_size: number;
  result: ComicSearchItem[];
}

export interface ComicDetail {
  title: string;
  thumb: string;
  /** Platform-specific fields; narrow this at the call site. */
  extra: Record<string, unknown>;
}

/**
 * Search a platform by keyword. `page` is 1-based.
 */
export function searchComics(
  platform: ComicPlatform,
  query: string,
  page = 1,
): Promise<ComicSearchResult> {
  return invoke<ComicSearchResult>("search_comics", { platform, query, page });
}

/**
 * Fetch exact comic metadata by the id returned from search.
 */
export function getComic(
  platform: ComicPlatform,
  id: string,
): Promise<ComicDetail> {
  return invoke<ComicDetail>("get_comic", { platform, id });
}
