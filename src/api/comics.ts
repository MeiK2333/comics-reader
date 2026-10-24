import { invoke } from "@tauri-apps/api/core";

/** Platforms that implement keyword search and exact-id detail lookups. */
export type ComicPlatform = "manhuagui" | "jm" | "nhentai";

/** Search fields returned by manhuagui. `null` means the upstream value is absent. */
export interface ManhuaguiExtra {
  originalTitle: string | null;
  authors: string[];
  status: string | null;
  latestChapter: string | null;
  updatedAt: string | null;
  year: string | null;
  region: string | null;
  categories: string[];
  aliases: string[];
  description: string | null;
  score: number | null;
  ratingCount: number | null;
}

export interface JmExtra {
  tags: string[];
}

export interface NhentaiExtra {
  mediaId: string;
  japaneseTitle: string | null;
  thumbnailWidth: number | null;
  thumbnailHeight: number | null;
  numPages: number | null;
  numFavorites: number | null;
  tagIds: number[];
  blacklisted: boolean | null;
}

interface SearchItemBase {
  id: string;
  title: string;
  thumb: string;
}

export type ManhuaguiSearchItem = SearchItemBase & {
  platform: "manhuagui";
  extra: ManhuaguiExtra;
};

export type JmSearchItem = SearchItemBase & {
  platform: "jm";
  extra: JmExtra;
};

export type NhentaiSearchItem = SearchItemBase & {
  platform: "nhentai";
  extra: NhentaiExtra;
};

/**
 * `platform` is the discriminant: each branch carries the exact fields its
 * parser produced, so cards never read an unstructured JSON bag.
 */
export type ComicSearchItem =
  | ManhuaguiSearchItem
  | JmSearchItem
  | NhentaiSearchItem;

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
