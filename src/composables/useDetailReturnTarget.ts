import { computed } from "vue";
import type { RouteLocationRaw } from "vue-router";
import { useAppI18n } from "../i18n";

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

function targetFor(origin: DetailOrigin): RouteLocationRaw {
  return { name: origin };
}

/** The explicit source wins; a deep link without a source returns home. */
export function detailBackTarget(
  origin: DetailOrigin | null,
): RouteLocationRaw {
  return origin ? targetFor(origin) : { name: "home" };
}

const LABEL_KEYS = {
  favorites: "detail.backToFavorites",
  history: "detail.backToHistory",
  downloads: "detail.backToDownloads",
  search: "detail.backToSearch",
  home: "detail.backHome",
} as const;

/**
 * Where the detail page's back button goes.
 *
 * The source is captured at setup and comes only from the explicit navigation
 * state written by the opener. It is deliberately not inferred from
 * `previousPath()` or the search session: the back target is the component
 * that opened this detail, not whichever page happens to sit below it in the
 * browser stack. A deep link has no source and therefore returns home.
 *
 * A reader pushed on top of this page cannot change the target.
 */
export function useDetailReturnTarget() {
  const { t } = useAppI18n();

  const source = readDetailOrigin(window.history.state);

  const backTarget = computed<RouteLocationRaw>(() =>
    detailBackTarget(source),
  );

  const backLabel = computed(() =>
    source ? t(LABEL_KEYS[source]) : t("detail.backHome"),
  );

  return { backTarget, backLabel };
}
