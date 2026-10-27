import type {
  ComicDetailBase,
  SearchItemBase,
} from "../types";

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

export type NhentaiSearchItem = SearchItemBase & {
  platform: "nhentai";
  extra: NhentaiExtra;
};

export type NhentaiDetail = ComicDetailBase & {
  platform: "nhentai";
  extra: NhentaiDetailExtra;
};
