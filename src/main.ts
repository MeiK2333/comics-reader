import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";
import pinia from "./stores";
import { applyTheme } from "./composables/useTheme";
import { useSettingsStore } from "./stores/settings";

const app = createApp(App).use(pinia).use(router);

// Reading the store hydrates persisted settings synchronously, so the saved
// theme is known before the first render.
applyTheme(useSettingsStore(pinia).theme);

app.mount("#app");
