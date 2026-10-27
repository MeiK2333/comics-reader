import { computed, onScopeDispose, ref, watch } from "vue";
import type { ComputedRef } from "vue";
import type { ComicDetail, ComicPlatform } from "../api/comics";
import { getComic } from "../api/comics";
import {
  getDownload,
  type DownloadChapterInput,
  type DownloadDetail,
} from "../api/downloads";
import { getHistoryEntry, type HistoryEntry } from "../api/history";
import { AppError, toAppError } from "../errors";
import { useAppI18n } from "../i18n";
import { setWindowTitle, updateWindowTitle } from "../router";
import { useLatestRequest } from "./useLatestRequest";
import type { DetailState } from "./detailState";
import { resolveDetailState } from "./detailState";

// A downloaded work should open without waiting for a failing network request,
// but a fast online response should still win without flashing the local view.
const OFFLINE_FALLBACK_DELAY_MS = 1_500;

export interface UseComicDetailOptions {
  platform: ComputedRef<ComicPlatform | null>;
  id: ComputedRef<string>;
}

/**
 * Owns the detail page's data state: the online request, the download index
 * used for offline fallback and the history row used by "continue reading".
 *
 * The view only renders `state`; actions such as favorites and downloads are
 * separate composables so they can be tested and changed independently.
 */
