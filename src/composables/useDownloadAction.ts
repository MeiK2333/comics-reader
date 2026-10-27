import { computed, onScopeDispose, ref, watch } from "vue";
import type { ComputedRef, Ref } from "vue";
import { useRouter } from "vue-router";
import { useMessage } from "naive-ui";
import type { ComicDetail, ComicPlatform } from "../platforms";
import {
  isDownloadActive,
  startDownload,
  type DownloadChapterInput,
  type DownloadDetail,
} from "../api/downloads";
import { useErrorMessage } from "./useErrorMessage";
import type { LatestRequest } from "./useLatestRequest";
import { useAppI18n } from "../i18n";

const DOWNLOAD_POLL_MS = 1_500;

export interface UseDownloadActionOptions {
  platform: ComputedRef<ComicPlatform | null>;
  id: ComputedRef<string>;
  detail: Ref<ComicDetail | null>;
  download: Ref<DownloadDetail | null>;
  downloadLoading: Ref<boolean>;
  downloadChapters: ComputedRef<DownloadChapterInput[]>;
  latest: LatestRequest;
  reloadDownload: (silent?: boolean) => Promise<DownloadDetail | null>;
}

/**
 * The download button on the detail page: start a work, poll while it is
 * active, and jump to the downloads page once it is queued/completed.
 */
export function useDownloadAction(options: UseDownloadActionOptions) {
  const router = useRouter();
  const message = useMessage();
  const { t } = useAppI18n();
  const formatError = useErrorMessage();
  const pending = ref(false);
  let pollTimer: ReturnType<typeof setInterval> | undefined;

  const inProgress = computed(
    () =>
      options.download.value !== null &&
      isDownloadActive(options.download.value.status),
  );

  const buttonLabel = computed(() => {
    const entry = options.download.value;
    if (!entry) {
      return t("detail.download");
    }
    if (entry.status === "queued") {
      return t("downloads.status.queued");
    }
    if (entry.status === "downloading") {
      return t("detail.downloading", {
        done: entry.completedChapters,
        total: entry.totalChapters,
      });
    }
    if (entry.status === "completed") {
      return t("detail.downloaded");
    }
    return t("detail.resumeDownload");
  });

  function clearPolling() {
    if (pollTimer !== undefined) {
      clearInterval(pollTimer);
      pollTimer = undefined;
    }
  }

  function syncPolling() {
    clearPolling();
    if (
      !options.download.value ||
      !isDownloadActive(options.download.value.status) ||
      !options.platform.value ||
      !options.id.value
    ) {
      return;
    }

    pollTimer = setInterval(() => {
      if (document.visibilityState !== "hidden") {
        void options.reloadDownload(true);
      }
    }, DOWNLOAD_POLL_MS);
  }

  // A start belongs to the work that was on screen. When the route switches to
  // another work, drop the pending flag immediately: the detail guard has
  // already invalidated the request token, so the `finally` in
  // `onDownloadClick` will never clear it, and the next work's button would
  // stay disabled.
  watch([options.platform, options.id], () => {
    pending.value = false;
    clearPolling();
  });

  // Polling follows the work as well as the status; without the work key a
  // switch would keep the previous work's timer until its status changed.
  watch(
    [() => options.download.value?.status, options.platform, options.id],
    syncPolling,
    { immediate: true },
  );

  onScopeDispose(() => {
    clearPolling();
  });

  async function onDownloadClick() {
    const current = options.detail.value;
    const comicPlatform = options.platform.value;
    const comicId = options.id.value;
    if (
      !current ||
      !comicPlatform ||
      !comicId ||
      options.downloadLoading.value ||
      pending.value
    ) {
      return;
    }

    const entry = options.download.value;
    if (entry && (inProgress.value || entry.status === "completed")) {
      void router.push({ name: "downloads" });
      return;
    }

    const token = options.latest.snapshot();
    pending.value = true;

    try {
      await startDownload({
        platform: comicPlatform,
        id: comicId,
        title: current.title,
        thumb: current.thumb,
        chapters: options.downloadChapters.value,
      });
      if (options.latest.isCurrent(token)) {
        message.success(t("downloads.queuedToast"));
        // Re-read the detail so the stored chapter list stays in sync.
        await options.reloadDownload();
      }
    } catch (cause) {
      if (options.latest.isCurrent(token)) {
        message.error(formatError(cause));
      }
    } finally {
      if (options.latest.isCurrent(token)) {
        pending.value = false;
      }
    }
  }

  return {
    downloadLoading: options.downloadLoading,
    pending,
    inProgress,
    buttonLabel,
    onDownloadClick,
  };
}
