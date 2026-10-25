<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import type { RouteLocationRaw } from "vue-router";
import { RouterLink, useRouter } from "vue-router";
import { useMessage } from "naive-ui";
import type { ComicPlatform } from "../api/comics";
import {
  clearHistory,
  listHistory,
  removeHistory,
  type HistoryEntry,
} from "../api/history";
import ComicThumb from "../components/ComicThumb.vue";
import { useErrorMessage } from "../composables/useErrorMessage";
import { toAppError, type AppError } from "../errors";
import { resolveLocale, useAppI18n, type MessageKey } from "../i18n";
import { useSettingsStore } from "../stores/settings";

const PAGE_SIZE = 50;

const router = useRouter();
const message = useMessage();
const settings = useSettingsStore();
const { t } = useAppI18n();
const formatError = useErrorMessage();

const entries = ref<HistoryEntry[]>([]);
const total = ref(0);
const page = ref(1);
const loading = ref(true);
const loadError = ref<AppError | null>(null);
const removingKey = ref<string | null>(null);
const clearing = ref(false);
let requestId = 0;

const pageCount = computed(() =>
  Math.max(1, Math.ceil(total.value / PAGE_SIZE)),
);
const countText = computed(() => t("history.count", { count: total.value }));
const errorText = computed(() =>
  loadError.value ? formatError(loadError.value) : "",
);

const PLATFORM_LABEL_KEYS: Record<ComicPlatform, MessageKey> = {
  manhuagui: "home.search.platforms.manhuagui",
  jm: "home.search.platforms.jm",
  nhentai: "home.search.platforms.nhentai",
};

// Rebuild the formatter when the app language changes instead of following
// the system locale, which may differ from the selected UI language.
const timestampFormatter = computed(
  () =>
    new Intl.DateTimeFormat(resolveLocale(settings.locale), {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      hour12: false,
    }),
);

function formatTimestamp(timestamp: number): string {
  return timestampFormatter.value.format(new Date(timestamp));
}

function isoTimestamp(timestamp: number): string {
  return new Date(timestamp).toISOString();
}

function entryKey(entry: HistoryEntry): string {
  return `${entry.platform}:${entry.id}`;
}

function resumeTarget(entry: HistoryEntry): RouteLocationRaw {
  const params: Record<string, string> = {
    platform: entry.platform,
    id: entry.id,
  };
  if (entry.chapterId) {
    params.chapterId = entry.chapterId;
  }

  return {
    name: "comic-reader",
    params,
    query: entry.page > 1 ? { page: String(entry.page) } : {},
  };
}

async function load() {
  const current = ++requestId;
  loading.value = true;
  loadError.value = null;

  try {
    const result = await listHistory(
      (page.value - 1) * PAGE_SIZE,
      PAGE_SIZE,
    );
    if (current !== requestId) {
      return;
    }
    total.value = result.total;
    entries.value = result.entries;
  } catch (cause) {
    if (current !== requestId) {
      return;
    }
    loadError.value = toAppError(cause);
  } finally {
    if (current === requestId) {
      loading.value = false;
    }
  }
}

function changePage(next: number) {
  if (next === page.value) {
    return;
  }
  page.value = next;
  // Pagination sits below a long list; start the new page from the top.
  window.scrollTo({ top: 0, behavior: "smooth" });
  void load();
}

async function remove(entry: HistoryEntry) {
  if (removingKey.value !== null) {
    return;
  }

  removingKey.value = entryKey(entry);
  try {
    await removeHistory(entry.platform, entry.id);
    // Deleting the last row of the last page would leave an empty page; step
    // back before asking for the replacement page.
    const remaining = Math.max(0, total.value - 1);
    const lastPage = Math.max(1, Math.ceil(remaining / PAGE_SIZE));
    if (page.value > lastPage) {
      page.value = lastPage;
    }
    await load();
  } catch (cause) {
    message.error(`${t("history.removeFailed")} ${formatError(cause)}`);
  } finally {
    removingKey.value = null;
  }
}

