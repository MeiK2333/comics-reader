import { defineStore } from "pinia";

/**
 * Transient startup state. Deliberately not persisted: a reload must always
 * re-run the ECH bootstrap instead of inheriting a stale "ready" flag.
 */
export const useRuntimeStore = defineStore("runtime", {
  state: () => ({
    /** True while `set_ech_enabled(true)` is still bootstrapping on launch. */
    echStarting: false,
  }),
});
