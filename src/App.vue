<script setup lang="ts">
import { computed } from "vue";
import { RouterView } from "vue-router";
import { darkTheme } from "naive-ui";
import { useLocale } from "./composables/useLocale";
import { useTheme } from "./composables/useTheme";

const { isDark } = useTheme();
const { naiveLocale, naiveDateLocale } = useLocale();

// Naive UI needs its own theme object; `useTheme` stays the single source of
// truth for the light/dark preference.
const naiveTheme = computed(() => (isDark.value ? darkTheme : null));
</script>

<template>
  <NConfigProvider
    :theme="naiveTheme"
    :locale="naiveLocale"
    :date-locale="naiveDateLocale"
  >
    <NMessageProvider>
      <!-- Shared by every route: the strip occupies the top safe area and
           stays above content while the page scrolls. -->
      <div class="safe-area-top" aria-hidden="true"></div>
      <RouterView />
    </NMessageProvider>
  </NConfigProvider>
</template>

<style></style>
