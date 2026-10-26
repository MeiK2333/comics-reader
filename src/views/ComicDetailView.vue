<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import type { RouteLocationRaw } from "vue-router";
import { useMessage } from "naive-ui";
import type {
  ComicDetail,
  ComicPlatform,
  JmDetail as JmDetailData,
  ManhuaguiDetail as ManhuaguiDetailData,
  NhentaiDetail as NhentaiDetailData,
} from "../api/comics";
import { getComic, isComicPlatform } from "../api/comics";
import {
  getDownload,
  isDownloadActive,
  startDownload,
  type DownloadChapterInput,
  type DownloadDetail,
} from "../api/downloads";
import {
  addFavorite,
  getFavorite,
  removeFavorite,
  type FavoriteEntry,
} from "../api/favorites";
import { getHistoryEntry, type HistoryEntry } from "../api/history";
import FavoriteGroupPicker from "../components/favorites/FavoriteGroupPicker.vue";
import FavoriteIcon from "../components/FavoriteIcon.vue";
import ShortcutIcon from "../components/ShortcutIcon.vue";
import ComicDetailSkeleton from "../components/comic-detail/ComicDetailSkeleton.vue";
import JmDetail from "../components/comic-detail/JmDetail.vue";
import ManhuaguiDetail from "../components/comic-detail/ManhuaguiDetail.vue";
import NhentaiDetail from "../components/comic-detail/NhentaiDetail.vue";
import OfflineDetail from "../components/comic-detail/OfflineDetail.vue";
import {
  previousPath,
  useBackNavigation,
} from "../composables/useBackNavigation";
import { useErrorMessage } from "../composables/useErrorMessage";
import { useLongPress } from "../composables/useLongPress";
import { AppError, toAppError } from "../errors";
import { useAppI18n } from "../i18n";
import { setWindowTitle, updateWindowTitle } from "../router";
import { useSearchStore } from "../stores/search";

const route = useRoute();
const router = useRouter();
const search = useSearchStore();
const { t } = useAppI18n();
const formatError = useErrorMessage();
const message = useMessage();
const { goBack: back } = useBackNavigation();

const detail = ref<ComicDetail | null>(null);
const loading = ref(true);
const error = ref<AppError | null>(null);
const favoriteEntry = ref<FavoriteEntry | null>(null);
const favoriteLoading = ref(false);
const favoritePending = ref(false);
const historyEntry = ref<HistoryEntry | null>(null);
const download = ref<DownloadDetail | null>(null);
const offline = ref(false);
const downloadLoading = ref(false);
const downloadPending = ref(false);
const showGroupPicker = ref(false);
let requestId = 0;
let downloadTimer: ReturnType<typeof setInterval> | undefined;
let offlineTimer: ReturnType<typeof setTimeout> | undefined;

const DOWNLOAD_POLL_MS = 1_500;
// A downloaded work should open without waiting for a failing network request,
// but a fast online response should still win without flashing the local view.
const OFFLINE_FALLBACK_DELAY_MS = 1_500;

const favorited = computed(() => favoriteEntry.value !== null);
const currentGroupId = computed(() => favoriteEntry.value?.groupId ?? null);

// Remember where the detail page was opened from before a reader can push
// another entry on top of it.
const previous = previousPath();
const cameFromFavorites = previous === "/favorites";
const cameFromHistory = previous === "/history";
const cameFromDownloads = previous === "/downloads";

// Long-press opens the group picker; the short press is handled by click.
const longPress = useLongPress({
  disabled: () =>
    !detail.value || favoriteLoading.value || favoritePending.value,
  onLongPress: () => {
    if (detail.value) {
      showGroupPicker.value = true;
    }
  },
});

const platform = computed<ComicPlatform | null>(() => {
  const value = route.params.platform;
  return isComicPlatform(value) ? value : null;
});

const id = computed(() => {
  const value = route.params.id;
  return typeof value === "string" ? value : "";
});

