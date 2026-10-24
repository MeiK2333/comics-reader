import { createI18n } from "vue-i18n";
import type { PickupPaths } from "vue-i18n";
import en from "./messages/en.json";
import zhCN from "./messages/zh-CN.json";

/** Locales that have a translation catalog. Add a locale here first. */
export const SUPPORTED_LOCALES = ["en", "zh-CN"] as const;

export type AppLocale = (typeof SUPPORTED_LOCALES)[number];

/** Persisted preference: `system` follows the language reported by the WebView. */
export type LocalePreference = "system" | AppLocale;

/** English is the master schema; every other catalog must mirror its keys. */
export type MessageSchema = typeof en;

/** Dotted path of every message key, e.g. `home.search.submit`. */
export type MessageKey = PickupPaths<MessageSchema>;

type MessageParams = Record<string, string | number>;

export const FALLBACK_LOCALE: AppLocale = "en";

// Assigning each catalog to the English schema turns a missing key into a
// compile-time error, so a half-translated locale cannot ship unnoticed.
const messages: Record<AppLocale, MessageSchema> = {
  en,
  "zh-CN": zhCN,
};

function isAppLocale(value: string): value is AppLocale {
  return (SUPPORTED_LOCALES as readonly string[]).includes(value);
}

/**
 * Pick the best supported locale for the current system language. Only
 * Simplified Chinese is translated, so every `zh-*` variant maps to it.
 */
export function detectSystemLocale(): AppLocale {
  const languages = navigator.languages?.length
    ? navigator.languages
    : [navigator.language];

  for (const language of languages) {
    const tag = language.toLowerCase();
    if (tag.startsWith("zh")) {
      return "zh-CN";
    }
    if (tag.startsWith("en")) {
      return "en";
    }
  }

  return FALLBACK_LOCALE;
}

/** Resolve the persisted preference to a locale that has a catalog. */
export function resolveLocale(preference: LocalePreference): AppLocale {
  return preference !== "system" && isAppLocale(preference)
    ? preference
    : detectSystemLocale();
}

/**
 * Create the singleton i18n instance. It starts with the system language;
 * `main.ts` applies the persisted preference before the app mounts so the
 * first paint already uses the right language.
 */
export const i18n = createI18n({
  legacy: false,
  locale: detectSystemLocale(),
  fallbackLocale: FALLBACK_LOCALE,
  messages,
});

/** Switch the active locale and keep the document language in sync. */
export function setLocale(locale: AppLocale) {
  i18n.global.locale.value = locale;
  document.documentElement.lang = locale;
}

/**
 * Typed translation function. `vue-i18n`'s own `t()` accepts any string, so
 * narrowing its key to `MessageKey` turns a typo into a compile-time error.
 */
export function translate(key: MessageKey, params?: MessageParams): string {
  return params ? i18n.global.t(key, params) : i18n.global.t(key);
}

/**
 * Access the typed global translator from a component. Reading the global
 * composer's locale makes `t()` re-evaluate when the language changes.
 */
export function useAppI18n() {
  return { t: translate };
}
