<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { RouterLink } from "vue-router";
import { useMessage } from "naive-ui";
import {
  deleteDownload,
  isDownloadActive,
  listDownloads,
  pauseDownload,
  resumeDownload,
  type DownloadStatus,
  type DownloadSummary,
} from "../api/downloads";
import { clearReaderChapterCache } from "../api/reader";
import ComicThumb from "../components/ComicThumb.vue";
import { useBackNavigation } from "../composables/useBackNavigation";
import { useErrorMessage } from "../composables/useErrorMessage";
import { useLatestRequest } from "../composables/useLatestRequest";
import { usePagedResource } from "../composables/usePagedResource";
import { useByteFormatter, useDateTimeFormatter } from "../i18n/format";
import { platformLabelKey } from "../i18n/platform";
import { useAppI18n, type MessageKey } from "../i18n";

const POLL_MS = 1_000;

const { goBack } = useBackNavigation();
const message = useMessage();
const { t } = useAppI18n();
const formatError = useErrorMessage();
const latest = useLatestRequest();
const { format: formatTimestamp, toIso: isoTimestamp } =
  useDateTimeFormatter();
const { format: formatBytes } = useByteFormatter();

const {
  entries,
  total,
  page,
  pageCount,
  loading,
  error: loadError,
  load: loadPage,
  changePage: changeResourcePage,
} = usePagedResource<DownloadSummary>({
  fetch: (offset, limit) => listDownloads(offset, limit),
});
const actionKey = ref<string | null>(null);
let pollTimer: ReturnType<typeof setInterval> | undefined;

const countText = computed(() => t("downloads.count", { count: total.value }));
const errorText = computed(() =>
  loadError.value ? formatError(loadError.value) : "",
);
const hasActive = computed(() =>
  entries.value.some((entry) => isDownloadActive(entry.status)),
);
const busy = computed(() => actionKey.value !== null);

const STATUS_LABEL_KEYS: Record<DownloadStatus, MessageKey> = {
  queued: "downloads.status.queued",
  downloading: "downloads.status.downloading",
  paused: "downloads.status.paused",
  completed: "downloads.status.completed",
  partial: "downloads.status.partial",
  failed: "downloads.status.failed",
};

type TagType = "default" | "info" | "success" | "warning" | "error";

const STATUS_TYPES: Record<DownloadStatus, TagType> = {
  queued: "default",
  downloading: "info",
  paused: "warning",
  completed: "success",
  partial: "warning",
  failed: "error",
};

function entryKey(entry: DownloadSummary): string {
  return `${entry.platform}:${entry.id}`;
}

function progressPercent(entry: DownloadSummary): number {
  if (entry.totalChapters === 0) {
    return 0;
  }
  const currentPage = entry.progress;
  const currentFraction =
    currentPage && currentPage.pageCount > 0
      ? currentPage.page / currentPage.pageCount
      : 0;
  return Math.min(
    100,
    Math.round(
      ((entry.completedChapters + currentFraction) / entry.totalChapters) * 100,
    ),
  );
}

function progressText(entry: DownloadSummary): string {
  const progress = entry.progress;
  if (!progress) {
    return "";
  }
  return `${t("downloads.chapterProgress", {
    current: progress.chapterIndex,
    total: progress.chapterCount,
  })} · ${t("downloads.pageProgress", {
    page: progress.page,
    total: progress.pageCount,
  })}`;
}

function clearPolling() {
  if (pollTimer !== undefined) {
    clearInterval(pollTimer);
    pollTimer = undefined;
  }
}

function syncPolling() {
  clearPolling();
  if (!hasActive.value) {
    return;
  }
  pollTimer = setInterval(() => {
    if (document.visibilityState !== "hidden") {
      void load(true);
    }
  }, POLL_MS);
}

async function load(silent = false) {
  await loadPage(silent);
  syncPolling();
}

async function changePage(next: number) {
  await changeResourcePage(next);
  syncPolling();
}

async function pause(entry: DownloadSummary) {
  if (busy.value) {
    return;
  }
  const key = entryKey(entry);
  actionKey.value = key;
  const token = latest.begin();

  try {
    await pauseDownload(entry.platform, entry.id);
    if (latest.isCurrent(token)) {
      message.success(t("downloads.pausedToast"));
      await load(true);
    }
  } catch (cause) {
    if (latest.isCurrent(token)) {
      message.error(formatError(cause));
    }
  } finally {
    actionKey.value = null;
  }
}

async function resume(entry: DownloadSummary) {
  if (busy.value) {
    return;
  }
  const key = entryKey(entry);
  actionKey.value = key;
  const token = latest.begin();

  try {
    await resumeDownload(entry.platform, entry.id);
    if (latest.isCurrent(token)) {
      message.success(t("downloads.resumedToast"));
      await load(true);
    }
  } catch (cause) {
    if (latest.isCurrent(token)) {
      message.error(formatError(cause));
    }
  } finally {
    actionKey.value = null;
  }
}

