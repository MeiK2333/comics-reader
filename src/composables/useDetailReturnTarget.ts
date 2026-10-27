import { computed } from "vue";
import type { RouteLocationRaw } from "vue-router";
import { useSearchStore } from "../stores/search";
import { useAppI18n } from "../i18n";
import { previousPath } from "./useBackNavigation";

/**
 * Where the detail page's back button goes. The source is captured at setup so
 * a reader pushed on top of this page cannot change it.
 */
export function useDetailReturnTarget() {
  const search = useSearchStore();
  const { t } = useAppI18n();

  const previous = previousPath();
  const cameFromFavorites = previous === "/favorites";
  const cameFromHistory = previous === "/history";
  const cameFromDownloads = previous === "/downloads";
  const canReturnToSearch = computed(() => search.active);

  const backTarget = computed<RouteLocationRaw>(() => {
    if (cameFromFavorites) {
      return { name: "favorites" };
    }
    if (cameFromHistory) {
      return { name: "history" };
    }
    if (cameFromDownloads) {
      return { name: "downloads" };
    }
    return canReturnToSearch.value ? { name: "search" } : { name: "home" };
  });

  const backLabel = computed(() => {
    if (cameFromFavorites) {
      return t("detail.backToFavorites");
    }
    if (cameFromHistory) {
      return t("detail.backToHistory");
    }
    if (cameFromDownloads) {
      return t("detail.backToDownloads");
    }
    return t(
      canReturnToSearch.value ? "detail.backToSearch" : "detail.backHome",
    );
  });

  return { backTarget, backLabel };
}
