import { flushPromises } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { effectScope, nextTick, ref } from "vue";
import type { EffectScope } from "vue";
import type { ComicPlatform } from "../api/comics";
import type { CachedImage } from "../api/images";

const fetchMock = vi.hoisted(() => vi.fn());
const getCachedMock = vi.hoisted(() => vi.fn());
const forgetMock = vi.hoisted(() => vi.fn());
const convertMock = vi.hoisted(() =>
  vi.fn((path: string) => `asset://${path}`),
);

vi.mock("../api/images", () => ({
  fetchCachedImage: fetchMock,
  getCachedImage: getCachedMock,
  forgetCachedImage: forgetMock,
}));
vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: convertMock }));

import { useCachedImage } from "./useCachedImage";

const scopes: EffectScope[] = [];

afterEach(() => {
  for (const scope of scopes) {
    scope.stop();
  }
  scopes.length = 0;
});

function image(path: string): CachedImage {
  return {
    path,
    contentType: "image/jpeg",
    byteLength: 10,
    fromCache: false,
    platform: "nhentai",
  };
}

function setup() {
  const platform = ref<ComicPlatform>("nhentai");
  const url = ref("https://example.com/1.jpg");
  const localPath = ref<string | null>(null);
  const scope = effectScope();
  const api = scope.run(() =>
    useCachedImage(platform, url, localPath),
  )!;
  scopes.push(scope);
  return { api, platform, url, localPath };
}

beforeEach(() => {
  fetchMock.mockReset();
  getCachedMock.mockReset();
  forgetMock.mockReset();
  convertMock.mockClear();

  getCachedMock.mockReturnValue(null);
  fetchMock.mockResolvedValue(image("/cache/default.jpg"));
});

describe("useCachedImage", () => {
  it("uses a downloaded page without touching the cache or network", async () => {
    const harness = setup();
    harness.localPath.value = "/downloads/1.jpg";
    await nextTick();

    await harness.api.load();

    expect(convertMock).toHaveBeenCalledWith("/downloads/1.jpg");
    expect(harness.api.src.value).toBe("asset:///downloads/1.jpg");
    expect(harness.api.loading.value).toBe(false);
    expect(getCachedMock).not.toHaveBeenCalled();
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("clears the source for an empty url", async () => {
    const harness = setup();
    harness.url.value = "   ";
    await nextTick();

    await harness.api.load();

    expect(harness.api.src.value).toBe("");
    expect(harness.api.loading.value).toBe(false);
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("renders an already resolved cache path without a request", async () => {
    getCachedMock.mockReturnValue(image("/cache/1.jpg"));
    const harness = setup();

    await harness.api.load();

    expect(harness.api.src.value).toBe("asset:///cache/1.jpg");
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it("shows loading while a remote image is fetched", async () => {
    let resolveImage!: (value: CachedImage) => void;
    fetchMock.mockReturnValue(
      new Promise<CachedImage>((resolve) => {
        resolveImage = resolve;
      }),
    );
    const harness = setup();

    const pending = harness.api.load();
    expect(harness.api.loading.value).toBe(true);

    resolveImage(image("/cache/2.jpg"));
    await pending;

    expect(harness.api.src.value).toBe("asset:///cache/2.jpg");
    expect(harness.api.loading.value).toBe(false);
    expect(harness.api.failed.value).toBe(false);
  });

  it("keeps the newest path when an older fetch resolves late", async () => {
    let resolveFirst!: (value: CachedImage) => void;
    fetchMock.mockReturnValueOnce(
      new Promise<CachedImage>((resolve) => {
        resolveFirst = resolve;
      }),
    );
    fetchMock.mockResolvedValueOnce(image("/cache/new.jpg"));
    const harness = setup();

    const first = harness.api.load();
    harness.url.value = "https://example.com/2.jpg";
    await nextTick();
    const second = harness.api.load();
    await second;

    resolveFirst(image("/cache/old.jpg"));
    await first;

    expect(harness.api.src.value).toBe("asset:///cache/new.jpg");
  });

  it("marks a failed fetch without keeping the previous source", async () => {
    fetchMock.mockRejectedValue(new Error("404"));
    const harness = setup();

    await harness.api.load();

    expect(harness.api.src.value).toBe("");
    expect(harness.api.failed.value).toBe(true);
    expect(harness.api.loading.value).toBe(false);
  });

  it("forgets a failed remote image but not a downloaded page", async () => {
    const remote = setup();
    remote.api.fail();
    expect(forgetMock).toHaveBeenCalledWith(
      "nhentai",
      "https://example.com/1.jpg",
    );
    expect(remote.api.failed.value).toBe(true);

    forgetMock.mockClear();
    const local = setup();
    local.localPath.value = "/downloads/1.jpg";
    await nextTick();
    local.api.fail();
    expect(forgetMock).not.toHaveBeenCalled();
  });

  it("resets the resolved source when its dependencies change", async () => {
    getCachedMock.mockReturnValue(image("/cache/1.jpg"));
    const harness = setup();
    await harness.api.load();
    expect(harness.api.src.value).not.toBe("");

    harness.platform.value = "jm";
    await nextTick();

    expect(harness.api.src.value).toBe("");
    expect(harness.api.loading.value).toBe(false);
    expect(harness.api.failed.value).toBe(false);
  });

  it("retries after a failure", async () => {
    fetchMock.mockRejectedValueOnce(new Error("404"));
    fetchMock.mockResolvedValueOnce(image("/cache/retry.jpg"));
    const harness = setup();

    await harness.api.load();
    expect(harness.api.failed.value).toBe(true);

    harness.api.retry();
    await flushPromises();

    expect(harness.api.src.value).toBe("asset:///cache/retry.jpg");
    expect(harness.api.failed.value).toBe(false);
  });
});
