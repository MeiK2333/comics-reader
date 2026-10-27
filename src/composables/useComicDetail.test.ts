import { flushPromises } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { computed, effectScope, nextTick, ref } from "vue";
import type { EffectScope } from "vue";
import type {
  ComicDetail,
  ComicPlatform,
  JmDetail,
  ManhuaguiDetail,
  NhentaiDetail,
} from "../platforms";
import type { DownloadDetail } from "../api/downloads";
import type { HistoryEntry } from "../api/history";
import { AppError } from "../errors";

const getComicMock = vi.hoisted(() => vi.fn());
const getDownloadMock = vi.hoisted(() => vi.fn());
const getHistoryEntryMock = vi.hoisted(() => vi.fn());

vi.mock("../api/comics", () => ({ getComic: getComicMock }));
vi.mock("../api/downloads", () => ({ getDownload: getDownloadMock }));
vi.mock("../api/history", () => ({
  getHistoryEntry: getHistoryEntryMock,
}));

import { useComicDetail } from "./useComicDetail";

const scopes: EffectScope[] = [];

afterEach(() => {
  for (const scope of scopes) {
    scope.stop();
  }
  scopes.length = 0;
});

function nhentaiDetail(title = "Work"): NhentaiDetail {
  return {
    platform: "nhentai",
    title,
    thumb: "thumb.jpg",
    chapters: [{ id: null, title: null }],
    extra: {
      mediaId: "1",
      japaneseTitle: null,
      prettyTitle: null,
      numPages: 3,
      numFavorites: 0,
      scanlator: null,
      uploadDate: null,
      cover: null,
      coverWidth: null,
      coverHeight: null,
      tags: [],
    },
  };
}

function manhuaguiDetail(): ManhuaguiDetail {
  return {
    platform: "manhuagui",
    title: "Manhua",
    thumb: "cover.jpg",
    chapters: [
      { id: "c1", title: "Chapter 1" },
      { id: "c2", title: "Chapter 2" },
    ],
    extra: {
      originalTitle: null,
      authors: [],
      status: null,
      latestChapter: null,
      updatedAt: null,
      year: null,
      region: null,
      categories: [],
      aliases: [],
      description: null,
      score: null,
      ratingCount: null,
      chapterGroups: [
        {
          title: "",
          chapters: [
            { id: "c1", title: "Chapter 1" },
            { id: "c2", title: "Chapter 2" },
          ],
        },
      ],
    },
  };
}

function jmDetail(chapters: JmDetail["extra"]["chapters"]): JmDetail {
  return {
    platform: "jm",
    title: "JM work",
    thumb: "jm.jpg",
    chapters: chapters.map((chapter) => ({
      id: chapter.id,
      title: chapter.title,
    })),
    extra: {
      authors: [],
      tags: [],
      works: [],
      actors: [],
      pageCount: null,
      pubDate: null,
      updateDate: null,
      likes: null,
      views: null,
      commentCount: null,
      chapters,
    },
  };
}

function downloadDetail(
  overrides: Partial<DownloadDetail> = {},
): DownloadDetail {
  return {
    platform: "nhentai",
    id: "1",
    title: "Work",
    thumb: null,
    coverPath: null,
    status: "completed",
    totalChapters: 1,
    completedChapters: 1,
    failedChapters: 0,
    downloadedPages: 10,
    totalBytes: 100,
    readChapterId: null,
    addedAt: 1,
    updatedAt: 2,
    error: null,
    progress: null,
    chapters: [],
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
    useComicDetail({ platform: platformValue, id: idValue }),
  )!;
  scopes.push(scope);

  return { api, platformRef, idRef };
}

beforeEach(() => {
  getComicMock.mockReset();
  getDownloadMock.mockReset();
  getHistoryEntryMock.mockReset();

  getComicMock.mockResolvedValue(nhentaiDetail());
  getDownloadMock.mockResolvedValue(null);
  getHistoryEntryMock.mockResolvedValue(null);
});

