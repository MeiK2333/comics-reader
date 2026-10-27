import type {
  ComicDetailBase,
  SearchItemBase,
} from "../types";

/** JM's APP search payload carries no tags, only the author and category. */
export interface JmExtra {
  tags: string[];
  author: string | null;
  category: string | null;
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

export type JmSearchItem = SearchItemBase & {
  platform: "jm";
  extra: JmExtra;
};

export type JmDetail = ComicDetailBase & {
  platform: "jm";
  extra: JmDetailExtra;
};
