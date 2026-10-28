import { computed, ref, watch } from "vue";
import type { ComputedRef } from "vue";
import type { ComicPlatform } from "../platforms";
import { getDownload, type DownloadDetail } from "../api/downloads";
import { getHistoryEntry, type HistoryEntry } from "../api/history";
import { AppError, toAppError } from "../errors";
import { useAppI18n } from "../i18n";
import type { FavoriteSnapshot } from "./useFavoriteAction";
import { useLatestRequest } from "./useLatestRequest";

export interface UseDownloadedComicDetailOptions {
  platform: ComputedRef<ComicPlatform | null>;
  id: ComputedRef<string>;
}

export type DownloadedDetailState =
  | { kind: "loading" }
  | { kind: "ready"; download: DownloadDetail }
  | { kind: "error"; error: AppError }
  | { kind: "empty" };

/**
 * Data for the downloaded-work detail route. It reads the local download index
 * only: no online detail request, no online-to-offline fallback timer, and no
 * dependency on the search session.
 */
export function useDownloadedComicDetail(
  options: UseDownloadedComicDetailOptions,
) {
  const { t } = useAppI18n();
  const latest = useLatestRequest();

  const download = ref<DownloadDetail | null>(null);
  const historyEntry = ref<HistoryEntry | null>(null);
  const loading = ref(true);
  const error = ref<AppError | null>(null);

  const completedChapterIds = computed<string[]>(() => {
    const ids: string[] = [];
    for (const chapter of download.value?.chapters ?? []) {
      if (chapter.completed && chapter.id !== null) {
        ids.push(chapter.id);
      }
    }
    return ids;
  });

  const startChapterId = computed<string | null>(
    () => download.value?.readChapterId ?? completedChapterIds.value[0] ?? null,
  );

  const canRead = computed(
    () => (download.value?.completedChapters ?? 0) > 0,
  );

  const canResume = computed(
    () => canRead.value && historyEntry.value !== null,
  );

  const resumeChapterId = computed<string | null>(() => {
    const entry = historyEntry.value;
    if (!entry) {
      return startChapterId.value;
    }
    if (
      entry.chapterId &&
      completedChapterIds.value.includes(entry.chapterId)
    ) {
      return entry.chapterId;
    }
    return startChapterId.value;
  });

  const resumePage = computed(() => {
    const entry = historyEntry.value;
    if (!entry) {
      return 1;
    }
    if (!entry.chapterId) {
      // Single-chapter platforms store an implicit chapter; its page is valid.
      return entry.page;
    }
    if (!completedChapterIds.value.includes(entry.chapterId)) {
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

  const favoriteSnapshot = computed<FavoriteSnapshot | null>(() => {
    const current = download.value;
    return current
      ? {
          platform: current.platform,
          title: current.title,
          thumb: current.thumb,
        }
      : null;
  });

  const state = computed<DownloadedDetailState>(() => {
    if (error.value) {
      return { kind: "error", error: error.value };
    }
    if (loading.value) {
      return { kind: "loading" };
    }
    if (download.value) {
      return { kind: "ready", download: download.value };
    }
    return { kind: "empty" };
  });

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
      // Reading progress is a convenience; the download detail still opens.
      if (latest.isCurrent(currentRequest)) {
        historyEntry.value = null;
      }
    }
  }

  async function load() {
    const currentRequest = latest.begin();
    const currentPlatform = options.platform.value;
    const currentId = options.id.value;

    loading.value = true;
    error.value = null;
    download.value = null;
    historyEntry.value = null;

    if (!currentPlatform || !currentId) {
      error.value = new AppError(
        "invalidInput",
        "invalid downloaded comic route",
        { field: "id" },
      );
      loading.value = false;
      return;
    }

    void loadHistoryState(currentPlatform, currentId, currentRequest);

    try {
      const saved = await getDownload(currentPlatform, currentId);
      if (!latest.isCurrent(currentRequest)) {
        return;
      }
      if (!saved) {
        error.value = new AppError("notFound", "download not found", {});
        return;
      }
      download.value = saved;
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
    download,
    historyEntry,
    loading,
    error,
    state,
    completedChapterIds,
    startChapterId,
    canRead,
    canResume,
    resumeChapterId,
    resumePage,
    resumeTitle,
    favoriteSnapshot,
    load,
    latest,
  };
}
