import { createRouter, createWebHashHistory } from "vue-router";
import type { RouteMeta } from "vue-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { translate } from "../i18n";
import HomeView from "../views/HomeView.vue";

type RouteTitleKey = "app.name" | "meta.notFound";

declare module "vue-router" {
  interface RouteMeta {
    titleKey?: RouteTitleKey;
  }
}

const router = createRouter({
  // Hash history does not rely on the webview re-serving index.html for a
  // route path, so it behaves the same on the web, desktop and mobile.
  history: createWebHashHistory(),
  routes: [
    {
      path: "/",
      name: "home",
      component: HomeView,
      meta: { titleKey: "app.name" },
    },
    {
      path: "/:pathMatch(.*)*",
      name: "not-found",
      component: () => import("../views/NotFoundView.vue"),
      meta: { titleKey: "meta.notFound" },
    },
  ],
});

function translateTitleKey(key: RouteTitleKey) {
  switch (key) {
    case "meta.notFound":
      return translate("meta.notFound", { app: translate("app.name") });
    case "app.name":
      return translate("app.name");
  }
}

/**
 * Set the document and native window title for a route. Called on every
 * navigation and again when the locale changes.
 */
export function updateWindowTitle(
  meta: RouteMeta = router.currentRoute.value.meta,
) {
  const title = translateTitleKey(meta.titleKey ?? "app.name");
  document.title = title;

  // The document title is not mirrored to the native window, so set it
  // explicitly. Android/iOS windows have no title and `setTitle` is a no-op
  // there, hence the swallowed rejection to stay safe on every platform.
  void getCurrentWindow()
    .setTitle(title)
    .catch(() => undefined);
}

router.afterEach((to) => updateWindowTitle(to.meta));

export default router;
