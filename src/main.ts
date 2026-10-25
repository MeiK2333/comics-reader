import { createApp } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import App from "./App.vue";
import "./styles/base.scss";
import router from "./router";
import pinia from "./stores";
import { setEchEnabled } from "./api/ech";
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

  // ECH is mutually exclusive with the custom proxy. Resolve a persisted
  // conflict before the first request can be issued.
  if (settings.ech && settings.proxy.enabled) {
    settings.proxy = { ...settings.proxy, enabled: false };
  }

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

    if (settings.ech) {
      // Bootstrap performs network I/O. Start it before mounting so the first
      // request usually finds the runtime ready, but do not delay the window:
      // a failed initialization rolls the persisted switch back.
      void setEchEnabled(true).catch((error) => {
        console.error("failed to initialize ECH", error);
        settings.ech = false;
      });
    }
  }

  app.mount("#app");
}

void bootstrap();
