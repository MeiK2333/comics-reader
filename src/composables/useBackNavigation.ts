import { useRouter } from "vue-router";
import type { RouteLocationRaw } from "vue-router";
import { backDeltaTo } from "../router/navigationStack";

/**
 * Vue Router stores the entry before the current one in `history.state.back`
 * (as a route path such as `/search`). Reading it lets a back button pop when
 * the previous page is already the parent it wants, instead of pushing a
 * duplicate that the system back button would later revisit.
 */
export function previousPath(): string | null {
  const back = (window.history.state as { back?: unknown } | null)?.back;
  if (typeof back !== "string" || back.length === 0) {
    return null;
  }

  if (back.startsWith("#")) {
    return back.slice(1) || "/";
  }
  if (back.startsWith("http://") || back.startsWith("https://")) {
    const url = new URL(back);
    return url.hash ? url.hash.slice(1) : url.pathname + url.search;
  }
  return back.startsWith("/") ? back : `/${back}`;
}

export function useBackNavigation() {
  const router = useRouter();

  /**
   * Leave the current page towards `target`.
   *
   * Popping is preferred because it keeps the history stack free of the page
   * being left; replacing is the fallback for deep links (no previous entry)
   * and for pages reached through an unexpected path, so back never lands on
   * the page the user just left.
   *
   * If the target is earlier but not immediately previous (for example
   * detail -> downloads -> home), walking back to it also removes the
   * intervening page instead of leaving it under the target.
   */
  function goBack(target: RouteLocationRaw) {
    const fullPath = router.resolve(target).fullPath;
    if (previousPath() === fullPath) {
      router.back();
      return;
    }

    const delta = backDeltaTo(fullPath);
    if (delta !== null && delta < 0) {
      router.go(delta);
      return;
    }

    void router.replace(target);
  }

  return { goBack };
}
