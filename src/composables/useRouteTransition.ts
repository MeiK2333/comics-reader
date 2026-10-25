import { nextTick } from "vue";
import { useRouter } from "vue-router";
import type { RouteLocationRaw } from "vue-router";

/**
 * Navigate with a same-document view transition when the webview supports it.
 *
 * The outgoing page snapshot stays on screen while the new route renders, so
 * shared elements (the search button and home control) animate across the
 * route change without delaying the actual navigation.
 */
export function useRouteTransition() {
  const router = useRouter();

  async function navigate(to: RouteLocationRaw) {
    const reduceMotion = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;

    if (reduceMotion || typeof document.startViewTransition !== "function") {
      await router.push(to);
      return;
    }

    const transition = document.startViewTransition(async () => {
      await router.push(to);
      await nextTick();
    });

    await transition.finished.catch(() => undefined);
  }

  return { navigate };
}
