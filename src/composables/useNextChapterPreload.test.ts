import { flushPromises } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { effectScope, nextTick, ref } from "vue";
import type { EffectScope } from "vue";
import type { ComicPlatform } from "../api/comics";
import type { ReaderChapter, ReaderPage } from "../api/reader";

const prefetchChapterMock = vi.hoisted(() => vi.fn());
const prefetchImagesMock = vi.hoisted(() => vi.fn());

vi.mock("../api/reader", () => ({
  prefetchReaderChapter: prefetchChapterMock,
}));
vi.mock("../api/images", () => ({
  prefetchCachedImages: prefetchImagesMock,
}));

import { useNextChapterPreload } from "./useNextChapterPreload";

const scopes: EffectScope[] = [];

afterEach(() => {
  for (const scope of scopes) {
    scope.stop();
  }
  scopes.length = 0;
});

function page(url: string, localPath: string | null = null): ReaderPage {
  return { url, width: null, height: null, localPath };
}

function chapter(
  pages: ReaderPage[],
  nextId: string | null = "ch-2",
): ReaderChapter {
  return {
    title: "Work",
    thumb: "",
    chapterId: "ch-1",
    chapterTitle: "Chapter 1",
    pages,
    previous: null,
    next: nextId ? { id: nextId, title: "Chapter 2" } : null,
  };
}

interface SetupOptions {
  chapter?: ReaderChapter | null;
  page?: number;
  enabled?: boolean;
  count?: number;
  platform?: ComicPlatform | null;
  id?: string;
}

function setup(options: SetupOptions = {}) {
  const chapterRef = ref<ReaderChapter | null>(
    options.chapter === undefined ? chapter([page("u1")]) : options.chapter,
  );
  const pageRef = ref(options.page ?? 1);
  const enabled = ref(options.enabled ?? true);
  const count = ref(options.count ?? 3);
  const platform = ref<ComicPlatform | null>(
    options.platform === undefined ? "nhentai" : options.platform,
  );
  const id = ref(options.id ?? "1");

  const scope = effectScope();
  scope.run(() =>
    useNextChapterPreload(
      chapterRef,
      pageRef,
      platform,
      id,
      enabled,
      count,
    ),
  );
  scopes.push(scope);

  return { chapterRef, pageRef, enabled, count, platform, id };
}

beforeEach(() => {
  prefetchChapterMock.mockReset();
  prefetchImagesMock.mockReset();
  prefetchChapterMock.mockResolvedValue(chapter([page("n1")]));
  prefetchImagesMock.mockResolvedValue(undefined);
});