async function remove(entry: DownloadSummary) {
  if (busy.value || isDownloadActive(entry.status)) {
    return;
  }
  const key = entryKey(entry);
  actionKey.value = key;
  const token = latest.begin();

  try {
    await deleteDownload(entry.platform, entry.id);
    if (!latest.isCurrent(token)) {
      return;
    }
    clearReaderChapterCache();
    message.success(t("downloads.deleted"));
    await load();
  } catch (cause) {
    if (latest.isCurrent(token)) {
      message.error(formatError(cause));
    }
  } finally {
    actionKey.value = null;
  }
}

function goHome() {
  goBack({ name: "home" });
}

onMounted(() => {
  void load();
});

onBeforeUnmount(() => {
  clearPolling();
});
</script>

<template>
  <main class="downloads-page">
    <header class="downloads-header">
      <NButton
        attr-type="button"
        quaternary
        circle
        size="large"
        class="downloads-back"
        :aria-label="t('downloads.back')"
        :title="t('downloads.back')"
        @click="goHome"
      >
        <template #icon>
          <svg
            class="downloads-back-icon"
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
      <h1>{{ t("downloads.title") }}</h1>
    </header>

    <NCard class="downloads-card" size="small">
      <div class="downloads-toolbar">
        <span class="downloads-count">{{ countText }}</span>
      </div>

      <NDivider class="downloads-divider" />

      <NAlert v-if="loadError" class="downloads-alert" type="error" :bordered="false">
        <div class="downloads-alert__body">
          <span>{{ errorText }}</span>
          <NButton size="small" @click="load()">
            {{ t("downloads.retry") }}
          </NButton>
        </div>
      </NAlert>

      <div
        v-if="loading && !entries.length"
        class="downloads-loading"
        aria-busy="true"
      >
        <NSpin size="medium" />
      </div>

      <NEmpty
        v-else-if="!entries.length && !loadError"
        class="downloads-empty"
        :description="t('downloads.emptyDescription')"
      />

      <div v-else-if="entries.length" class="downloads-list">
        <div
          v-for="entry in entries"
          :key="entryKey(entry)"
          class="download-entry"
        >
          <RouterLink
            class="download-entry__main"
            :to="{
              name: 'comic-detail',
              params: { platform: entry.platform, id: entry.id },
              state: { from: 'downloads' },
            }"
          >
            <div class="download-entry__cover">
              <ComicThumb
                :platform="entry.platform"
                :thumb="entry.thumb ?? ''"
                :title="entry.title"
                :local-path="entry.coverPath ?? undefined"
              />
            </div>

            <div class="download-entry__body">
              <span class="download-entry__title" :title="entry.title">
                {{ entry.title }}
              </span>

              <span class="download-entry__meta">
                <NTag size="tiny" :bordered="false">
                  {{ t(platformLabelKey(entry.platform)) }}
                </NTag>
                <NTag
                  size="tiny"
                  :bordered="false"
                  :type="STATUS_TYPES[entry.status]"
                >
                  {{ t(STATUS_LABEL_KEYS[entry.status]) }}
                </NTag>
                <span>
                  {{
                    t("downloads.chapters", {
                      done: entry.completedChapters,
                      total: entry.totalChapters,
                    })
                  }}
                </span>
                <span>{{ formatBytes(entry.totalBytes) }}</span>
                <time :datetime="isoTimestamp(entry.updatedAt)">
                  {{ formatTimestamp(entry.updatedAt) }}
                </time>
              </span>

              <div
                v-if="isDownloadActive(entry.status) || entry.progress"
                class="download-entry__progress"
              >
                <NProgress
                  type="line"
                  :percentage="progressPercent(entry)"
                  :processing="entry.status === 'queued'"
                  :status="entry.status === 'failed' ? 'error' : undefined"
                  :show-indicator="false"
                />
                <span class="download-entry__progress-text">
                  {{ progressText(entry) }}
                </span>
              </div>

              <p
                v-if="entry.failedChapters > 0"
                class="download-entry__warning"
              >
                {{
                  t("downloads.failedChapters", {
                    count: entry.failedChapters,
                  })
                }}
              </p>

              <p v-if="entry.error" class="download-entry__error">
                {{ formatError(entry.error) }}
              </p>
            </div>
          </RouterLink>

          <div class="download-entry__actions">
            <NButton
              v-if="isDownloadActive(entry.status)"
              attr-type="button"
              size="small"
              :loading="actionKey === entryKey(entry)"
              :disabled="busy && actionKey !== entryKey(entry)"
              @click="pause(entry)"
            >
              {{ t("downloads.pause") }}
            </NButton>
            <NButton
              v-else-if="entry.status !== 'completed'"
              attr-type="button"
              size="small"
              :loading="actionKey === entryKey(entry)"
              :disabled="busy && actionKey !== entryKey(entry)"
              @click="resume(entry)"
            >
              {{ t("downloads.resume") }}
            </NButton>
            <NPopconfirm
              :positive-text="t('downloads.delete')"
              :negative-text="t('downloads.cancel')"
              @positive-click="remove(entry)"
            >
              <template #trigger>
                <NButton
                  attr-type="button"
                  size="small"
                  type="error"
                  quaternary
                  :loading="actionKey === entryKey(entry)"
                  :disabled="busy || isDownloadActive(entry.status)"
                >
                  {{ t("downloads.delete") }}
                </NButton>
              </template>
              {{ t("downloads.deleteConfirm") }}
            </NPopconfirm>
          </div>
        </div>
      </div>

      <div v-if="pageCount > 1" class="downloads-pagination">
        <NPagination
          :page="page"
          :page-count="pageCount"
          :page-slot="7"
          :disabled="loading || busy"
          @update:page="changePage"
        />
      </div>
    </NCard>
  </main>
