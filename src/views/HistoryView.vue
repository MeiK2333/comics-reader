<script setup lang="ts">
import { computed, onMounted } from "vue";
import type { RouteLocationRaw } from "vue-router";
import { RouterLink } from "vue-router";
import { useBackNavigation } from "../composables/useBackNavigation";
import { listHistory, type HistoryEntry } from "../api/history";
import ComicThumb from "../components/ComicThumb.vue";
import { useErrorMessage } from "../composables/useErrorMessage";
import { usePagedResource } from "../composables/usePagedResource";
import { useDateTimeFormatter } from "../i18n/format";
import { platformLabel } from "../platforms";
import { useAppI18n } from "../i18n";

const { goBack } = useBackNavigation();
const { t } = useAppI18n();
const formatError = useErrorMessage();

const { format: formatTimestamp, toIso: isoTimestamp } =
  useDateTimeFormatter();
const {
  entries,
  total,
  page,
  pageCount,
  loading,
  error: loadError,
  load,
  reload,
  changePage,
} = usePagedResource<HistoryEntry>({
  fetch: (offset, limit) => listHistory(offset, limit),
});

const countText = computed(() => t("history.count", { count: total.value }));
const errorText = computed(() =>
  loadError.value ? formatError(loadError.value) : "",
);

function entryKey(entry: HistoryEntry): string {
  return `${entry.platform}:${entry.id}`;
}

function detailTarget(entry: HistoryEntry): RouteLocationRaw {
  return {
    name: "comic-detail",
    params: { platform: entry.platform, id: entry.id },
    state: { from: "history" },
  };
}

function goHome() {
  goBack({ name: "home" });
}

onMounted(() => {
  void load();
});
</script>

<template>
  <main class="history-page">
    <header class="history-header">
      <NButton attr-type="button" quaternary circle size="large" class="history-back" :aria-label="t('history.back')"
        :title="t('history.back')" @click="goHome">
        <template #icon>
          <svg class="history-back-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <path d="M15 5l-7 7 7 7" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"
              stroke-linejoin="round" />
          </svg>
        </template>
      </NButton>
      <h1>{{ t("history.title") }}</h1>
    </header>

    <NCard class="history-card" size="small">
      <div class="history-toolbar">
        <span class="history-count">{{ countText }}</span>
      </div>

      <NDivider class="history-divider" />

      <NAlert v-if="loadError" class="history-alert" type="error" :bordered="false">
        <div class="history-alert__body">
          <span>{{ errorText }}</span>
          <NButton size="small" @click="reload">
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
        :show-description="false"
      />

      <div v-else-if="entries.length" class="history-list">
        <div v-for="entry in entries" :key="entryKey(entry)" class="history-entry">
          <RouterLink class="history-entry__main" :to="detailTarget(entry)">
            <span class="history-entry__cover">
              <ComicThumb :platform="entry.platform" :thumb="entry.thumb ?? ''" :title="entry.title" />
            </span>

            <span class="history-entry__body">
              <span class="history-entry__title" :title="entry.title">
                {{ entry.title }}
              </span>
              <span v-if="entry.chapterTitle" class="history-entry__chapter">
                {{ entry.chapterTitle }}
              </span>
              <span class="history-entry__meta">
                <NTag size="tiny" :bordered="false">
                  {{ platformLabel(entry.platform) }}
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

          </RouterLink>
        </div>
      </div>

      <div v-if="pageCount > 1" class="history-pagination">
        <NPagination :page="page" :page-count="pageCount" :page-slot="7" :disabled="loading"
          @update:page="changePage" />
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

.history-entry__chevron {
  display: block;
  width: 16px;
  height: 16px;
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

  .history-entry__meta time {
    width: 100%;
  }
}
</style>
