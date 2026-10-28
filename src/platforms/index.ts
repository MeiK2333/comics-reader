import { copymanga } from "./copymanga";
import type {
  CopymangaDetail,
  CopymangaSearchItem,
} from "./copymanga/types";
import { ehentai } from "./ehentai";
import type { EhentaiDetail, EhentaiSearchItem } from "./ehentai/types";
import { jm } from "./jm";
import type { JmDetail, JmSearchItem } from "./jm/types";
import { manhuagui } from "./manhuagui";
import type { ManhuaguiDetail, ManhuaguiSearchItem } from "./manhuagui/types";
import { nhentai } from "./nhentai";
import type { NhentaiDetail, NhentaiSearchItem } from "./nhentai/types";
import { i18n } from "../i18n";
import type { AppLocale } from "../i18n";

/**
 * The single registry consumed by the picker, the result/detail dispatchers
 * and the router. Adding a platform means adding its directory and one entry
 * here.
 */
export const platformList = [
  jm,
  nhentai,
  copymanga,
  manhuagui,
  ehentai,
] as const;

export type ComicPlatform = (typeof platformList)[number]["id"];

export type ComicSearchItem =
  | ManhuaguiSearchItem
  | JmSearchItem
  | NhentaiSearchItem
  | CopymangaSearchItem
  | EhentaiSearchItem;

export type ComicDetail =
  | ManhuaguiDetail
  | JmDetail
  | NhentaiDetail
  | CopymangaDetail
  | EhentaiDetail;

export interface ComicSearchResult {
  total: number;
  page_size: number;
  result: ComicSearchItem[];
  /** Cursor-paginated platforms use this instead of an exact total. */
  has_next?: boolean;
}

export const PLATFORM_ROUTE_PATTERN = platformList
  .map((platform) => platform.id)
  .join("|");

export const DEFAULT_PLATFORM: ComicPlatform = "jm";

export function isComicPlatform(value: unknown): value is ComicPlatform {
  return (
    typeof value === "string" &&
    platformList.some((platform) => platform.id === value)
  );
}

export function platformFor(id: ComicPlatform): (typeof platformList)[number] {
  const platform = platformList.find((candidate) => candidate.id === id);
  if (!platform) {
    throw new Error(`unknown platform: ${id}`);
  }
  return platform;
}

export function platformLabel(id: ComicPlatform): string {
  const locale = i18n.global.locale.value as AppLocale;
  const messages = platformFor(id).messages;
  return messages[locale]?.label ?? messages.en.label;
}

export type { ComicDetailBase, DetailChapter, SearchItemBase } from "./types";
export type { CopymangaDetail, CopymangaSearchItem } from "./copymanga/types";
export type { EhentaiDetail, EhentaiSearchItem } from "./ehentai/types";
export type { JmDetail, JmSearchItem } from "./jm/types";
export type { ManhuaguiDetail, ManhuaguiSearchItem } from "./manhuagui/types";
export type { NhentaiDetail, NhentaiSearchItem } from "./nhentai/types";
