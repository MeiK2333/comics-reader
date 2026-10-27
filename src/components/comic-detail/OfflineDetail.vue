<script setup lang="ts">
import { computed } from "vue";
import type { DownloadDetail } from "../../api/downloads";
import type { ComicDetail } from "../../platforms";
import PlatformDetail from "../../platforms/PlatformDetail.vue";
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

/**
 * The download index keeps the online detail captured at download time.
 * Render it through the same platform component as the online page, but with
 * the online chapter lists hidden: the stored chapters below are the only
 * ones that can be opened without a network request.
 */
const snapshot = computed<ComicDetail | null>(() => {
  const current = props.detail.detail;
  if (!current || current.platform !== props.detail.platform) {
    return null;
  }
  return {
    ...current,
    title: props.detail.title,
    thumb: props.detail.thumb ?? current.thumb,
  };
});

function chapterLabel(chapter: OfflineChapter) {
  const title = chapter.title?.trim() ?? "";
  return title || t("detail.chapterFallback", { number: chapter.number });
}
</script>

<template>
  <PlatformDetail
    v-if="snapshot"
    :detail="snapshot"
    :hide-chapters="true"
    :local-cover-path="detail.coverPath ?? undefined"
    @open-chapter="emit('openChapter', $event)"
  >
    <template #summary-extra>
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
    </template>

    <template #extra>
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
    </template>
  </PlatformDetail>

  <DetailShell
    v-else
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