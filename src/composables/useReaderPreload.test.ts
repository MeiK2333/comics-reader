import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { effectScope, nextTick, ref } from "vue";
import type { ComicPlatform } from "../platforms";
import type { ReaderPage } from "../api/reader";

const prefetchMock = vi.hoisted(() => vi.fn());

vi.mock("../api/images", () => ({ prefetchCachedImage: prefetchMock }));

import { useReaderPreload } from "./useReaderPreload";

function page(url: string, localPath: string | null = null): ReaderPage {
  return { url, width: null, height: null, localPath };
}

function setup(options: {
  pages: ReaderPage[];
  page?: number;
  count?: number;
  platform?: ComicPlatform;
}) {
  const scope = effectScope();
  const platform = ref<ComicPlatform>(options.platform ?? "nhentai");
  const pages = ref(options.pages);
  const pageIndex = ref(options.page ?? 1);
  const count = ref(options.count ?? 3);
  scope.run(() =>
    useReaderPreload(platform, pages, pageIndex, count),
  );
  return { scope, platform, pages, page: pageIndex, count };
}

function warmedUrls(): string[] {
  return prefetchMock.mock.calls
    .map((call) => call[1] as string)
    .sort();
}

async function flush() {
  for (let index = 0; index < 10; index += 1) {
    await Promise.resolve();
  }
}

beforeEach(() => {
  prefetchMock.mockReset();
  prefetchMock.mockResolvedValue(undefined);
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useReaderPreload", () => {
  it("waits for the debounce before warming upcoming pages", async () => {
    const state = setup({
      pages: ["p1", "p2", "p3", "p4", "p5"].map((url) => page(url)),
      page: 1,
      count: 3,
    });

    await vi.advanceTimersByTimeAsync(199);
    expect(prefetchMock).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    await flush();
    expect(warmedUrls()).toEqual(["p2", "p3", "p4"]);
    expect(state.scope).toBeDefined();
    state.scope.stop();
  });

  it("skips downloaded and empty pages", async () => {
    setup({
      pages: [
        page("p1"),
        page("local", "/downloads/1/0.jpg"),
        page("   "),
        page("p4"),
      ],
      page: 1,
      count: 3,
    });

    await vi.advanceTimersByTimeAsync(200);
    await flush();

    expect(warmedUrls()).toEqual(["p4"]);
  });

  it("warms with two concurrent workers", async () => {
    const gates: Array<() => void> = [];
    let active = 0;
    let peak = 0;
    prefetchMock.mockImplementation(
      () =>
        new Promise<void>((resolve) => {
          active += 1;
          peak = Math.max(peak, active);
          gates.push(() => {
            active -= 1;
            resolve();
          });
        }),
    );
    setup({
      pages: Array.from({ length: 6 }, (_, index) => page(`p${index}`)),
      page: 1,
      count: 5,
    });

    await vi.advanceTimersByTimeAsync(200);
    await flush();
    expect(prefetchMock).toHaveBeenCalledTimes(2);

    while (gates.length > 0) {
      gates.shift()?.();
      await flush();
    }

    expect(prefetchMock).toHaveBeenCalledTimes(5);
    expect(peak).toBe(2);
  });

  it("moves the window with the page without repeating warmed urls", async () => {
    const state = setup({
      pages: ["p1", "p2", "p3", "p4", "p5", "p6"].map((url) => page(url)),
      page: 1,
      count: 3,
    });

    await vi.advanceTimersByTimeAsync(200);
    await flush();

    state.page.value = 2;
    await nextTick();
    await vi.advanceTimersByTimeAsync(200);
    await flush();

    expect(warmedUrls()).toEqual(["p2", "p3", "p4", "p5"]);
  });

  it("stops scheduling once the reader scope is disposed", async () => {
    const state = setup({
      pages: ["p1", "p2", "p3", "p4"].map((url) => page(url)),
      page: 1,
      count: 3,
    });

    state.scope.stop();
    await vi.advanceTimersByTimeAsync(1_000);
    await flush();

    expect(prefetchMock).not.toHaveBeenCalled();
  });

  it("ignores prefetch failures", async () => {
    prefetchMock.mockRejectedValue(new Error("offline"));
    setup({
      pages: ["p1", "p2", "p3"].map((url) => page(url)),
      page: 1,
      count: 2,
    });

    await vi.advanceTimersByTimeAsync(200);
    await flush();

    expect(prefetchMock).toHaveBeenCalledTimes(2);
  });

  it("never warms more than the maximum window", async () => {
    setup({
      pages: Array.from({ length: 20 }, (_, index) => page(`p${index}`)),
      page: 1,
      count: 99,
    });

    await vi.advanceTimersByTimeAsync(200);
    await flush();

    expect(prefetchMock).toHaveBeenCalledTimes(10);
  });
});
