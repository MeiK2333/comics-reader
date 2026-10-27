import type { ComicPlatform } from "../api/comics";
import type { MessageKey } from "./index";

/**
 * Platform badge labels shared by result cards, detail shells and the list
 * pages. Keeping the map here avoids four copies drifting apart.
 */
export const PLATFORM_LABEL_KEYS: Record<ComicPlatform, MessageKey> = {
  manhuagui: "home.search.platforms.manhuagui",
  jm: "home.search.platforms.jm",
  nhentai: "home.search.platforms.nhentai",
};

export function platformLabelKey(platform: ComicPlatform): MessageKey {
  return PLATFORM_LABEL_KEYS[platform];
}
