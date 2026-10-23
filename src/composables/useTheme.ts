import { computed, onScopeDispose, ref, watchEffect } from "vue";
import { storeToRefs } from "pinia";
import { useSettingsStore } from "../stores/settings";
import type { ThemeMode } from "../stores/settings";

const DARK_QUERY = "(prefers-color-scheme: dark)";

function systemPrefersDark() {
  return window.matchMedia(DARK_QUERY).matches;
}

/** Resolve a theme setting to the concrete theme that should be displayed. */
export function resolveTheme(theme: ThemeMode): "light" | "dark" {
  if (theme === "system") {
    return systemPrefersDark() ? "dark" : "light";
  }

  return theme;
}

/**
 * Apply the theme to the document. Called before the app mounts so the first
 * paint already uses the persisted theme.
 */
export function applyTheme(theme: ThemeMode) {
  document.documentElement.dataset.theme = resolveTheme(theme);
}

/** Keep the document theme in sync with the persisted setting. */
export function useTheme() {
  const { theme } = storeToRefs(useSettingsStore());

  const media = window.matchMedia(DARK_QUERY);
  const prefersDark = ref(media.matches);
  const onMediaChange = (event: MediaQueryListEvent) => {
    prefersDark.value = event.matches;
  };

  media.addEventListener("change", onMediaChange);
  onScopeDispose(() => media.removeEventListener("change", onMediaChange));

  const isDark = computed(
    () =>
      theme.value === "dark" ||
      (theme.value === "system" && prefersDark.value),
  );

  watchEffect(() => {
    document.documentElement.dataset.theme = isDark.value ? "dark" : "light";
  });

  return { theme, isDark };
}
