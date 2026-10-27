import type {
  ComicDetailBase,
  SearchItemBase,
} from "../types";

/** Fields returned by copymanga search and detail. Detail fills the full set. */
export interface CopymangaExtra {
  originalTitle: string | null;
  authors: string[];
  status: string | null;
  latestChapter: string | null;
  updatedAt: string | null;
  region: string | null;
  categories: string[];
  aliases: string[];
  description: string | null;
  popularity: number | null;
  /** Chapter groups in reading order; empty on search results. */
  chapterGroups: CopymangaChapterGroup[];
}

/** A site chapter group such as the default serialization or a volume set. */
export interface CopymangaChapterGroup {
  /** Empty when the site did not label the group. */
  title: string;
  chapters: CopymangaChapterRef[];
}

/** One chapter of a copymanga work. `id` is the chapter UUID. */
export interface CopymangaChapterRef {
  id: string;
  title: string;
}

export type CopymangaSearchItem = SearchItemBase & {
  platform: "copymanga";
  extra: CopymangaExtra;
};

export type CopymangaDetail = ComicDetailBase & {
  platform: "copymanga";
  extra: CopymangaExtra;
};
