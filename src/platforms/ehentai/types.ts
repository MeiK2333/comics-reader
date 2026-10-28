import type { ComicDetailBase, SearchItemBase } from "../types";

export interface EhentaiTag {
  /** E-Hentai namespace, such as language, artist, parody or female. */
  namespace: string;
  name: string;
}

export interface EhentaiSearchExtra {
  category: string;
  languages: string[];
  pages: number | null;
}

export interface EhentaiDetailExtra {
  gid: number;
  token: string;
  titleJpn: string | null;
  category: string;
  uploader: string | null;
  /** Unix timestamp in seconds. */
  posted: number | null;
  pages: number | null;
  /** Bytes. */
  fileSize: number | null;
  rating: number | null;
  torrentCount: number | null;
  expunged: boolean;
  languages: string[];
  tags: EhentaiTag[];
}

export type EhentaiSearchItem = SearchItemBase & {
  platform: "ehentai";
  extra: EhentaiSearchExtra;
};

export type EhentaiDetail = ComicDetailBase & {
  platform: "ehentai";
  extra: EhentaiDetailExtra;
};
