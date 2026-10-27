import { computed } from "vue";
import { resolveLocale } from "./index";
import { useSettingsStore } from "../stores/settings";

// Rebuild the formatters when the app language changes instead of following
// the system locale, which may differ from the selected UI language.
const DATE_TIME_OPTIONS: Intl.DateTimeFormatOptions = {
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  hour12: false,
};

export function useDateTimeFormatter() {
  const settings = useSettingsStore();
  const formatter = computed(
    () =>
      new Intl.DateTimeFormat(resolveLocale(settings.locale), DATE_TIME_OPTIONS),
  );

  function format(timestamp: number): string {
    return formatter.value.format(new Date(timestamp));
  }

  function toIso(timestamp: number): string {
    return new Date(timestamp).toISOString();
  }

  return { format, toIso };
}

const BYTE_UNITS = ["B", "KB", "MB", "GB", "TB"];

export function useByteFormatter() {
  const settings = useSettingsStore();
  const numberFormatter = computed(
    () =>
      new Intl.NumberFormat(resolveLocale(settings.locale), {
        maximumFractionDigits: 1,
      }),
  );

  function format(bytes: number): string {
    if (bytes <= 0) {
      return "0 B";
    }

    let value = bytes;
    let unit = 0;
    while (value >= 1024 && unit < BYTE_UNITS.length - 1) {
      value /= 1024;
      unit += 1;
    }
    return `${numberFormatter.value.format(value)} ${BYTE_UNITS[unit]}`;
  }

  return { format };
}
