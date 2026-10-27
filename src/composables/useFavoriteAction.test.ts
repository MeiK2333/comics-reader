import { enableAutoUnmount, flushPromises, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { computed, defineComponent, h, nextTick, ref } from "vue";
import { NMessageProvider } from "naive-ui";
import type { ComicPlatform } from "../api/comics";
import type { FavoriteEntry } from "../api/favorites";
import type { FavoriteSnapshot } from "./useFavoriteAction";

const getFavoriteMock = vi.hoisted(() => vi.fn());
const addFavoriteMock = vi.hoisted(() => vi.fn());
const removeFavoriteMock = vi.hoisted(() => vi.fn());

vi.mock("../api/favorites", () => ({
  getFavorite: getFavoriteMock,
  addFavorite: addFavoriteMock,
  removeFavorite: removeFavoriteMock,
}));

import { useFavoriteAction } from "./useFavoriteAction";

enableAutoUnmount(afterEach);

afterEach(() => {
  vi.useRealTimers();
});

const snapshot: FavoriteSnapshot = {
  platform: "nhentai",
  title: "Work",
  thumb: null,
};

function entry(overrides: Partial<FavoriteEntry> = {}): FavoriteEntry {
  return {
    platform: "nhentai",
    id: "1",
    title: "Work",
    thumb: null,
    addedAt: 1,
    groupId: "default",
    ...overrides,
  };
}

function mountAction() {
  const platformValue = ref<ComicPlatform>("nhentai");
  const idValue = ref("1");
  const platform = computed<ComicPlatform | null>(() => platformValue.value);
  const id = computed(() => idValue.value);
  const detail = ref<FavoriteSnapshot | null>(null);
  let api!: ReturnType<typeof useFavoriteAction>;

  const Harness = defineComponent({
    setup() {
      api = useFavoriteAction({ platform, id, detail });
      return () => h("div");
    },
  });

  const Wrapper = defineComponent({
    setup() {
      return () => h(NMessageProvider, null, { default: () => h(Harness) });
    },
  });

  const wrapper = mount(Wrapper);
  return { api, detail, platformValue, idValue, wrapper };
}

async function showDetail(
  harness: ReturnType<typeof mountAction>,
  value: FavoriteSnapshot = snapshot,
) {
  harness.detail.value = value;
  await flushPromises();
}

beforeEach(() => {
  getFavoriteMock.mockReset();
  addFavoriteMock.mockReset();
  removeFavoriteMock.mockReset();

  getFavoriteMock.mockResolvedValue(null);
  addFavoriteMock.mockResolvedValue(entry());
  removeFavoriteMock.mockResolvedValue(undefined);
});

describe("useFavoriteAction", () => {
  it("loads the favorite state when the detail first arrives", async () => {
    getFavoriteMock.mockResolvedValue(entry({ groupId: "group-2" }));
    const harness = mountAction();

    expect(getFavoriteMock).not.toHaveBeenCalled();

    await showDetail(harness);

    expect(getFavoriteMock).toHaveBeenCalledWith("nhentai", "1");
    expect(harness.api.favorited.value).toBe(true);
    expect(harness.api.currentGroupId.value).toBe("group-2");
    expect(harness.api.loading.value).toBe(false);
  });

  it("treats a failed lookup as not favorited and keeps the button usable", async () => {
    getFavoriteMock.mockRejectedValue(new Error("index unavailable"));
    const harness = mountAction();
    await showDetail(harness);

    expect(harness.api.favorited.value).toBe(false);
    expect(harness.api.loading.value).toBe(false);

    addFavoriteMock.mockResolvedValue(entry());
    await harness.api.toggle();

    expect(addFavoriteMock).toHaveBeenCalledOnce();
    expect(harness.api.favorited.value).toBe(true);
  });

  it("adds an unfavorited work through a short click", async () => {
    const harness = mountAction();
    await showDetail(harness);

    harness.api.handleClick();
    await flushPromises();

    expect(addFavoriteMock).toHaveBeenCalledWith({
      platform: "nhentai",
      id: "1",
      title: "Work",
      thumb: null,
    });
    expect(harness.api.favorited.value).toBe(true);
  });

  it("removes an already favorited work on toggle", async () => {
    getFavoriteMock.mockResolvedValue(entry());
    const harness = mountAction();
    await showDetail(harness);

    await harness.api.toggle();

    expect(removeFavoriteMock).toHaveBeenCalledWith("nhentai", "1");
    expect(harness.api.favorited.value).toBe(false);
    expect(harness.api.currentGroupId.value).toBeNull();
  });

  it("opens the group picker on right click", async () => {
    const harness = mountAction();
    await showDetail(harness);
    const preventDefault = vi.fn();

    harness.api.handlers.contextmenu({
      preventDefault,
    } as unknown as MouseEvent);

    expect(preventDefault).toHaveBeenCalled();
    expect(harness.api.showGroupPicker.value).toBe(true);
  });

  it("opens the picker after a long press and swallows the release click", async () => {
    const harness = mountAction();
    await showDetail(harness);
    vi.useFakeTimers();

    harness.api.handlers.pointerdown({
      isPrimary: true,
      pointerType: "touch",
      pointerId: 1,
      button: 0,
      clientX: 10,
      clientY: 10,
    } as unknown as PointerEvent);
    vi.advanceTimersByTime(500);
    harness.api.handlers.pointerup({
      pointerId: 1,
    } as unknown as PointerEvent);

    expect(harness.api.showGroupPicker.value).toBe(true);

    harness.api.handleClick();
    await Promise.resolve();

    expect(addFavoriteMock).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it("opens the group picker on Shift+Enter and swallows the next click", async () => {
    const harness = mountAction();
    await showDetail(harness);
    const preventDefault = vi.fn();

    harness.api.handlers.keydown({
      key: "Enter",
      shiftKey: true,
      preventDefault,
    } as unknown as KeyboardEvent);

    expect(preventDefault).toHaveBeenCalled();
    expect(harness.api.showGroupPicker.value).toBe(true);

    harness.api.handleClick();
    await flushPromises();

    expect(addFavoriteMock).not.toHaveBeenCalled();
  });

  it("discards a favorite lookup that finishes after the work changed", async () => {
    let resolveFirst!: (value: FavoriteEntry | null) => void;
    getFavoriteMock.mockReturnValueOnce(
      new Promise<FavoriteEntry | null>((resolve) => {
        resolveFirst = resolve;
      }),
    );

    const harness = mountAction();
    await showDetail(harness);

    harness.idValue.value = "2";
    harness.detail.value = { ...snapshot, title: "Other work" };
    await nextTick();
    await flushPromises();

    expect(getFavoriteMock).toHaveBeenLastCalledWith("nhentai", "2");

    resolveFirst(entry());
    await flushPromises();

    expect(harness.api.favorited.value).toBe(false);
  });

  it("clears the state when the detail goes away", async () => {
    getFavoriteMock.mockResolvedValue(entry());
    const harness = mountAction();
    await showDetail(harness);
    expect(harness.api.favorited.value).toBe(true);

    harness.detail.value = null;
    await nextTick();

    expect(harness.api.entry.value).toBeNull();
    expect(harness.api.favorited.value).toBe(false);
    expect(harness.api.showGroupPicker.value).toBe(false);
    expect(harness.api.pending.value).toBe(false);
  });

  it("stores the entry returned by the group picker", async () => {
    const harness = mountAction();
    await showDetail(harness);
    harness.api.showGroupPicker.value = true;

    harness.api.onSaved(entry({ groupId: "group-3" }));

    expect(harness.api.currentGroupId.value).toBe("group-3");
    expect(harness.api.showGroupPicker.value).toBe(false);
    expect(harness.api.favorited.value).toBe(true);
  });
});
