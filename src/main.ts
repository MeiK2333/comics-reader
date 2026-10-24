import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";
import pinia from "./stores";
import { applyTheme } from "./composables/useTheme";
import { i18n, resolveLocale, setLocale } from "./i18n";
import { useSettingsStore } from "./stores/settings";

const app = createApp(App).use(pinia).use(i18n).use(router);

// Reading the store hydrates persisted settings synchronously, so the saved
// theme and language are known before the first render.
const settings = useSettingsStore(pinia);
applyTheme(settings.theme);
setLocale(resolveLocale(settings.locale));

app.mount("#app");
