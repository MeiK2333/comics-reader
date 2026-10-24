import { computed, watch } from "vue";
import { storeToRefs } from "pinia";
import { dateEnUS, dateZhCN, enUS, zhCN } from "naive-ui";
import type { NDateLocale, NLocale } from "naive-ui";
import { resolveLocale, setLocale } from "../i18n";
import type { AppLocale } from "../i18n";
import { updateWindowTitle } from "../router";
import { useSettingsStore } from "../stores/settings";

// Naive UI keeps its own built-in copy (component labels, empty states, date
// pickers), so it has to follow the same resolved locale as our catalogs.
const NAIVE_LOCALES: Record<
  AppLocale,
  { locale: NLocale; dateLocale: NDateLocale }
> = {
  en: { locale: enUS, dateLocale: dateEnUS },
  "zh-CN": { locale: zhCN, dateLocale: dateZhCN },
};

/**
 * Keep the runtime locale, the document language, Naive UI and the window
 * title in sync with the persisted locale preference. Mounted once in
 * `App.vue`; `main.ts` applies the initial value before the first render.
 */
export function useLocale() {
  const { locale } = storeToRefs(useSettingsStore());
  const resolvedLocale = computed(() => resolveLocale(locale.value));

  watch(
    resolvedLocale,
    (next) => {
      setLocale(next);
      updateWindowTitle();
    },
    { immediate: true },
  );

  const naiveLocale = computed(
    () => NAIVE_LOCALES[resolvedLocale.value].locale,
  );
  const naiveDateLocale = computed(
    () => NAIVE_LOCALES[resolvedLocale.value].dateLocale,
  );

  return { locale, resolvedLocale, naiveLocale, naiveDateLocale };
}