async function clearAll() {
  if (clearing.value) {
    return;
  }

  clearing.value = true;
  // Invalidate a list request that may still be in flight.
  requestId++;
  try {
    await clearHistory();
    page.value = 1;
    total.value = 0;
    entries.value = [];
    message.success(t("history.cleared"));
  } catch (cause) {
    message.error(`${t("history.clearFailed")} ${formatError(cause)}`);
  } finally {
    loading.value = false;
    clearing.value = false;
  }
}

function goHome() {
  void router.push({ name: "home" });
}

onMounted(() => {
  void load();
});

onBeforeUnmount(() => {
  requestId++;
});
</script>

<template>
  <main class="history-page">
    <header class="history-header">
      <NButton
        attr-type="button"
        quaternary
        circle
        size="large"
        class="history-back"
        :aria-label="t('history.back')"
        :title="t('history.back')"
        @click="goHome"
      >
        <template #icon>
          <svg
            class="history-back-icon"
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
      <h1>{{ t("history.title") }}</h1>

      <div class="history-clear">
        <NPopconfirm @positive-click="clearAll">
          <template #trigger>
            <NButton
              attr-type="button"
              quaternary
              type="error"
              :disabled="total === 0 || loading"
              :loading="clearing"
            >
              {{ t("history.clear") }}
            </NButton>
          </template>
          {{ t("history.clearConfirm") }}
        </NPopconfirm>
      </div>
    </header>

    <NCard class="history-card" size="small">
      <div class="history-toolbar">
        <span class="history-count">{{ countText }}</span>
      </div>

      <NDivider class="history-divider" />

      <NAlert
        v-if="loadError"
        class="history-alert"
        type="error"
        :bordered="false"
      >
        <div class="history-alert__body">
          <span>{{ errorText }}</span>
          <NButton size="small" @click="load">
            {{ t("history.retry") }}
          </NButton>
        </div>
      </NAlert>

      <div v-if="loading && !entries.length" class="history-loading" aria-busy="true">
        <NSpin size="medium" />
      </div>

      <NEmpty
        v-else-if="!entries.length && !loadError"
        class="history-empty"
        :description="t('history.emptyDescription')"
      />

      <div v-else-if="entries.length" class="history-list">
        <div
          v-for="entry in entries"
          :key="entryKey(entry)"
          class="history-entry"
        >
          <RouterLink class="history-entry__main" :to="resumeTarget(entry)">
            <span class="history-entry__cover">
              <ComicThumb
                :platform="entry.platform"
                :thumb="entry.thumb ?? ''"
                :title="entry.title"
              />
            </span>

            <span class="history-entry__body">
              <span class="history-entry__title" :title="entry.title">
                {{ entry.title }}
              </span>
              <span
                v-if="entry.chapterTitle"
                class="history-entry__chapter"
              >
                {{ entry.chapterTitle }}
              </span>
              <span class="history-entry__meta">
                <NTag size="tiny" :bordered="false">
                  {{ t(PLATFORM_LABEL_KEYS[entry.platform]) }}
                </NTag>
                <span>
                  {{
                    t("history.progress", {
                      page: entry.page,
                      total: entry.totalPages,
                    })
                  }}
                </span>
                <time :datetime="isoTimestamp(entry.updatedAt)">
                  {{ formatTimestamp(entry.updatedAt) }}
                </time>
              </span>
            </span>

            <span class="history-entry__resume" aria-hidden="true">
              {{ t("history.resume") }}
              <svg
                class="history-entry__chevron"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                focusable="false"
              >
                <path d="m9 5 7 7-7 7" />
              </svg>
            </span>
          </RouterLink>

          <NButton
            attr-type="button"
            quaternary
            circle
            size="small"
            class="history-entry__remove"
            :loading="removingKey === entryKey(entry)"
            :aria-label="t('history.remove')"
            :title="t('history.remove')"
            @click="remove(entry)"
          >
            <template #icon>
              <svg
                class="history-entry__trash"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
                focusable="false"
              >
                <path d="M4 7h16" />
                <path d="M9 7V4h6v3" />
                <path d="m6 7 1 13h10l1-13" />
                <path d="M10 11v5M14 11v5" />
              </svg>
            </template>
          </NButton>
        </div>
      </div>

      <div v-if="total > PAGE_SIZE" class="history-pagination">
        <NPagination
          :page="page"
          :page-count="pageCount"
          :page-slot="7"
          :disabled="loading"
          @update:page="changePage"
        />
      </div>
    </NCard>
  </main>
