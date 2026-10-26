import { defineStore } from "pinia";
import type { LocalePreference } from "../i18n";

export type ThemeMode = "system" | "light" | "dark";

/** Reading order shared by the reader's layout, swipe and arrow-key mapping. */
export type ReaderDirection = "ltr" | "rtl" | "ttb";

/** Number of upcoming reader pages warmed in the image cache. */
export const READER_PRELOAD_MIN = 3;
export const READER_PRELOAD_MAX = 10;
export const READER_PRELOAD_DEFAULT = 3;

export function clampReaderPreloadCount(value: number): number {
  if (!Number.isFinite(value)) {
    return READER_PRELOAD_DEFAULT;
  }
  return Math.min(
    READER_PRELOAD_MAX,
    Math.max(READER_PRELOAD_MIN, Math.round(value)),
  );
}

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
  /**
   * How many pages after the current one the reader warms in the image cache.
   * This is a read-ahead window, not a disk-cache eviction limit.
   */
  readerPreloadCount: number;
  /**
   * Whether reaching the last page of a chapter warms the next chapter's first
   * images. This does not change navigation; the page-turn boundary still
   * switches chapters normally.
   */
  readerPreloadNextChapter: boolean;
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
    readerPreloadCount: READER_PRELOAD_DEFAULT,
    readerPreloadNextChapter: true,
  }),
  // Persisted under `comics-reader:settings` (see `src/stores/index.ts`) and
  // rehydrated synchronously when the store is first used.
  persist: true,
});
