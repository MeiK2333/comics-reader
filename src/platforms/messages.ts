import { computed } from "vue";
import { i18n } from "../i18n";
import type { AppLocale } from "../i18n";

function interpolate(
  template: string,
  params?: Record<string, string | number>,
): string {
  if (!params) {
    return template;
  }
  return template.replace(/\{(\w+)\}/g, (match, key: string) =>
    key in params ? String(params[key]) : match,
  );
}

/**
 * Typed translator for a platform's own copy. The catalog is required to have
 * the same keys in every locale, so a missing translation is a compile error.
 */
export function usePlatformText<
  M extends Record<AppLocale, Record<string, string>>,
>(messages: M) {
  const locale = computed(() => i18n.global.locale.value as AppLocale);
  return (
    key: keyof M["en"],
    params?: Record<string, string | number>,
  ): string => {
    const catalog = (messages[locale.value] ??
      messages.en) as Record<string, string>;
    const fallback = messages.en as Record<string, string>;
    return interpolate(
      catalog[String(key)] ?? fallback[String(key)],
      params,
    );
  };
}
