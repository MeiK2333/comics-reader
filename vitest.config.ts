import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
import Components from "unplugin-vue-components/vite";
import { NaiveUiResolver } from "unplugin-vue-components/resolvers";

export default defineConfig({
  plugins: [
    vue(),
    // Resolve Naive UI components from templates exactly like the app build.
    // `dts: false` keeps tests from rewriting the generated components.d.ts.
    Components({ dts: false, resolvers: [NaiveUiResolver()] }),
  ],

  // Kept in sync with vite.config.ts: vue-i18n's bundler build needs the same
  // feature flags, otherwise tests exercise a different runtime branch.
  define: {
    __VUE_I18N_FULL_INSTALL__: false,
    __VUE_I18N_LEGACY_API__: false,
    __INTLIFY_PROD_DEVTOOLS__: false,
  },

  test: {
    environment: "jsdom",
    setupFiles: ["src/test/setup.ts"],
    include: ["src/**/*.test.ts"],
    clearMocks: true,
  },
});
