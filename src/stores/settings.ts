import { defineStore } from "pinia";
import type { LocalePreference } from "../i18n";

export type ThemeMode = "system" | "light" | "dark";

export interface SettingsState {
  theme: ThemeMode;
  locale: LocalePreference;
}

export const useSettingsStore = defineStore("settings", {
  state: (): SettingsState => ({
    theme: "system",
    locale: "system",
  }),
  // Persisted under `comics-reader:settings` (see `src/stores/index.ts`) and
  // rehydrated synchronously when the store is first used.
  persist: true,
});
