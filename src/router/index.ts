import { createRouter, createWebHashHistory } from "vue-router";
import type { RouteMeta } from "vue-router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { translate } from "../i18n";
import { PLATFORM_ROUTE_PATTERN } from "../platforms";
import pinia from "../stores";
import { useSearchStore } from "../stores/search";
import HomeView from "../views/HomeView.vue";
import { recordNavigation } from "./navigationStack";

type RouteTitleKey =
  | "app.name"
  | "meta.search"
  | "meta.comic"
  | "meta.reader"
  | "meta.favorites"
  | "meta.downloads"
  | "meta.history"
  | "meta.settings"
  | "meta.logs"
  | "meta.notFound";

/**
 * Detail pages are the parent components of their reader route. The reader
 * reads these names instead of assuming a single detail/reader pair, so the
 * online and downloaded detail trees can stay separate.
 */
type DetailRouteName = "comic-detail" | "download-detail";
type ReaderRouteName = "comic-reader" | "download-reader";

declare module "vue-router" {
  interface RouteMeta {
    titleKey?: RouteTitleKey;
    detailRoute?: DetailRouteName;
    readerRoute?: ReaderRouteName;
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
      path: "/favorites",
      name: "favorites",
      component: () => import("../views/FavoritesView.vue"),
      meta: { titleKey: "meta.favorites" },
    },
    {
      path: "/downloads",
      name: "downloads",
      component: () => import("../views/DownloadsView.vue"),
      meta: { titleKey: "meta.downloads" },
    },
    {
      path: `/downloads/:platform(${PLATFORM_ROUTE_PATTERN})/:id`,
      name: "download-detail",
      component: () => import("../views/DownloadedComicDetailView.vue"),
      meta: {
        titleKey: "meta.comic",
        detailRoute: "download-detail",
        readerRoute: "download-reader",
      },
      children: [
        {
          path: "read/:chapterId?",
          name: "download-reader",
          component: () => import("../views/ComicReaderView.vue"),
          meta: {
            titleKey: "meta.reader",
            detailRoute: "download-detail",
            readerRoute: "download-reader",
          },
        },
      ],
    },
    {
      path: "/history",
      name: "history",
      component: () => import("../views/HistoryView.vue"),
      meta: { titleKey: "meta.history" },
    },
    {
      path: "/settings",
      name: "settings",
      component: () => import("../views/SettingsView.vue"),
      meta: { titleKey: "meta.settings" },
    },
    {
      path: "/settings/logs",
      name: "logs",
      component: () => import("../views/LogsView.vue"),
      meta: { titleKey: "meta.logs" },
    },
    {
      path: `/comic/:platform(${PLATFORM_ROUTE_PATTERN})/:id`,
      name: "comic-detail",
      component: () => import("../views/ComicDetailView.vue"),
      meta: {
        titleKey: "meta.comic",
        detailRoute: "comic-detail",
        readerRoute: "comic-reader",
      },
      // The reader is a child of the detail page, not a sibling route. The
      // detail component therefore stays mounted while the reader is open and
      // closing it does not reload the detail data.
      children: [
        {
          path: "read/:chapterId?",
          name: "comic-reader",
          component: () => import("../views/ComicReaderView.vue"),
          meta: {
            titleKey: "meta.reader",
            detailRoute: "comic-detail",
            readerRoute: "comic-reader",
          },
        },
      ],
    },
    {
      path: "/search",
      name: "search",
      component: () => import("../views/SearchView.vue"),
      meta: { titleKey: "meta.search" },
      // `/search` has no URL state, so a fresh load or manual visit has
      // nothing to show. Only in-session searches may enter the results view.
      beforeEnter: () => {
        const search = useSearchStore(pinia);
        return search.active ? true : { name: "home", replace: true };
      },
    },
    {
      path: "/:pathMatch(.*)*",
      name: "not-found",
      component: () => import("../views/NotFoundView.vue"),
      meta: { titleKey: "meta.notFound" },
    },
  ],
  // New pages start at the top; browser back/forward keeps the previous
  // position instead of dropping the user at a stale scroll offset.
  scrollBehavior: (_to, _from, savedPosition) =>
    savedPosition ?? { top: 0 },
});

function translateTitleKey(key: RouteTitleKey) {
  switch (key) {
    case "meta.notFound":
      return translate("meta.notFound", { app: translate("app.name") });
    case "meta.search":
    case "meta.comic":
    case "meta.reader":
    case "meta.favorites":
    case "meta.downloads":
    case "meta.history":
    case "meta.settings":
    case "meta.logs":
      return translate(key, { app: translate("app.name") });
    case "app.name":
      return translate("app.name");
  }
}

/**
 * Set the document and native window title. Exported for views whose title is
 * data-driven (comic details) and cannot come from static route metadata.
 */
export function setWindowTitle(title: string) {
  document.title = title;

  // The document title is not mirrored to the native window, so set it
  // explicitly. Android/iOS windows have no title and `setTitle` is a no-op
  // there, hence the swallowed rejection to stay safe on every platform.
  void getCurrentWindow()
    .setTitle(title)
    .catch(() => undefined);
}

/**
 * Set the title for a route. Called on every navigation and again when the
 * locale changes.
 */
export function updateWindowTitle(
  meta: RouteMeta = router.currentRoute.value.meta,
) {
  setWindowTitle(translateTitleKey(meta.titleKey ?? "app.name"));
}

router.afterEach((to) => {
  updateWindowTitle(to.meta);
  const position = (
    window.history.state as { position?: unknown } | null
  )?.position;
  recordNavigation(to.fullPath, position);
});

export default router;
