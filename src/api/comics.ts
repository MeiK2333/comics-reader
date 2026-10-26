import { invoke } from "@tauri-apps/api/core";
import { waitForEchStartup } from "./ech";

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
  /** Chapters in reading order; empty on search results. */
  chapters: ManhuaguiChapterRef[];
}

/** One chapter of a manhuagui work. */
export interface ManhuaguiChapterRef {
  id: string;
  title: string;
}

/** JM's APP search payload carries no tags, only the author and category. */
export interface JmExtra {
  tags: string[];
  author: string | null;
  category: string | null;
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

/** One chapter of a jm album, in reading order. */
export interface JmChapterRef {
  id: string;
  title: string;
  /** JM's 1-based chapter number. */
  sort: number;
}

/** Detail fields for jm albums, which search results do not expose. */
export interface JmDetailExtra {
  authors: string[];
  tags: string[];
  works: string[];
  actors: string[];
  pageCount: number | null;
  pubDate: string | null;
  updateDate: string | null;
  likes: string | null;
  views: string | null;
  commentCount: number | null;
  chapters: JmChapterRef[];
}

export interface NhentaiTag {
  id: number;
  /** API `type`: tag, language, artist, character, parody, group or category. */
  type: string;
  name: string;
  slug: string;
  count: number | null;
}

/** Detail fields for nhentai galleries, grouped by tag type in the UI. */
export interface NhentaiDetailExtra {
  mediaId: string;
  japaneseTitle: string | null;
  prettyTitle: string | null;
  numPages: number | null;
  numFavorites: number | null;
  scanlator: string | null;
  /** Unix timestamp in seconds. */
  uploadDate: number | null;
  cover: string | null;
  coverWidth: number | null;
  coverHeight: number | null;
  tags: NhentaiTag[];
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

interface ComicDetailBase {
  title: string;
  thumb: string;
}

export type ManhuaguiDetail = ComicDetailBase & {
  platform: "manhuagui";
  extra: ManhuaguiExtra;
};

export type JmDetail = ComicDetailBase & {
  platform: "jm";
  extra: JmDetailExtra;
};

export type NhentaiDetail = ComicDetailBase & {
  platform: "nhentai";
  extra: NhentaiDetailExtra;
};

/**
 * `platform` is the discriminant: each detail branch carries the exact fields
 * its parser produced, so detail views never read an unstructured JSON bag.
 */
export type ComicDetail = ManhuaguiDetail | JmDetail | NhentaiDetail;

/** Runtime guard for route params and persisted/navigated platform values. */
export function isComicPlatform(value: unknown): value is ComicPlatform {
  return value === "manhuagui" || value === "jm" || value === "nhentai";
}

/**
 * Search a platform by keyword. `page` is 1-based.
 */
export async function searchComics(
  platform: ComicPlatform,
  query: string,
  page = 1,
): Promise<ComicSearchResult> {
  await waitForEchStartup();
  return invoke<ComicSearchResult>("search_comics", { platform, query, page });
}

/**
 * Fetch exact comic metadata by the id returned from search.
 */
export async function getComic(
  platform: ComicPlatform,
  id: string,
): Promise<ComicDetail> {
  await waitForEchStartup();
  return invoke<ComicDetail>("get_comic", { platform, id });
}