describe("useComicDetail", () => {
  it("loads the online detail and clears loading", async () => {
    const { api } = setup();
    await flushPromises();

    expect(getComicMock).toHaveBeenCalledWith("nhentai", "1");
    expect(api.state.value.kind).toBe("online");
    expect(api.detail.value?.title).toBe("Work");
    expect(api.loading.value).toBe(false);
    expect(api.error.value).toBeNull();
    expect(api.downloadLoading.value).toBe(false);
  });

  it("shows the online error instead of a local fallback", async () => {
    getComicMock.mockRejectedValue(new AppError("network", "offline"));
    const { api } = setup();
    await flushPromises();

    expect(api.state.value.kind).toBe("error");
    if (api.state.value.kind === "error") {
      expect(api.state.value.error.code).toBe("network");
    }
    expect(api.detail.value).toBeNull();
  });

  it("rejects an invalid route before calling the platform", async () => {
    const missingPlatform = setup(null, "1");
    const missingId = setup("nhentai", "");
    await flushPromises();

    for (const { api } of [missingPlatform, missingId]) {
      expect(api.error.value?.code).toBe("invalidInput");
      expect(api.loading.value).toBe(false);
    }
    expect(getComicMock).not.toHaveBeenCalled();
  });

  it("discards a detail response for a work the user already left", async () => {
    let resolveFirst!: (value: ComicDetail) => void;
    getComicMock.mockReturnValueOnce(
      new Promise<ComicDetail>((resolve) => {
        resolveFirst = resolve;
      }),
    );
    getComicMock.mockResolvedValueOnce(nhentaiDetail("Second"));

    const { api, idRef } = setup();
    idRef.value = "2";
    await nextTick();
    await flushPromises();

    resolveFirst(nhentaiDetail("First"));
    await flushPromises();

    expect(api.detail.value?.title).toBe("Second");
  });

  it("keeps the online detail usable when the auxiliary reads fail", async () => {
    getDownloadMock.mockRejectedValue(new Error("index"));
    getHistoryEntryMock.mockRejectedValue(new Error("history"));

    const { api } = setup();
    await flushPromises();

    expect(api.state.value.kind).toBe("online");
    expect(api.download.value).toBeNull();
    expect(api.historyEntry.value).toBeNull();
    expect(api.downloadLoading.value).toBe(false);
  });

  it("maps the download chapter list per platform", async () => {
    getComicMock.mockResolvedValue(manhuaguiDetail());
    const manhuagui = setup();
    await flushPromises();

    expect(manhuagui.api.downloadChapters.value).toEqual([
      { id: "c1", title: "Chapter 1" },
      { id: "c2", title: "Chapter 2" },
    ]);

    getComicMock.mockResolvedValue(
      jmDetail([
        { id: "j1", title: "JM 1", sort: 1 },
        { id: "j2", title: "JM 2", sort: 2 },
      ]),
    );
    const jm = setup("jm", "9");
    await flushPromises();

    expect(jm.api.downloadChapters.value).toEqual([
      { id: "j1", title: "JM 1" },
      { id: "j2", title: "JM 2" },
    ]);

    getComicMock.mockResolvedValue(nhentaiDetail());
    const nhentai = setup();
    await flushPromises();

    expect(nhentai.api.downloadChapters.value).toEqual([]);
  });

  it("decides readability from the platform chapter list", async () => {
    const nhentai = setup();
    await flushPromises();
    expect(nhentai.api.canRead.value).toBe(true);

    getComicMock.mockResolvedValue(manhuaguiDetail());
    const manhuagui = setup("manhuagui", "2");
    await flushPromises();
    expect(manhuagui.api.canRead.value).toBe(true);

    getComicMock.mockResolvedValue(jmDetail([]));
    const jm = setup("jm", "3");
    await flushPromises();
    expect(jm.api.canRead.value).toBe(false);
  });

  it("resumes the saved chapter and page", async () => {
    getComicMock.mockResolvedValue(manhuaguiDetail());
    getHistoryEntryMock.mockResolvedValue(
      historyEntry({
        platform: "manhuagui",
        chapterId: "c2",
        chapterTitle: "Chapter 2",
        page: 7,
      }),
    );

    const { api } = setup("manhuagui", "2");
    await flushPromises();

    expect(api.canResume.value).toBe(true);
    expect(api.resumeChapterId.value).toBe("c2");
    expect(api.resumePage.value).toBe(7);
  });

  it("falls back to the first chapter when the saved chapter disappeared", async () => {
    getComicMock.mockResolvedValue(manhuaguiDetail());
    getHistoryEntryMock.mockResolvedValue(
      historyEntry({
        platform: "manhuagui",
        chapterId: "removed",
        page: 9,
      }),
    );

    const { api } = setup("manhuagui", "2");
    await flushPromises();

    expect(api.resumeChapterId.value).toBe("c1");
    expect(api.resumePage.value).toBe(1);
  });

  it("keeps the nhentai page because it has an implicit chapter", async () => {
    getHistoryEntryMock.mockResolvedValue(historyEntry({ page: 4 }));

    const { api } = setup();
    await flushPromises();

    expect(api.resumeChapterId.value).toBeNull();
    expect(api.resumePage.value).toBe(4);
    expect(api.canResume.value).toBe(true);
  });

  it("reloads the download silently without flipping the spinner", async () => {
    const { api } = setup();
    await flushPromises();

    let resolveReload!: (value: DownloadDetail) => void;
    getDownloadMock.mockReturnValueOnce(
      new Promise<DownloadDetail>((resolve) => {
        resolveReload = resolve;
      }),
    );

    const pending = api.reloadDownload(true);
    expect(api.downloadLoading.value).toBe(false);

    resolveReload(downloadDetail());
    await pending;

    expect(api.download.value?.completedChapters).toBe(1);
    expect(api.downloadLoading.value).toBe(false);
  });

  it("keeps the previous download value when a silent reload fails", async () => {
    const { api } = setup();
    getDownloadMock.mockResolvedValueOnce(downloadDetail());
    await api.reloadDownload(true);

    getDownloadMock.mockRejectedValueOnce(new Error("missing"));
    const result = await api.reloadDownload(true);

    expect(result).toBeNull();
    expect(api.download.value?.completedChapters).toBe(1);
  });
});
