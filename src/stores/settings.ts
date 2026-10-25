import { defineStore } from "pinia";
import type { LocalePreference } from "../i18n";

export type ThemeMode = "system" | "light" | "dark";

export interface ProxySettings {
  enabled: boolean;
  host: string;
  port: number;
}

export interface SettingsState {
  theme: ThemeMode;
  locale: LocalePreference;
  /** Applied to the Rust HTTP client on startup and after saving the form. */
  proxy: ProxySettings;
  /** Mirrored to Rust through set_ech_enabled; enabling clears any proxy. */
  ech: boolean;
}

export const useSettingsStore = defineStore("settings", {
  state: (): SettingsState => ({
    theme: "system",
    locale: "system",
    proxy: {
      enabled: false,
      host: "",
      port: 8080,
    },
    ech: false,
  }),
  // Persisted under `comics-reader:settings` (see `src/stores/index.ts`) and
  // rehydrated synchronously when the store is first used.
  persist: true,
});