const canReturnToSearch = computed(() => search.active);
const backTarget = computed<RouteLocationRaw>(() => {
  if (cameFromFavorites) {
    return { name: "favorites" };
  }
  if (cameFromHistory) {
    return { name: "history" };
  }
  if (cameFromDownloads) {
    return { name: "downloads" };
  }
  return canReturnToSearch.value ? { name: "search" } : { name: "home" };
});
const backLabel = computed(() => {
  if (cameFromFavorites) {
    return t("detail.backToFavorites");
  }
  if (cameFromHistory) {
    return t("detail.backToHistory");
  }
  if (cameFromDownloads) {
    return t("detail.backToDownloads");
  }
  return t(canReturnToSearch.value ? "detail.backToSearch" : "detail.backHome");
});
const manhuaguiChapters = computed(() => {
  const current = detail.value;
  return current?.platform === "manhuagui"
    ? current.extra.chapterGroups.flatMap((group) => group.chapters)
    : [];
});

// nhentai has a single implicit chapter; jm and manhuagui expose chapters
// through the detail payload, so the entry only appears once one exists.
const canRead = computed(() => {
  if (offline.value) {
    return (download.value?.completedChapters ?? 0) > 0;
  }
  const current = detail.value;
  if (!current || platform.value === null) {
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

const offlineDetail = computed<DownloadDetail | null>(() =>
  offline.value ? download.value : null,
);

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

const downloadInProgress = computed(
  () => download.value !== null && isDownloadActive(download.value.status),
);
const downloadButtonLabel = computed(() => {
  const entry = download.value;
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

const favoriteButtonText = computed(() =>
  favorited.value ? t("favorites.added") : t("favorites.add"),
);
const favoriteActionLabel = computed(() =>
  favorited.value ? t("favorites.remove") : t("favorites.addTitle"),
);

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
    if (platform.value === "nhentai") {
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
    if (entry.chapterId && chapters.some((chapter) => chapter.id === entry.chapterId)) {
      return entry.chapterId;
    }
    return chapters[0]?.id ?? null;
  }
  if (current.platform === "manhuagui") {
    const chapters = manhuaguiChapters.value;
    if (entry.chapterId && chapters.some((chapter) => chapter.id === entry.chapterId)) {
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
    if (platform.value === "nhentai") {
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
    !current.extra.chapters.some((chapter) => chapter.id === entry.chapterId)
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

const manhuaguiDetail = computed<ManhuaguiDetailData | null>(() =>
  detail.value?.platform === "manhuagui" ? detail.value : null,
);
const jmDetail = computed<JmDetailData | null>(() =>
  detail.value?.platform === "jm" ? detail.value : null,
);
const nhentaiDetail = computed<NhentaiDetailData | null>(() =>
  detail.value?.platform === "nhentai" ? detail.value : null,
);

const notFound = computed(
  () =>
    error.value?.code === "notFound" || error.value?.code === "invalidInput",
);
const errorTitle = computed(() =>
  notFound.value ? t("detail.notFound") : t("detail.loadFailed"),
);
const errorText = computed(() => {
  const current = error.value;
  if (!current) {
    return "";
  }
  return notFound.value
    ? t("detail.notFoundDescription")
    : formatError(current);
});

function goBack() {
  back(backTarget.value);
}

function openChapter(chapterId: string) {
  const currentPlatform = platform.value;
  if (!currentPlatform) {
    return;
  }
  void router.push({
    name: "comic-reader",
    params: {
      platform: currentPlatform,
      id: id.value,
      chapterId,
    },
  });
}

function openReader() {
  const currentPlatform = platform.value;
  if (!currentPlatform) {
    return;
  }

  // Offline start uses the first chapter that is actually on disk.
  if (offline.value) {
    const params: Record<string, string> = {
      platform: currentPlatform,
      id: id.value,
    };
    const chapterId = offlineChapterIds.value[0];
    if (chapterId) {
      params.chapterId = chapterId;
    }
    void router.push({ name: "comic-reader", params });
    return;
  }

  const current = detail.value;
  if (!current) {
    return;
  }

  // Chaptered platforms start at the first chapter and let the reader navigate
  // from there.
  if (current.platform === "jm") {
    const first = current.extra.chapters[0];
    if (first) {
      openChapter(first.id);
    }
    return;
  }
  if (current.platform === "manhuagui") {
    const first = manhuaguiChapters.value[0];
    if (first) {
      openChapter(first.id);
    }
    return;
  }

  void router.push({
    name: "comic-reader",
    params: { platform: current.platform, id: id.value },
  });
}

function openResume() {
  const currentPlatform = platform.value;
  const entry = historyEntry.value;
  if (!currentPlatform || !entry) {
    return;
  }

  const params: Record<string, string> = {
    platform: currentPlatform,
    id: id.value,
  };
  const chapterId = resumeChapterId.value;
  if (chapterId) {
    params.chapterId = chapterId;
  }
  const page = resumePage.value;

  void router.push({
    name: "comic-reader",
    params,
    query: page > 1 ? { page: String(page) } : {},
  });
}

async function loadFavoriteState(
  comicPlatform: ComicPlatform,
  comicId: string,
  currentRequest: number,
) {
  favoriteLoading.value = true;

  try {
    const saved = await getFavorite(comicPlatform, comicId);
    if (currentRequest === requestId) {
      favoriteEntry.value = saved;
    }
  } catch {
    // The status lookup is auxiliary. The toggle is idempotent, so a failure
    // should leave the button usable instead of blocking the detail page.
    if (currentRequest === requestId) {
      favoriteEntry.value = null;
    }
  } finally {
    if (currentRequest === requestId) {
      favoriteLoading.value = false;
    }
  }
}

async function loadHistoryState(
  comicPlatform: ComicPlatform,
  comicId: string,
  currentRequest: number,
) {
  try {
    const entry = await getHistoryEntry(comicPlatform, comicId);
    if (currentRequest === requestId) {
      historyEntry.value = entry;
    }
  } catch {
    // History only adds a convenience button; never block the detail page.
    if (currentRequest === requestId) {
      historyEntry.value = null;
    }
  }
}

function clearOfflineFallback() {
  if (offlineTimer !== undefined) {
    clearTimeout(offlineTimer);
    offlineTimer = undefined;
  }
}

function clearDownloadPolling() {
  if (downloadTimer !== undefined) {
    clearInterval(downloadTimer);
    downloadTimer = undefined;
  }
}

function syncDownloadPolling() {
  clearDownloadPolling();
  const entry = download.value;
  const currentPlatform = platform.value;
  const currentId = id.value;
  if (
    !entry ||
    !isDownloadActive(entry.status) ||
    !currentPlatform ||
    !currentId
  ) {
    return;
  }

  downloadTimer = setInterval(() => {
    void loadDownloadState(currentPlatform, currentId, requestId, true);
  }, DOWNLOAD_POLL_MS);
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
    if (currentRequest === requestId) {
      download.value = saved;
    }
  } catch {
    // Download status is auxiliary. The detail page stays usable when the
    // index is unavailable, and the download action reports its own errors.
    if (currentRequest === requestId && !silent) {
      download.value = null;
    }
  } finally {
    if (currentRequest === requestId) {
      if (!silent) {
        downloadLoading.value = false;
      }
      syncDownloadPolling();
    }
  }
  return loaded;
}

async function onDownloadClick() {
  const current = detail.value;
  const currentPlatform = platform.value;
  const currentId = id.value;
  if (
    !current ||
    !currentPlatform ||
    !currentId ||
    downloadLoading.value ||
    downloadPending.value
  ) {
    return;
  }

  const entry = download.value;
  if (entry && (downloadInProgress.value || entry.status === "completed")) {
    void router.push({ name: "downloads" });
    return;
  }

  const currentRequest = requestId;
  downloadPending.value = true;

  try {
    await startDownload({
      platform: currentPlatform,
      id: currentId,
      title: current.title,
      thumb: current.thumb,
      chapters: downloadChapters.value,
    });
    if (currentRequest === requestId) {
      message.success(t("downloads.queuedToast"));
      // Re-read the detail so the stored chapter list stays in sync.
      await loadDownloadState(currentPlatform, currentId, currentRequest);
    }
  } catch (cause) {
    if (currentRequest === requestId) {
      message.error(formatError(cause));
    }
  } finally {
    if (currentRequest === requestId) {
      downloadPending.value = false;
    }
  }
}

function handleFavoriteClick() {
  if (longPress.consumeClick()) {
    return;
  }
  void toggleFavorite();
}

async function toggleFavorite() {
  const current = detail.value;
  if (!current || favoriteLoading.value || favoritePending.value) {
    return;
  }

  const currentRequest = requestId;
  favoritePending.value = true;

  try {
    if (favorited.value) {
      await removeFavorite(current.platform, id.value);
      if (currentRequest === requestId) {
        favoriteEntry.value = null;
        message.success(t("favorites.removedToast"));
      }
    } else {
      const saved = await addFavorite({
        platform: current.platform,
        id: id.value,
        title: current.title,
        thumb: current.thumb,
      });
      if (currentRequest === requestId) {
        favoriteEntry.value = saved;
        message.success(t("favorites.addedToast"));
      }
    }
  } catch (cause) {
    if (currentRequest === requestId) {
      message.error(formatError(cause));
    }
  } finally {
    if (currentRequest === requestId) {
      favoritePending.value = false;
    }
  }
}

function onGroupSaved(entry: FavoriteEntry) {
  favoriteEntry.value = entry;
  showGroupPicker.value = false;
}

async function load() {
  const currentRequest = ++requestId;
  const currentPlatform = platform.value;
  const currentId = id.value;

  loading.value = true;
  error.value = null;
  detail.value = null;
  favoriteEntry.value = null;
  favoriteLoading.value = false;
  favoritePending.value = false;
  historyEntry.value = null;
  download.value = null;
  offline.value = false;
  downloadLoading.value = false;
  downloadPending.value = false;
  clearDownloadPolling();
  clearOfflineFallback();
  showGroupPicker.value = false;
  // Drop a previous comic's dynamic title while the next request is running.
  updateWindowTitle();

  if (!currentPlatform || !currentId) {
    error.value = new AppError(
      "invalidInput",
      "invalid comic detail route",
      { field: "id" },
    );
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
        currentRequest !== requestId ||
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
    if (currentRequest !== requestId) {
      return;
    }
    clearOfflineFallback();
    offline.value = false;
    detail.value = result;
    setWindowTitle(`${result.title} · ${t("app.name")}`);
    void loadFavoriteState(currentPlatform, currentId, currentRequest);
  } catch (cause) {
    if (currentRequest !== requestId) {
      return;
    }
    clearOfflineFallback();
    const saved = await downloadTask;
    if (currentRequest !== requestId) {
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
    if (currentRequest === requestId) {
      loading.value = false;
    }
  }
}

watch([platform, id], () => void load(), { immediate: true });

onBeforeUnmount(() => {
  requestId++;
  clearDownloadPolling();
  clearOfflineFallback();
});
</script>

<template>
  <main class="detail-page">
    <header class="detail-topbar">
      <NButton
        attr-type="button"
        quaternary
        circle
        size="large"
        class="back-button"
        :aria-label="backLabel"
        :title="backLabel"
        @click="goBack"
      >
        <template #icon>
          <svg
            class="back-icon"
            viewBox="0 0 24 24"
            aria-hidden="true"
            focusable="false"
          >
            <path
              d="M15 5l-7 7 7 7"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </template>
      </NButton>

      <div v-if="detail || offlineDetail" class="detail-topbar-actions">
        <span v-if="detail" class="favorite-trigger" v-on="longPress.handlers">
          <NButton
            attr-type="button"
            :type="favorited ? 'primary' : 'default'"
            secondary
            :loading="favoriteLoading || favoritePending"
            :disabled="favoriteLoading || favoritePending"
            :aria-pressed="favorited"
            :aria-label="favoriteActionLabel"
            :aria-description="t('favorites.longPressHint')"
            aria-haspopup="dialog"
            aria-keyshortcuts="Shift+Enter"
            :title="favoriteActionLabel"
            class="favorite-button"
            @click="handleFavoriteClick"
          >
            <template #icon>
              <FavoriteIcon :filled="favorited" />
            </template>
            <span class="favorite-button__label">{{ favoriteButtonText }}</span>
          </NButton>
        </span>
        <NButton
          v-if="detail && canRead"
          attr-type="button"
          secondary
          :loading="downloadPending"
          :disabled="downloadLoading || downloadPending"
          :aria-label="downloadButtonLabel"
          :title="downloadButtonLabel"
          class="download-button"
          @click="onDownloadClick"
        >
          <template #icon>
            <ShortcutIcon name="downloads" />
          </template>
          <span class="download-button__label">{{ downloadButtonLabel }}</span>
        </NButton>
        <NButton
          v-if="canRead"
          attr-type="button"
          :type="canResume ? 'default' : 'primary'"
          :secondary="canResume"
          @click="openReader"
        >
          {{ t("detail.read") }}
        </NButton>
        <NButton
          v-if="canResume"
          attr-type="button"
          type="primary"
          :title="resumeTitle"
          @click="openResume"
        >
          {{ t("detail.continueReading") }}
        </NButton>
      </div>
      <div v-if="loading" class="detail-topbar-actions" aria-hidden="true">
        <NSkeleton round width="96px" height="34px" />
        <NSkeleton round width="80px" height="34px" />
      </div>
    </header>

    <FavoriteGroupPicker
      v-if="detail"
      v-model:show="showGroupPicker"
      :platform="detail.platform"
      :id="id"
      :title="detail.title"
      :thumb="detail.thumb"
      :favorited="favorited"
      :current-group-id="currentGroupId"
      @saved="onGroupSaved"
    />

    <ComicDetailSkeleton v-if="loading" />

    <OfflineDetail
      v-else-if="offlineDetail"
      :detail="offlineDetail"
      @open-chapter="openChapter"
    />

    <NResult
      v-else-if="error"
      class="detail-result"
      :status="notFound ? '404' : 'error'"
      :title="errorTitle"
      :description="errorText"
    >
      <template #footer>
        <div class="detail-actions">
          <NButton v-if="!notFound" type="primary" @click="load">
            {{ t("detail.retry") }}
          </NButton>
          <NButton quaternary @click="goBack">{{ backLabel }}</NButton>
        </div>
      </template>
    </NResult>

    <ManhuaguiDetail
      v-else-if="manhuaguiDetail"
      :detail="manhuaguiDetail"
      :resume-chapter-id="resumeChapterId"
      @open-chapter="openChapter"
    />
    <JmDetail v-else-if="jmDetail" :detail="jmDetail" @open-chapter="openChapter" />
    <NhentaiDetail v-else-if="nhentaiDetail" :detail="nhentaiDetail" />

    <NResult
      v-else
      status="404"
      :title="t('detail.notFound')"
      :description="t('detail.notFoundDescription')"
    />
  </main>
</template>

<style scoped>
.detail-page {
  width: min(960px, calc(100% - 2.5rem));
  margin: 0 auto;
  padding: 1.25rem 1.25rem 4rem;
}

.detail-topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  margin-bottom: 1rem;
}

.back-button {
  flex: none;
  margin-left: -0.35rem;
}

.back-icon {
  display: block;
  width: 20px;
  height: 20px;
}

.detail-topbar-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.favorite-trigger {
  display: inline-flex;
  touch-action: manipulation;
  user-select: none;
  -webkit-touch-callout: none;
}

.favorite-button__label,
.download-button__label {
  white-space: nowrap;
}

.download-button :deep(.shortcut-icon-svg) {
  width: 18px;
  height: 18px;
}

.detail-result {
  padding: 2rem 0;
}

.detail-actions {
  display: flex;
  justify-content: center;
  gap: 0.75rem;
}

@media (max-width: 560px) {
  .detail-page {
    padding-top: 0.75rem;
  }

  .favorite-button__label,
  .download-button__label {
    display: none;
  }

  /* With the label hidden, Naive UI's icon margin and horizontal padding make
     the icon look off-center in the still-wide button. Collapse it to a square
     icon target and hide the empty content flex item so only the icon (or
     loading spinner) remains centered. */
  .favorite-button,
  .download-button {
    width: 34px;
    min-width: 34px;
    height: 34px;
    padding: 0;
    justify-content: center;
  }

  .favorite-button :deep(.n-button__content),
  .download-button :deep(.n-button__content) {
    display: none;
  }

  .favorite-button :deep(.n-button__icon),
  .download-button :deep(.n-button__icon) {
    margin: 0;
  }
}
</style>
