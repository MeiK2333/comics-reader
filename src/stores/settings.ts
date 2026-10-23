import { defineStore } from "pinia";

export type ThemeMode = "system" | "light" | "dark";

export interface SettingsState {
  theme: ThemeMode;
}

export const useSettingsStore = defineStore("settings", {
  state: (): SettingsState => ({
    theme: "system",
  }),
  // Persisted under `comics-reader:settings` (see `src/stores/index.ts`) and
  // rehydrated synchronously when the store is first used.
  persist: true,
});
