<script setup lang="ts">
import { computed } from "vue";
import type { DownloadDetail } from "../../api/downloads";
import { useAppI18n } from "../../i18n";
import DetailSection from "./DetailSection.vue";
import DetailShell from "./DetailShell.vue";

const props = defineProps<{
  detail: DownloadDetail;
}>();

const emit = defineEmits<{
  openChapter: [chapterId: string];
}>();

const { t } = useAppI18n();

interface OfflineChapter {
  id: string;
  title: string | null;
  number: number;
}

// Keep the stored order (and therefore the fallback number) instead of
// renumbering the downloaded subset from one.
const chapters = computed<OfflineChapter[]>(() => {
  const downloaded: OfflineChapter[] = [];
  props.detail.chapters.forEach((chapter, index) => {
    if (chapter.completed && chapter.id !== null) {
      downloaded.push({
        id: chapter.id,
        title: chapter.title,
        number: index + 1,
      });
    }
  });
  return downloaded;
});

function chapterLabel(chapter: OfflineChapter) {
  const title = chapter.title?.trim() ?? "";
  return title || t("detail.chapterFallback", { number: chapter.number });
}
</script>

<template>
  <DetailShell
    :platform="detail.platform"
    :title="detail.title"
    :thumb="detail.thumb ?? ''"
    :local-path="detail.coverPath ?? undefined"
  >
    <template #summary>
      <div class="offline-summary">
        <NTag size="small" type="warning" :bordered="false">
          {{ t("detail.offline.tag") }}
        </NTag>
        <span class="offline-summary__count">
          {{
            t("detail.offline.summary", {
              done: detail.completedChapters,
              total: detail.totalChapters,
            })
          }}
        </span>
      </div>
      <p class="offline-notice">{{ t("detail.offline.notice") }}</p>
    </template>

    <DetailSection
      v-if="chapters.length > 0"
      :title="t('detail.offline.chapters')"
    >
      <div class="chapter-grid">
        <NButton
          v-for="chapter in chapters"
          :key="chapter.id"
          size="small"
          type="primary"
          secondary
          class="chapter-item"
          @click="emit('openChapter', chapter.id)"
        >
          <span class="chapter-name">{{ chapterLabel(chapter) }}</span>
        </NButton>
      </div>
    </DetailSection>
  </DetailShell>
</template>

<style scoped>
.offline-summary {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.5rem;
  margin-top: 0.6rem;
}

.offline-summary__count {
  font-size: 0.82rem;
  opacity: 0.72;
}

.offline-notice {
  margin: 0.6rem 0 0;
  font-size: 0.85rem;
  line-height: 1.55;
  opacity: 0.68;
}

.chapter-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(8.5rem, 1fr));
  gap: 0.4rem;
  padding-top: 0.15rem;
}

.chapter-name {
  display: inline-block;
  max-width: 14rem;
  overflow: hidden;
  text-overflow: ellipsis;
  vertical-align: bottom;
  white-space: nowrap;
}
</style>
