import { flushPromises } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { computed, effectScope, nextTick, ref } from "vue";
import type { EffectScope } from "vue";
import type { ComicPlatform } from "../api/comics";
import type { DownloadDetail } from "../api/downloads";
import type { HistoryEntry } from "../api/history";

const getDownloadMock = vi.hoisted(() => vi.fn());
const getHistoryEntryMock = vi.hoisted(() => vi.fn());

vi.mock("../api/downloads", () => ({ getDownload: getDownloadMock }));
vi.mock("../api/history", () => ({
  getHistoryEntry: getHistoryEntryMock,
}));

import { useDownloadedComicDetail } from "./useDownloadedComicDetail";

const scopes: EffectScope[] = [];

afterEach(() => {
  for (const scope of scopes) {
    scope.stop();
  }
  scopes.length = 0;
});

function downloadDetail(
  overrides: Partial<DownloadDetail> = {},
): DownloadDetail {
  return {
    platform: "nhentai",
    id: "1",
    title: "Work",
    thumb: "thumb.jpg",
    coverPath: null,
    status: "completed",
    totalChapters: 2,
    completedChapters: 2,
    failedChapters: 0,
    downloadedPages: 20,
    totalBytes: 200,
    readChapterId: "c1",
    addedAt: 1,
    updatedAt: 2,
    error: null,
    progress: null,
    chapters: [
      { id: "c1", title: "Chapter 1", pageCount: 10, completed: true },
      { id: "c2", title: "Chapter 2", pageCount: 10, completed: true },
    ],
    ...overrides,
  };
}

function historyEntry(overrides: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    platform: "nhentai",
    id: "1",
    title: "Work",
    thumb: null,
    chapterId: null,
    chapterTitle: null,
    page: 4,
    totalPages: 10,
    updatedAt: 1,
    ...overrides,
  };
}

function setup(
  platform: ComicPlatform | null = "nhentai",
  id = "1",
) {
  const platformRef = ref<ComicPlatform | null>(platform);
  const idRef = ref(id);
  const platformValue = computed<ComicPlatform | null>(
    () => platformRef.value,
  );
  const idValue = computed(() => idRef.value);

  const scope = effectScope();
  const api = scope.run(() =>
    useDownloadedComicDetail({ platform: platformValue, id: idValue }),
  )!;
  scopes.push(scope);

  return { api, idRef };
}

beforeEach(() => {
  getDownloadMock.mockReset();
  getHistoryEntryMock.mockReset();

  getDownloadMock.mockResolvedValue(downloadDetail());
  getHistoryEntryMock.mockResolvedValue(null);
});

describe("useDownloadedComicDetail", () => {
  it("starts in loading and exposes the ready download", async () => {
    const { api } = setup();
    expect(api.state.value.kind).toBe("loading");

    await flushPromises();

    expect(api.state.value.kind).toBe("ready");
    expect(api.download.value?.title).toBe("Work");
    expect(api.completedChapterIds.value).toEqual(["c1", "c2"]);
    expect(api.startChapterId.value).toBe("c1");
    expect(api.canRead.value).toBe(true);
    expect(api.favoriteSnapshot.value).toEqual({
      platform: "nhentai",
      title: "Work",
      thumb: "thumb.jpg",
    });
  });

  it("reports a missing download as not found", async () => {
    getDownloadMock.mockResolvedValue(null);

    const { api } = setup();
    await flushPromises();

    expect(api.error.value?.code).toBe("notFound");
    expect(api.state.value.kind).toBe("error");
    expect(api.loading.value).toBe(false);
  });

  it("prefers readChapterId over the first completed chapter", async () => {
    getDownloadMock.mockResolvedValue(
      downloadDetail({ readChapterId: "c2" }),
    );

    const { api } = setup();
    await flushPromises();

    expect(api.startChapterId.value).toBe("c2");
  });

  it("falls back to the first completed chapter when no readChapterId is set", async () => {
    getDownloadMock.mockResolvedValue(
      downloadDetail({
        readChapterId: null,
        chapters: [
          { id: "c1", title: "Chapter 1", pageCount: 10, completed: false },
          { id: "c2", title: "Chapter 2", pageCount: 10, completed: true },
        ],
      }),
    );

    const { api } = setup();
    await flushPromises();

    expect(api.startChapterId.value).toBe("c2");
  });

  it("has nothing to read without a completed chapter", async () => {
    getDownloadMock.mockResolvedValue(
      downloadDetail({
        readChapterId: null,
        completedChapters: 0,
        chapters: [
          { id: "c1", title: "Chapter 1", pageCount: 10, completed: false },
        ],
      }),
    );

    const { api } = setup();
    await flushPromises();

    expect(api.canRead.value).toBe(false);
    expect(api.startChapterId.value).toBeNull();
  });

  it("resumes a completed history chapter and page", async () => {
    getHistoryEntryMock.mockResolvedValue(
      historyEntry({ chapterId: "c2", page: 6 }),
    );

    const { api } = setup();
    await flushPromises();

    expect(api.canResume.value).toBe(true);
    expect(api.resumeChapterId.value).toBe("c2");
    expect(api.resumePage.value).toBe(6);
  });

  it("falls back to the start when the history chapter is gone", async () => {
    getHistoryEntryMock.mockResolvedValue(
      historyEntry({ chapterId: "removed", page: 6 }),
    );

    const { api } = setup();
    await flushPromises();

    expect(api.resumeChapterId.value).toBe("c1");
    expect(api.resumePage.value).toBe(1);
  });

  it("keeps the page for an nhentai history without a chapter", async () => {
    getDownloadMock.mockResolvedValue(
      downloadDetail({
        readChapterId: null,
        chapters: [
          { id: null, title: null, pageCount: 20, completed: true },
        ],
      }),
    );
    getHistoryEntryMock.mockResolvedValue(historyEntry({ page: 4 }));

    const { api } = setup();
    await flushPromises();

    expect(api.startChapterId.value).toBeNull();
    expect(api.resumeChapterId.value).toBeNull();
    expect(api.resumePage.value).toBe(4);
  });

  it("keeps the detail usable when the history lookup fails", async () => {
    getHistoryEntryMock.mockRejectedValue(new Error("history"));

    const { api } = setup();
    await flushPromises();

    expect(api.state.value.kind).toBe("ready");
    expect(api.historyEntry.value).toBeNull();
    expect(api.canResume.value).toBe(false);
  });

  it("discards a download response for a work the user already left", async () => {
    let resolveFirst!: (value: DownloadDetail) => void;
    getDownloadMock.mockReturnValueOnce(
      new Promise<DownloadDetail>((resolve) => {
        resolveFirst = resolve;
      }),
    );
    getDownloadMock.mockResolvedValueOnce(
      downloadDetail({ title: "Second" }),
    );

    const { api, idRef } = setup();
    idRef.value = "2";
    await nextTick();
    await flushPromises();

    resolveFirst(downloadDetail({ title: "First" }));
    await flushPromises();

    expect(api.download.value?.title).toBe("Second");
  });
});
