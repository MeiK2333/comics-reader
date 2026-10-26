import { beforeEach, describe, expect, it, vi } from "vitest";
import { createApp } from "vue";
import { clampReaderPreloadCount } from "./settings";

/**
 * Import a fresh store because the persisted-state plugin only hydrates on
 * store creation, and module-level singletons would keep the first state.
 *
 * Pinia 4 queues plugins until `app.use(pinia)` installs the pinia instance;
 * without an app the persistence plugin would never run in tests.
 */
async function createSettingsStore() {
  vi.resetModules();
  const [{ setActivePinia }, { default: pinia }, { useSettingsStore }] =
    await Promise.all([
      import("pinia"),
      import("./index"),
      import("./settings"),
    ]);
  createApp({ render: () => null }).use(pinia);
  setActivePinia(pinia);
  return useSettingsStore();
}

beforeEach(() => {
  localStorage.clear();
});

describe("clampReaderPreloadCount", () => {
  it("falls back to the default for non-finite values", () => {
    expect(clampReaderPreloadCount(Number.NaN)).toBe(3);
    expect(clampReaderPreloadCount(Number.POSITIVE_INFINITY)).toBe(3);
    expect(clampReaderPreloadCount(Number.NEGATIVE_INFINITY)).toBe(3);
  });

  it("clamps into the supported 3..10 window", () => {
    expect(clampReaderPreloadCount(-4)).toBe(3);
    expect(clampReaderPreloadCount(2)).toBe(3);
    expect(clampReaderPreloadCount(10)).toBe(10);
    expect(clampReaderPreloadCount(42)).toBe(10);
  });

  it("rounds fractional values", () => {
    expect(clampReaderPreloadCount(6.4)).toBe(6);
    expect(clampReaderPreloadCount(6.6)).toBe(7);
  });
});

describe("settings persistence", () => {
  it("starts with the documented defaults", async () => {
    const store = await createSettingsStore();

    expect(store.theme).toBe("system");
    expect(store.locale).toBe("system");
    expect(store.ech).toBe(false);
    expect(store.logging).toBe(false);
    expect(store.readerDirection).toBe("ltr");
    expect(store.readerPreloadCount).toBe(3);
    expect(store.readerPreloadNextChapter).toBe(true);
  });

  it("hydrates from the namespaced localStorage key", async () => {
    localStorage.setItem(
      "comics-reader:settings",
      JSON.stringify({ theme: "dark", readerPreloadCount: 9 }),
    );

    const store = await createSettingsStore();

    expect(store.theme).toBe("dark");
    expect(store.readerPreloadCount).toBe(9);
    // Missing persisted fields keep their defaults instead of becoming
    // undefined, which would break the reader.
    expect(store.readerDirection).toBe("ltr");
  });

  it("persists through the same namespaced key", async () => {
    const store = await createSettingsStore();
    store.readerPreloadCount = 7;
    store.theme = "light";

    store.$persist();

    const raw = localStorage.getItem("comics-reader:settings");
    expect(raw).not.toBeNull();
    expect(JSON.parse(raw ?? "{}")).toMatchObject({
      readerPreloadCount: 7,
      theme: "light",
    });
  });
});
