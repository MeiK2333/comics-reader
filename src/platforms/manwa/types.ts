import type { ComicDetailBase, SearchItemBase } from "../types";

/**
 * Fields shared by manwa search cards and the detail page. Search cards only
 * expose the author, status and description; the detail page fills the rest.
 */
export interface ManwaExtra {
  authors: string[];
  status: string | null;
  latestChapter: string | null;
  updatedAt: string | null;
  region: string | null;
  categories: string[];
  tags: string[];
  aliases: string[];
  subscribers: string | null;
  description: string | null;
}

export type ManwaSearchItem = SearchItemBase & {
  platform: "manwa";
  extra: ManwaExtra;
};

export type ManwaDetail = ComicDetailBase & {
  platform: "manwa";
  extra: ManwaExtra;
};
