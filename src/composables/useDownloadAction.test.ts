import { enableAutoUnmount, mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { computed, defineComponent, h, nextTick, ref } from "vue";
import { NMessageProvider } from "naive-ui";
import type { ComicDetail, ComicPlatform } from "../api/comics";
import type {
  DownloadDetail,
  DownloadStatus,
  DownloadSummary,
} from "../api/downloads";
import { useLatestRequest } from "./useLatestRequest";

const startMock = vi.hoisted(() => vi.fn());
const pushMock = vi.hoisted(() => vi.fn());

vi.mock("../api/downloads", () => ({
  startDownload: startMock,
  isDownloadActive: (status: DownloadStatus) =>
    status === "queued" || status === "downloading",
}));

vi.mock("vue-router", () => ({
  useRouter: () => ({ push: pushMock }),
}));

import { useDownloadAction } from "./useDownloadAction";

enableAutoUnmount(afterEach);

const detail: ComicDetail = {
  platform: "nhentai",
  title: "Test work",
  thumb: "https://example.com/thumb.jpg",
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

function downloadDetail(
  overrides: Partial<DownloadDetail> = {},
): DownloadDetail {
  return {
    platform: "nhentai",
    id: "1",
    title: "Test work",
    thumb: null,
    coverPath: null,
    status: "queued",
    totalChapters: 1,
    completedChapters: 0,
    failedChapters: 0,
    downloadedPages: 0,
    totalBytes: 0,
    readChapterId: null,
    addedAt: 1,
    updatedAt: 2,
    error: null,
    progress: null,
    chapters: [],
    ...overrides,
  };
}

function summary(): DownloadSummary {
  return downloadDetail();
}

interface MountOptions {
  platform?: ComicPlatform;
  id?: string;
  detail?: ComicDetail | null;
  download?: DownloadDetail | null;
}

function mountAction(options: MountOptions = {}) {
  const platformValue = ref<ComicPlatform>(options.platform ?? "nhentai");
  const idValue = ref(options.id ?? "1");
  const platform = computed<ComicPlatform | null>(() => platformValue.value);
  const id = computed(() => idValue.value);
  const detailRef = ref<ComicDetail | null>(
    options.detail === undefined ? detail : options.detail,
  );
  const downloadRef = ref<DownloadDetail | null>(options.download ?? null);
  const downloadLoading = ref(false);
  const reloadDownload = vi.fn(async () => downloadRef.value);
  // Mirrors useComicDetail's guard: loading another work bumps the token.
  const guard = useLatestRequest();
  let api!: ReturnType<typeof useDownloadAction>;

  const Harness = defineComponent({
    setup() {
      api = useDownloadAction({
        platform,
        id,
        detail: detailRef,
        download: downloadRef,
        downloadLoading,
        downloadChapters: computed(() => [{ id: "1", title: "Chapter 1" }]),
        latest: guard,
        reloadDownload,
      });
      return () => h("div");
    },
  });

  const Wrapper = defineComponent({
    setup() {
      return () => h(NMessageProvider, null, { default: () => h(Harness) });
    },
  });

  const wrapper = mount(Wrapper);
  return {
    api,
    wrapper,
    platform: platformValue,
    id: idValue,
    detailRef,
    downloadRef,
    downloadLoading,
    reloadDownload,
    guard,
  };
}

beforeEach(() => {
  startMock.mockReset();
  pushMock.mockReset();
});

afterEach(() => {
  vi.useRealTimers();
  Object.defineProperty(document, "visibilityState", {
    configurable: true,
    value: "visible",
  });
});

describe("useDownloadAction", () => {
  it("starts the current work with its chapter list and reloads the detail", async () => {
    startMock.mockResolvedValue(summary());

    const { api, reloadDownload, downloadLoading } = mountAction();
    await api.onDownloadClick();

    expect(startMock).toHaveBeenCalledWith({
      platform: "nhentai",
      id: "1",
      title: "Test work",
      thumb: "https://example.com/thumb.jpg",
      chapters: [{ id: "1", title: "Chapter 1" }],
    });
    expect(reloadDownload).toHaveBeenCalledOnce();
    expect(api.pending.value).toBe(false);
    expect(downloadLoading.value).toBe(false);
  });

  it("clears pending after a failed start", async () => {
    startMock.mockRejectedValue(new Error("network down"));

    const { api, reloadDownload } = mountAction();
    await api.onDownloadClick();

    expect(api.pending.value).toBe(false);
    expect(reloadDownload).not.toHaveBeenCalled();
  });

  it("clears pending when the work changes while a start is in flight", async () => {
    let resolveStart!: (value: DownloadSummary) => void;
    startMock.mockReturnValue(
      new Promise<DownloadSummary>((resolve) => {
        resolveStart = resolve;
      }),
    );

    const { api, platform, id, guard, reloadDownload } = mountAction();
    const pendingStart = api.onDownloadClick();
    expect(api.pending.value).toBe(true);

    // useComicDetail.load() starts a request for the next work, which
    // invalidates the token the start was using.
    platform.value = "jm";
    id.value = "2";
    guard.begin();
    await nextTick();

    expect(api.pending.value).toBe(false);

    resolveStart(summary());
    await pendingStart;

    expect(api.pending.value).toBe(false);
    expect(reloadDownload).not.toHaveBeenCalled();
  });

  it("opens the downloads page instead of re-queuing an active work", async () => {
    const { api } = mountAction({
      download: downloadDetail({ status: "downloading" }),
    });

    await api.onDownloadClick();

    expect(pushMock).toHaveBeenCalledWith({ name: "downloads" });
    expect(startMock).not.toHaveBeenCalled();
  });

  it("polls an active download and stops once it completes", async () => {
    vi.useFakeTimers();
    const { downloadRef, reloadDownload } = mountAction({
      download: downloadDetail({ status: "downloading" }),
    });
    await nextTick();

    vi.advanceTimersByTime(1_500);
    expect(reloadDownload).toHaveBeenCalledWith(true);

    reloadDownload.mockClear();
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "hidden",
    });
    vi.advanceTimersByTime(1_500);
    expect(reloadDownload).not.toHaveBeenCalled();

    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      value: "visible",
    });
    downloadRef.value = downloadDetail({ status: "completed" });
    await nextTick();
    vi.advanceTimersByTime(1_500);
    expect(reloadDownload).not.toHaveBeenCalled();
  });

  it("clears the poll timer when the scope is disposed", async () => {
    vi.useFakeTimers();
    const { wrapper, reloadDownload } = mountAction({
      download: downloadDetail({ status: "queued" }),
    });
    await nextTick();

    wrapper.unmount();
    vi.advanceTimersByTime(5_000);

    expect(reloadDownload).not.toHaveBeenCalled();
  });
});