</template>

<style scoped>
.downloads-page {
  width: min(960px, calc(100% - 2.5rem));
  margin: 0 auto;
  padding: 1.25rem 1.25rem 4rem;
}

.downloads-header {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  margin-bottom: 1rem;
}

.downloads-back {
  flex: none;
  margin-left: -0.35rem;
}

.downloads-back-icon {
  display: block;
  width: 20px;
  height: 20px;
}

.downloads-header h1 {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.downloads-card {
  margin: 0 auto;
  text-align: left;
}

.downloads-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
}

.downloads-count {
  font-size: 0.8rem;
  opacity: 0.7;
}

.downloads-divider {
  margin: 1rem 0;
}

.downloads-alert {
  margin-bottom: 0.75rem;
}

.downloads-alert__body {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  flex-wrap: wrap;
}

.downloads-loading {
  display: flex;
  justify-content: center;
  padding: 3rem 0;
}

.downloads-empty {
  padding: 2.5rem 0;
}

.downloads-list {
  display: flex;
  flex-direction: column;
}

.download-entry {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: start;
  gap: 0.85rem;
  padding: 0.75rem 0.25rem;
  border-bottom: 1px solid rgba(128, 128, 128, 0.18);
}

.download-entry:last-child {
  border-bottom: none;
}

.download-entry__main {
  display: grid;
  grid-template-columns: 64px minmax(0, 1fr);
  align-items: start;
  gap: 0.85rem;
  min-width: 0;
  padding: 0.1rem 0;
  border-radius: 8px;
  color: inherit;
  text-decoration: none;
  transition: background-color 150ms ease;
}

.download-entry__main:hover {
  background-color: rgba(127, 127, 127, 0.08);
}

.download-entry__main:focus-visible {
  outline: 2px solid currentColor;
  outline-offset: 2px;
}

.download-entry__cover {
  width: 64px;
  aspect-ratio: 3 / 4;
  overflow: hidden;
  border-radius: 6px;
  background-color: rgba(127, 127, 127, 0.08);
}

.download-entry__cover :deep(.comic-thumb) {
  height: 100%;
}

.download-entry__cover :deep(.comic-thumb img) {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.download-entry__cover :deep(.comic-thumb__placeholder) {
  height: 100%;
  aspect-ratio: auto;
  font-size: 1.4rem;
}

.download-entry__body {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  min-width: 0;
}

.download-entry__title {
  display: -webkit-box;
  overflow: hidden;
  font-size: 0.95rem;
  font-weight: 600;
  line-height: 1.35;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.download-entry__meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.76rem;
  opacity: 0.72;
}

.download-entry__meta time {
  font-variant-numeric: tabular-nums;
}

.download-entry__progress {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  max-width: 28rem;
  margin-top: 0.1rem;
}

.download-entry__progress-text {
  font-size: 0.74rem;
  font-variant-numeric: tabular-nums;
  opacity: 0.65;
}

.download-entry__warning {
  margin: 0;
  font-size: 0.76rem;
  color: #f0a020;
}

.download-entry__error {
  margin: 0;
  font-size: 0.76rem;
  color: #d03050;
}

.download-entry__actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: 0.35rem;
  max-width: 16rem;
}

.downloads-pagination {
  display: flex;
  justify-content: center;
  margin-top: 1rem;
}

@media (max-width: 700px) {
  .download-entry {
    grid-template-columns: minmax(0, 1fr);
    gap: 0.65rem;
  }

  .download-entry__main {
    grid-template-columns: 52px minmax(0, 1fr);
    gap: 0.65rem;
  }

  .download-entry__cover {
    width: 52px;
  }

  .download-entry__actions {
    grid-column: 1 / -1;
    justify-content: flex-start;
    max-width: none;
    padding-top: 0.15rem;
  }

  .download-entry__meta time {
    width: 100%;
  }
}

@media (max-width: 560px) {
  .downloads-page {
    padding-top: 0.75rem;
  }
}
</style>