describe("useNextChapterPreload", () => {
  it("waits for the last page before warming the next chapter", async () => {
    const harness = setup({
      chapter: chapter([page("u1"), page("u2")]),
      page: 1,
    });
    await flushPromises();

    expect(prefetchChapterMock).not.toHaveBeenCalled();

    harness.pageRef.value = 2;
    await flushPromises();

    expect(prefetchChapterMock).toHaveBeenCalledWith(
      "nhentai",
      "1",
      "ch-2",
    );
  });

  it("does nothing when disabled, without a next chapter or without a work key", async () => {
    setup({
      chapter: chapter([page("u1")]),
      enabled: false,
    });
    setup({
      chapter: chapter([page("u1")], null),
    });
    setup({
      chapter: chapter([page("u1")]),
      platform: null,
    });
    setup({
      chapter: chapter([]),
    });
    await flushPromises();

    expect(prefetchChapterMock).not.toHaveBeenCalled();
  });

  it("warms only the first N remote pages, skipping local and blank URLs", async () => {
    prefetchChapterMock.mockResolvedValue(
      chapter([
        page("n1"),
        page("n2", "/downloads/n2.jpg"),
        page("n3"),
        page("n4"),
        page("n5"),
      ]),
    );

    setup({
      chapter: chapter([page("u1"), page("u2"), page("u3")]),
      page: 3,
      count: 3,
    });
    await flushPromises();

    expect(prefetchImagesMock).toHaveBeenCalledWith("nhentai", ["n1", "n3"]);
  });

  it("clamps the requested preload count to ten", async () => {
    const pages = Array.from({ length: 12 }, (_, index) =>
      page(`n${index + 1}`),
    );
    prefetchChapterMock.mockResolvedValue(chapter(pages));

    setup({ chapter: chapter([page("u1")]), page: 1, count: 99 });
    await flushPromises();

    const [, urls] = prefetchImagesMock.mock.calls[0] as [
      ComicPlatform,
      string[],
    ];
    expect(urls).toHaveLength(10);
    expect(urls[0]).toBe("n1");
    expect(urls[9]).toBe("n10");
  });

  it("warms the same chapter and count only once", async () => {
    const harness = setup({
      chapter: chapter([page("u1"), page("u2")]),
      page: 2,
    });
    await flushPromises();

    harness.pageRef.value = 1;
    await nextTick();
    harness.pageRef.value = 2;
    await flushPromises();

    expect(prefetchChapterMock).toHaveBeenCalledOnce();
    expect(prefetchImagesMock).toHaveBeenCalledOnce();
  });

  it("does not start a second request while the first is in flight", async () => {
    let resolveChapter!: (value: ReaderChapter) => void;
    prefetchChapterMock.mockReturnValue(
      new Promise<ReaderChapter>((resolve) => {
        resolveChapter = resolve;
      }),
    );

    const harness = setup({
      chapter: chapter([page("u1"), page("u2")]),
      page: 2,
    });

    harness.pageRef.value = 1;
    await nextTick();
    harness.pageRef.value = 2;
    await Promise.resolve();

    expect(prefetchChapterMock).toHaveBeenCalledOnce();

    resolveChapter(chapter([page("n1")]));
    await flushPromises();

    expect(prefetchImagesMock).toHaveBeenCalledOnce();
  });

  it("retries after a chapter prefetch failure", async () => {
    prefetchChapterMock.mockRejectedValueOnce(new Error("offline"));

    const harness = setup({
      chapter: chapter([page("u1"), page("u2")]),
      page: 2,
    });
    await flushPromises();

    expect(prefetchChapterMock).toHaveBeenCalledOnce();
    expect(prefetchImagesMock).not.toHaveBeenCalled();

    harness.pageRef.value = 1;
    await nextTick();
    harness.pageRef.value = 2;
    await flushPromises();

    expect(prefetchChapterMock).toHaveBeenCalledTimes(2);
    expect(prefetchImagesMock).toHaveBeenCalledOnce();
  });

  it("retries after an image preload failure", async () => {
    prefetchImagesMock.mockRejectedValueOnce(new Error("cache down"));

    const harness = setup({
      chapter: chapter([page("u1"), page("u2")]),
      page: 2,
    });
    await flushPromises();

    expect(prefetchImagesMock).toHaveBeenCalledOnce();

    harness.pageRef.value = 1;
    await nextTick();
    harness.pageRef.value = 2;
    await flushPromises();

    expect(prefetchImagesMock).toHaveBeenCalledTimes(2);
  });

  it("passes an empty image list when the next chapter has no remote pages", async () => {
    prefetchChapterMock.mockResolvedValue(
      chapter([page("n1", "/downloads/1.jpg"), page("  ")]),
    );

    const harness = setup({
      chapter: chapter([page("u1"), page("u2")]),
      page: 2,
    });
    await flushPromises();

    expect(prefetchImagesMock).toHaveBeenCalledWith("nhentai", []);

    harness.pageRef.value = 1;
    await nextTick();
    harness.pageRef.value = 2;
    await flushPromises();

    // The empty warm-up still counts as warmed.
    expect(prefetchImagesMock).toHaveBeenCalledOnce();
  });

  it("ignores a chapter response that arrives after the scope is disposed", async () => {
    let resolveChapter!: (value: ReaderChapter) => void;
    prefetchChapterMock.mockReturnValue(
      new Promise<ReaderChapter>((resolve) => {
        resolveChapter = resolve;
      }),
    );

    const harness = setup({
      chapter: chapter([page("u1"), page("u2")]),
      page: 2,
    });
    harness.chapterRef.value = null;

    // Dispose before the pending chapter resolves.
    scopes[scopes.length - 1]?.stop();
    resolveChapter(chapter([page("n1")]));
    await flushPromises();

    expect(prefetchImagesMock).not.toHaveBeenCalled();
  });
});
