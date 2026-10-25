import { createApp } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import App from "./App.vue";
import "./styles/base.scss";
import router from "./router";
import pinia from "./stores";
import { applyProxy } from "./api/proxy";
import { applyTheme } from "./composables/useTheme";
import { i18n, resolveLocale, setLocale } from "./i18n";
import { useSettingsStore } from "./stores/settings";

const app = createApp(App).use(pinia).use(i18n).use(router);

async function bootstrap() {
  // Reading the store hydrates persisted settings synchronously, so the saved
  // theme and language are known before the first render.
  const settings = useSettingsStore(pinia);
  applyTheme(settings.theme);
  setLocale(resolveLocale(settings.locale));

  // The proxy is runtime state in Rust. Restore it before mounting so the
  // first search or thumbnail cannot race ahead with a direct connection.
  if (isTauri()) {
    try {
      await applyProxy(settings.proxy);
    } catch (error) {
      // A stale or hand-edited setting should not keep the whole app from
      // starting; the settings page will report it when saved again.
      console.error("failed to apply the saved HTTP proxy", error);
    }
  }

  app.mount("#app");
}

void bootstrap();
