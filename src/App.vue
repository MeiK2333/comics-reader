<script setup lang="ts">
import { computed } from "vue";
import { RouterView } from "vue-router";
import { darkTheme } from "naive-ui";
import { useLocale } from "./composables/useLocale";
import { useTheme } from "./composables/useTheme";
import { darkThemeOverrides, lightThemeOverrides } from "./theme/naive";

const { isDark } = useTheme();
const { naiveLocale, naiveDateLocale } = useLocale();

// Naive UI needs its own theme object; `useTheme` stays the single source of
// truth for the light/dark preference.
const naiveTheme = computed(() => (isDark.value ? darkTheme : null));
const naiveThemeOverrides = computed(() =>
  isDark.value ? darkThemeOverrides : lightThemeOverrides,
);
</script>

<template>
  <NConfigProvider
    :theme="naiveTheme"
    :theme-overrides="naiveThemeOverrides"
    :locale="naiveLocale"
    :date-locale="naiveDateLocale"
  >
    <NMessageProvider>
      <RouterView />
    </NMessageProvider>
  </NConfigProvider>
</template>

<style>
:root {
  color-scheme: light;

  --color-text: #0f0f0f;
  --color-bg: #f6f6f6;
  --color-surface: #ffffff;
  --color-surface-active: #e8e8e8;
  --color-accent: #396cd8;
  --color-link-hover: #535bf2;

  font-family:
    Inter, Avenir, "Microsoft YaHei", "PingFang SC", "Noto Sans SC", Helvetica,
    Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;

  color: var(--color-text);
  background-color: var(--color-bg);

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

:root[data-theme="dark"] {
  color-scheme: dark;

  --color-text: #f6f6f6;
  --color-bg: #2f2f2f;
  --color-surface: #0f0f0f98;
  --color-surface-active: #0f0f0f69;
  --color-link-hover: #24c8db;
}

.container {
  margin: 0;
  padding-top: 10vh;
  display: flex;
  flex-direction: column;
  justify-content: center;
  text-align: center;
}

.logo {
  height: 6em;
  padding: 1.5em;
  will-change: filter;
  transition: 0.75s;
}

.logo.tauri:hover {
  filter: drop-shadow(0 0 2em #24c8db);
}

.row {
  display: flex;
  justify-content: center;
}

a {
  font-weight: 500;
  color: #646cff;
  text-decoration: inherit;
}

a:hover {
  color: var(--color-link-hover);
}

h1 {
  text-align: center;
}
</style>
