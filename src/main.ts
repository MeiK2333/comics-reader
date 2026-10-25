import { createApp } from "vue";
import { isTauri } from "@tauri-apps/api/core";
import App from "./App.vue";
import "./styles/base.scss";
import router from "./router";
import pinia from "./stores";
import { beginEchStartup, setEchEnabled } from "./api/ech";
import { applyProxy } from "./api/proxy";
import { setLoggingEnabled } from "./api/logs";
import { applyTheme } from "./composables/useTheme";
import { i18n, resolveLocale, setLocale } from "./i18n";
import { useSettingsStore } from "./stores/settings";
import { useRuntimeStore } from "./stores/runtime";

const app = createApp(App).use(pinia).use(i18n).use(router);

async function bootstrap() {
  // Reading the store hydrates persisted settings synchronously, so the saved
  // theme and language are known before the first render.
  const settings = useSettingsStore(pinia);
  const runtime = useRuntimeStore(pinia);
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
    // Logging is also runtime state. Enable it before the network setup so
    // proxy and ECH startup failures are recorded when the user opted in.
    if (settings.logging) {
      try {
        await setLoggingEnabled(true);
      } catch (error) {
        settings.logging = false;
        console.error("failed to enable logging", error);
      }
    }

    try {
      await applyProxy(settings.proxy);
    } catch (error) {
      // A stale or hand-edited setting should not keep the whole app from
      // starting; the settings page will report it when saved again.
      console.error("failed to apply the saved HTTP proxy", error);
    }

    if (settings.ech) {
      // Bootstrap performs network I/O. Keep the home page waiting until this
      // settles so the first search cannot fall back to the ordinary client.
      // A failed initialization rolls the persisted switch back.
      runtime.echStarting = true;
      const startup = setEchEnabled(true).catch((error) => {
        console.error("failed to initialize ECH", error);
        settings.ech = false;
      });
      beginEchStartup(startup);
      void startup.finally(() => {
        runtime.echStarting = false;
      });
    }
  }

  app.mount("#app");
}

void bootstrap();
