import { computed } from "vue";
import type { RouteLocationRaw } from "vue-router";
import { useSearchStore } from "../stores/search";
import { useAppI18n } from "../i18n";
import { previousPath } from "./useBackNavigation";

export type DetailOrigin =
  | "favorites"
  | "history"
  | "downloads"
  | "search"
  | "home";

const ORIGINS: readonly DetailOrigin[] = [
  "favorites",
  "history",
  "downloads",
  "search",
  "home",
];

function isDetailOrigin(value: unknown): value is DetailOrigin {
  return typeof value === "string" && ORIGINS.includes(value as DetailOrigin);
}

/** Read the explicit `state.from` written by the entry page. */
export function readDetailOrigin(state: unknown): DetailOrigin | null {
  if (!state || typeof state !== "object") {
    return null;
  }
  const from = (state as { from?: unknown }).from;
  return isDetailOrigin(from) ? from : null;
}

/** Legacy fallback for entries created before `state.from` existed. */
export function detailOriginFromPath(path: string | null): DetailOrigin | null {
  switch (path) {
    case "/favorites":
      return "favorites";
    case "/history":
      return "history";
    case "/downloads":
      return "downloads";
    default:
      return null;
  }
}

function targetFor(origin: DetailOrigin): RouteLocationRaw {
  return { name: origin };
}

const LABEL_KEYS = {
  favorites: "detail.backToFavorites",
  history: "detail.backToHistory",
  downloads: "detail.backToDownloads",
  search: "detail.backToSearch",
  home: "detail.backHome",
} as const;

/**
 * Where the detail page's back button goes. The source is captured at setup so
 * a reader pushed on top of this page cannot change it. An explicit
 * `state.from` wins over the previous history entry; the latter only supports
 * links created before the state was added.
 */
export function useDetailReturnTarget() {
  const search = useSearchStore();
  const { t } = useAppI18n();

  const source =
    readDetailOrigin(window.history.state) ?? detailOriginFromPath(previousPath());
  const canReturnToSearch = computed(() => search.active);

  const backTarget = computed<RouteLocationRaw>(() => {
    if (source) {
      return targetFor(source);
    }
    return canReturnToSearch.value ? { name: "search" } : { name: "home" };
  });

  const backLabel = computed(() => {
    if (source) {
      return t(LABEL_KEYS[source]);
    }
    return t(
      canReturnToSearch.value ? "detail.backToSearch" : "detail.backHome",
    );
  });

  return { backTarget, backLabel };
}
