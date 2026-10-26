import type { MessageKey } from "./i18n";

/** Home page quick links shared by the home grid and the placeholder page. */
export type ShortcutFeature = "favorites" | "downloads" | "history" | "settings";

export const SHORTCUT_LABEL_KEYS: Record<ShortcutFeature, MessageKey> = {
  favorites: "home.shortcuts.favorites",
  downloads: "home.shortcuts.downloads",
  history: "home.shortcuts.history",
  settings: "home.shortcuts.settings",
};

export interface ShortcutDefinition {
  feature: ShortcutFeature;
  path: string;
}

export const SHORTCUTS: readonly ShortcutDefinition[] = [
  { feature: "history", path: "/history" },
  { feature: "favorites", path: "/favorites" },
  { feature: "downloads", path: "/downloads" },
  { feature: "settings", path: "/settings" },
];
