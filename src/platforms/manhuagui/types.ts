import type {
  ComicDetailBase,
  SearchItemBase,
} from "../types";

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
  /** Chapter groups in reading order; empty on search results. */
  chapterGroups: ManhuaguiChapterGroup[];
}

/** A site chapter group such as a volume or a one-shot collection. */
export interface ManhuaguiChapterGroup {
  /** Empty when the page did not label the group. */
  title: string;
  chapters: ManhuaguiChapterRef[];
}

/** One chapter of a manhuagui work. */
export interface ManhuaguiChapterRef {
  id: string;
  title: string;
}

export type ManhuaguiSearchItem = SearchItemBase & {
  platform: "manhuagui";
  extra: ManhuaguiExtra;
};

export type ManhuaguiDetail = ComicDetailBase & {
  platform: "manhuagui";
  extra: ManhuaguiExtra;
};
