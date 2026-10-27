import type { AppLocale } from "../i18n";

export type PlatformMessages = Record<string, string>;

/** One chapter in a work's reading order; `null` id/title is a single-chapter gallery. */
export interface DetailChapter {
  id: string | null;
  title: string | null;
}

export interface SearchItemBase {
  id: string;
  title: string;
  thumb: string;
}

export interface ComicDetailBase {
  title: string;
  thumb: string;
  chapters: DetailChapter[];
}

/** Metadata shared by the picker, the dispatchers and the router. */
export interface PlatformDefinition<Id extends string> {
  id: Id;
  messages: Record<AppLocale, PlatformMessages>;
  icon: string;
  iconStyle: "plain" | "dark-chip";
  /** Single-chapter galleries ignore the chapter id in downloads and the reader. */
  singleChapter: boolean;
}

export function definePlatform<Id extends string>(
  definition: PlatformDefinition<Id>,
): PlatformDefinition<Id> {
  return definition;
}
