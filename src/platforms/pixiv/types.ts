import type { ComicDetailBase, SearchItemBase } from "../types";

/** One Pixiv tag plus the site-provided English translation, when present. */
export interface PixivTag {
  name: string;
  translated: string | null;
}

export interface PixivSearchExtra {
  userId: string;
  userName: string;
  /** 0 = illustration, 1 = manga, 2 = ugoira. */
  illustType: number;
  xRestrict: number;
  pageCount: number;
  width: number;
  height: number;
  tags: string[];
  /** ISO 8601 timestamp in Pixiv's response timezone. */
  createDate: string;
}

export interface PixivDetailExtra {
  userId: string;
  userName: string;
  userAccount: string;
  /** 0 = illustration, 1 = manga, 2 = ugoira. */
  illustType: number;
  xRestrict: number;
  pageCount: number;
  width: number;
  height: number;
  bookmarkCount: number | null;
  likeCount: number | null;
  viewCount: number | null;
  /** ISO 8601 timestamp. */
  createDate: string;
  description: string | null;
  seriesTitle: string | null;
  tags: PixivTag[];
}

export type PixivSearchItem = SearchItemBase & {
  platform: "pixiv";
  extra: PixivSearchExtra;
};

export type PixivDetail = ComicDetailBase & {
  platform: "pixiv";
  extra: PixivDetailExtra;
};
