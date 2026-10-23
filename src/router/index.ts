import { createRouter, createWebHashHistory } from "vue-router";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import HomeView from "../views/HomeView.vue";

declare module "vue-router" {
  interface RouteMeta {
    title?: string;
  }
}

const DEFAULT_TITLE = "Comics Reader";

const router = createRouter({
  // Hash history does not rely on the webview re-serving index.html for a
  // route path, so it behaves the same on the web, desktop and mobile.
  history: createWebHashHistory(),
  routes: [
    {
      path: "/",
      name: "home",
      component: HomeView,
      meta: { title: DEFAULT_TITLE },
    },
    {
      path: "/:pathMatch(.*)*",
      name: "not-found",
      component: () => import("../views/NotFoundView.vue"),
      meta: { title: `Page Not Found · ${DEFAULT_TITLE}` },
    },
  ],
});

router.afterEach((to) => {
  const title = to.meta.title ?? DEFAULT_TITLE;
  document.title = title;

  // The document title is not mirrored to the native window, so set it
  // explicitly. Android/iOS windows have no title and `setTitle` is a no-op
  // there, hence the swallowed rejection to stay safe on every platform.
  if (isTauri()) {
    void getCurrentWindow()
      .setTitle(title)
      .catch(() => undefined);
  }
});

export default router;