</template>

<style scoped>
.history-page {
  width: min(900px, calc(100% - 2.5rem));
  margin: 0 auto;
  padding: 1.25rem 1.25rem 4rem;
}

.history-header {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  margin-bottom: 1rem;
}

.history-back {
  flex: none;
  margin-left: -0.35rem;
}

.history-back-icon {
  display: block;
  width: 20px;
  height: 20px;
}

.history-header h1 {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.history-clear {
  margin-left: auto;
}

.history-card {
  margin: 0 auto;
  text-align: left;
}

.history-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  flex-wrap: wrap;
}

.history-count {
  font-size: 0.8rem;
  opacity: 0.7;
}

.history-divider {
  margin: 1rem 0;
}

.history-alert {
  margin-bottom: 0.75rem;
}

.history-alert__body {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  flex-wrap: wrap;
}

.history-loading {
  display: flex;
  justify-content: center;
  padding: 3rem 0;
}

.history-empty {
  padding: 2.5rem 0;
}

.history-list {
  display: flex;
  flex-direction: column;
}

.history-entry {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  align-items: center;
  gap: 0.25rem;
  border-bottom: 1px solid rgba(128, 128, 128, 0.18);
}

.history-entry:last-child {
  border-bottom: none;
}

.history-entry__main {
  display: grid;
  grid-template-columns: 64px minmax(0, 1fr) auto;
  align-items: center;
  gap: 0.85rem;
  min-width: 0;
  padding: 0.65rem 0.25rem;
  border-radius: 8px;
  color: inherit;
  text-decoration: none;
  transition: background-color 150ms ease;
}

.history-entry__main:hover {
  background-color: rgba(127, 127, 127, 0.08);
}

.history-entry__main:focus-visible {
  outline: 2px solid currentColor;
  outline-offset: 2px;
}

.history-entry__cover {
  width: 64px;
  aspect-ratio: 3 / 4;
  overflow: hidden;
  border-radius: 6px;
  background-color: rgba(127, 127, 127, 0.08);
}

.history-entry__cover :deep(.comic-thumb) {
  height: 100%;
}

.history-entry__cover :deep(.comic-thumb img) {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.history-entry__cover :deep(.comic-thumb__placeholder) {
  height: 100%;
  aspect-ratio: auto;
  font-size: 1.4rem;
}

.history-entry__body {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
  min-width: 0;
}

.history-entry__title {
  display: -webkit-box;
  overflow: hidden;
  font-size: 0.95rem;
  font-weight: 600;
  line-height: 1.35;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.history-entry__chapter {
  overflow: hidden;
  font-size: 0.8rem;
  text-overflow: ellipsis;
  white-space: nowrap;
  opacity: 0.75;
}

.history-entry__meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.76rem;
  opacity: 0.72;
}

.history-entry__meta time {
  font-variant-numeric: tabular-nums;
}

.history-entry__resume {
  display: inline-flex;
  align-items: center;
  gap: 0.15rem;
  font-size: 0.78rem;
  white-space: nowrap;
  opacity: 0.65;
}

.history-entry__chevron {
  display: block;
  width: 16px;
  height: 16px;
}

.history-entry__remove {
  flex: none;
  margin-right: 0.15rem;
}

.history-entry__trash {
  display: block;
  width: 18px;
  height: 18px;
}

.history-pagination {
  display: flex;
  justify-content: center;
  margin-top: 1rem;
}

@media (max-width: 560px) {
  .history-page {
    padding-top: 0.75rem;
  }

  .history-entry__main {
    grid-template-columns: 52px minmax(0, 1fr);
    gap: 0.65rem;
  }

  .history-entry__cover {
    width: 52px;
  }

  .history-entry__resume {
    display: none;
  }

  .history-entry__meta time {
    width: 100%;
  }
}
</style>
