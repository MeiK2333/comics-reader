import { computed, ref, watch } from "vue";
import type { ComputedRef } from "vue";
import type { ComicDetail, ComicPlatform } from "../platforms";
import { getComic } from "../api/comics";
import {
  getDownload,
  type DownloadChapterInput,
  type DownloadDetail,
} from "../api/downloads";
import { getHistoryEntry, type HistoryEntry } from "../api/history";
import { AppError, toAppError } from "../errors";
import { useAppI18n } from "../i18n";
import { useLatestRequest } from "./useLatestRequest";
import type { DetailState } from "./detailState";
import { resolveDetailState } from "./detailState";

export interface UseComicDetailOptions {
  platform: ComputedRef<ComicPlatform | null>;
  id: ComputedRef<string>;
}

/**
 * Online detail data. The download index is read only to drive the download
 * button; it is never used as a fallback detail surface. Downloaded works have
 * their own route and composable.
 */
export function useComicDetail(options: UseComicDetailOptions) {
  const { t } = useAppI18n();
  const latest = useLatestRequest();

  const detail = ref<ComicDetail | null>(null);
  const download = ref<DownloadDetail | null>(null);
  const historyEntry = ref<HistoryEntry | null>(null);
  const loading = ref(true);
  const error = ref<AppError | null>(null);
  const downloadLoading = ref(false);

  const chapters = computed(() => detail.value?.chapters ?? []);

  const downloadChapters = computed<DownloadChapterInput[]>(() =>
    chapters.value.flatMap((chapter) =>
      chapter.id === null
        ? []
        : [{ id: chapter.id, title: chapter.title }],
    ),
  );

  const canRead = computed(
    () =>
      detail.value !== null &&
      options.platform.value !== null &&
      chapters.value.length > 0,
  );

  const canResume = computed(
    () => canRead.value && historyEntry.value !== null,
  );

  const resumeChapterId = computed<string | null>(() => {
    const entry = historyEntry.value;
    if (!entry) {
      return null;
    }
    if (
      entry.chapterId &&
      chapters.value.some((chapter) => chapter.id === entry.chapterId)
    ) {
      return entry.chapterId;
    }
    return chapters.value[0]?.id ?? null;
  });

  const resumePage = computed(() => {
    const entry = historyEntry.value;
    if (!entry) {
      return 1;
    }

    // A removed chapter cannot continue its old page; start the fallback
    // chapter at page one instead of showing an out-of-range page.
    if (
      entry.chapterId &&
      !chapters.value.some((chapter) => chapter.id === entry.chapterId)
    ) {
      return 1;
    }
    return entry.page;
  });

  const resumeTitle = computed(() => {
    const entry = historyEntry.value;
    return entry
      ? t("detail.continueAt", {
          page: resumePage.value,
          total: entry.totalPages,
        })
      : t("detail.continueReading");
  });

  const hasDownload = computed(
    () => (download.value?.completedChapters ?? 0) > 0,
  );

  const state = computed<DetailState>(() =>
    resolveDetailState({
      detail: detail.value,
      error: error.value,
      loading: loading.value,
    }),
  );

  async function loadHistoryState(
    comicPlatform: ComicPlatform,
    comicId: string,
    currentRequest: number,
  ) {
    try {
      const entry = await getHistoryEntry(comicPlatform, comicId);
      if (latest.isCurrent(currentRequest)) {
        historyEntry.value = entry;
      }
    } catch {
      // History only adds a convenience button; never block the detail page.
      if (latest.isCurrent(currentRequest)) {
        historyEntry.value = null;
      }
    }
  }

  async function loadDownloadState(
    comicPlatform: ComicPlatform,
    comicId: string,
    currentRequest: number,
    silent = false,
  ): Promise<DownloadDetail | null> {
    if (!silent) {
      downloadLoading.value = true;
    }

    try {
      const saved = await getDownload(comicPlatform, comicId);
      if (latest.isCurrent(currentRequest)) {
        download.value = saved;
      }
      return saved;
    } catch {
      // Download status is auxiliary. The online detail stays usable when the
      // local index is unavailable; the download action reports its own errors.
      if (latest.isCurrent(currentRequest) && !silent) {
        download.value = null;
      }
      return null;
    } finally {
      if (latest.isCurrent(currentRequest) && !silent) {
        downloadLoading.value = false;
      }
    }
  }

  /** Re-read the download index for the current work after an action. */
  async function reloadDownload(silent = false): Promise<DownloadDetail | null> {
    const currentPlatform = options.platform.value;
    const currentId = options.id.value;
    if (!currentPlatform || !currentId) {
      return null;
    }
    return loadDownloadState(
      currentPlatform,
      currentId,
      latest.snapshot(),
      silent,
    );
  }

  async function load() {
    const currentRequest = latest.begin();
    const currentPlatform = options.platform.value;
    const currentId = options.id.value;

    loading.value = true;
    error.value = null;
    detail.value = null;
    historyEntry.value = null;
    download.value = null;
    downloadLoading.value = false;

    if (!currentPlatform || !currentId) {
      error.value = new AppError("invalidInput", "invalid comic detail route", {
        field: "id",
      });
      loading.value = false;
      return;
    }

    // Download and history are separate auxiliary reads. They never determine
    // which detail surface is rendered.
    void loadDownloadState(currentPlatform, currentId, currentRequest);
    void loadHistoryState(currentPlatform, currentId, currentRequest);

    try {
      const result = await getComic(currentPlatform, currentId);
      if (!latest.isCurrent(currentRequest)) {
        return;
      }
      detail.value = result;
    } catch (cause) {
      if (!latest.isCurrent(currentRequest)) {
        return;
      }
      error.value = toAppError(cause);
    } finally {
      if (latest.isCurrent(currentRequest)) {
        loading.value = false;
      }
    }
  }

  watch([options.platform, options.id], () => void load(), { immediate: true });

  return {
    detail,
    download,
    historyEntry,
    loading,
    error,
    downloadLoading,
    state,
    chapters,
    downloadChapters,
    canRead,
    canResume,
    resumeChapterId,
    resumePage,
    resumeTitle,
    hasDownload,
    load,
    reloadDownload,
    latest,
  };
}