export function useComicDetail(options: UseComicDetailOptions) {
  const { t } = useAppI18n();
  const latest = useLatestRequest();

  const detail = ref<ComicDetail | null>(null);
  const download = ref<DownloadDetail | null>(null);
  const historyEntry = ref<HistoryEntry | null>(null);
  const loading = ref(true);
  const error = ref<AppError | null>(null);
  const offline = ref(false);
  const downloadLoading = ref(false);

  let offlineTimer: ReturnType<typeof setTimeout> | undefined;

  const manhuaguiChapters = computed(() => {
    const current = detail.value;
    return current?.platform === "manhuagui"
      ? current.extra.chapterGroups.flatMap((group) => group.chapters)
      : [];
  });

  const offlineChapterIds = computed<string[]>(() => {
    const ids: string[] = [];
    for (const chapter of download.value?.chapters ?? []) {
      if (chapter.completed && chapter.id !== null) {
        ids.push(chapter.id);
      }
    }
    return ids;
  });

  const downloadChapters = computed<DownloadChapterInput[]>(() => {
    const current = detail.value;
    if (!current) {
      return [];
    }
    if (current.platform === "manhuagui") {
      return manhuaguiChapters.value.map((chapter) => ({
        id: chapter.id,
        title: chapter.title,
      }));
    }
    if (current.platform === "jm") {
      return current.extra.chapters.map((chapter) => ({
        id: chapter.id,
        title: chapter.title,
      }));
    }
    return [];
  });

  const canRead = computed(() => {
    if (offline.value) {
      return (download.value?.completedChapters ?? 0) > 0;
    }
    const current = detail.value;
    if (!current || options.platform.value === null) {
      return false;
    }
    if (current.platform === "nhentai") {
      return true;
    }
    if (current.platform === "manhuagui") {
      return manhuaguiChapters.value.length > 0;
    }
    return current.platform === "jm" && current.extra.chapters.length > 0;
  });

  const canResume = computed(
    () => canRead.value && historyEntry.value !== null,
  );

  const resumeChapterId = computed<string | null>(() => {
    const entry = historyEntry.value;
    if (!entry) {
      return null;
    }

    // Offline progress can only resume inside chapters that are on disk.
    if (offline.value) {
      if (options.platform.value === "nhentai") {
        return null;
      }
      if (
        entry.chapterId &&
        offlineChapterIds.value.includes(entry.chapterId)
      ) {
        return entry.chapterId;
      }
      return offlineChapterIds.value[0] ?? null;
    }

    const current = detail.value;
    if (!current) {
      return null;
    }

    if (current.platform === "jm") {
      const chapters = current.extra.chapters;
      if (
        entry.chapterId &&
        chapters.some((chapter) => chapter.id === entry.chapterId)
      ) {
        return entry.chapterId;
      }
      return chapters[0]?.id ?? null;
    }
    if (current.platform === "manhuagui") {
      const chapters = manhuaguiChapters.value;
      if (
        entry.chapterId &&
        chapters.some((chapter) => chapter.id === entry.chapterId)
      ) {
        return entry.chapterId;
      }
      return chapters[0]?.id ?? null;
    }
    return entry.chapterId;
  });

  const resumePage = computed(() => {
    const entry = historyEntry.value;
    if (!entry) {
      return 1;
    }

    if (offline.value) {
      if (options.platform.value === "nhentai") {
        return entry.page;
      }
      if (
        !entry.chapterId ||
        !offlineChapterIds.value.includes(entry.chapterId)
      ) {
        return 1;
      }
      return entry.page;
    }

    const current = detail.value;
    if (!current) {
      return 1;
    }

    // A removed JM chapter cannot continue its old page; start the fallback
    // chapter at page one instead of showing an out-of-range page.
    if (
      current.platform === "jm" &&
      entry.chapterId &&
      !current.extra.chapters.some(
        (chapter) => chapter.id === entry.chapterId,
      )
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

  const state = computed<DetailState>(() =>
    resolveDetailState({
      detail: detail.value,
      download: download.value,
      error: error.value,
      offline: offline.value,
      loading: loading.value,
    }),
  );

  function clearOfflineFallback() {
    if (offlineTimer !== undefined) {
      clearTimeout(offlineTimer);
      offlineTimer = undefined;
    }
  }

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

    let loaded: DownloadDetail | null = null;
    try {
      const saved = await getDownload(comicPlatform, comicId);
      loaded = saved;
      if (latest.isCurrent(currentRequest)) {
        download.value = saved;
      }
    } catch {
      // Download status is auxiliary. The detail page stays usable when the
      // index is unavailable, and the download action reports its own errors.
      if (latest.isCurrent(currentRequest) && !silent) {
        download.value = null;
      }
    } finally {
      if (latest.isCurrent(currentRequest) && !silent) {
        downloadLoading.value = false;
      }
    }
    return loaded;
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
    offline.value = false;
    downloadLoading.value = false;
    clearOfflineFallback();
    // Drop a previous comic's dynamic title while the next request is running.
    updateWindowTitle();

    if (!currentPlatform || !currentId) {
      error.value = new AppError("invalidInput", "invalid comic detail route", {
        field: "id",
      });
      loading.value = false;
      return;
    }

    // Download and history live on disk, so load them even when the online
    // detail request fails and the page falls back to downloaded chapters.
    const downloadTask = loadDownloadState(
      currentPlatform,
      currentId,
      currentRequest,
    );
    void loadHistoryState(currentPlatform, currentId, currentRequest);

    offlineTimer = setTimeout(() => {
      void downloadTask.then((saved) => {
        if (
          !latest.isCurrent(currentRequest) ||
          detail.value ||
          error.value ||
          !saved ||
          saved.completedChapters === 0
        ) {
          return;
        }
        offline.value = true;
        loading.value = false;
        setWindowTitle(`${saved.title} · ${t("app.name")}`);
      });
    }, OFFLINE_FALLBACK_DELAY_MS);

    try {
      const result = await getComic(currentPlatform, currentId);
      if (!latest.isCurrent(currentRequest)) {
        return;
      }
      clearOfflineFallback();
      offline.value = false;
      detail.value = result;
      setWindowTitle(`${result.title} · ${t("app.name")}`);
    } catch (cause) {
      if (!latest.isCurrent(currentRequest)) {
        return;
      }
      clearOfflineFallback();
      const saved = await downloadTask;
      if (!latest.isCurrent(currentRequest)) {
        return;
      }
      if (saved && saved.completedChapters > 0) {
        offline.value = true;
        setWindowTitle(`${saved.title} · ${t("app.name")}`);
      } else {
        error.value = toAppError(cause);
        updateWindowTitle();
      }
    } finally {
      clearOfflineFallback();
      if (latest.isCurrent(currentRequest)) {
        loading.value = false;
      }
    }
  }

  watch([options.platform, options.id], () => void load(), { immediate: true });

  onScopeDispose(() => {
    clearOfflineFallback();
  });

  return {
    detail,
    download,
    historyEntry,
    loading,
    error,
    offline,
    downloadLoading,
    state,
    manhuaguiChapters,
    offlineChapterIds,
    downloadChapters,
    canRead,
    canResume,
    resumeChapterId,
    resumePage,
    resumeTitle,
    load,
    reloadDownload,
    latest,
  };
}
