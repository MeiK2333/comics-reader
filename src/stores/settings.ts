import { defineStore } from "pinia";
import type { LocalePreference } from "../i18n";

export type ThemeMode = "system" | "light" | "dark";

/** Reading order shared by the reader's layout, swipe and arrow-key mapping. */
export type ReaderDirection = "ltr" | "rtl" | "ttb";

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
  /** Mirrored to Rust through set_logging_enabled; writes to the app log file. */
  logging: boolean;
  /** Layout and navigation direction used by the comic reader. */
  readerDirection: ReaderDirection;
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
    logging: false,
    readerDirection: "ltr",
  }),
  // Persisted under `comics-reader:settings` (see `src/stores/index.ts`) and
  // rehydrated synchronously when the store is first used.
  persist: true,
});
