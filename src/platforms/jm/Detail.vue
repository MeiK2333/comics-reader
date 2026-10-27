<script setup lang="ts">
import { computed } from "vue";
import type { JmDetail } from "./types";
import { useAppI18n } from "../../i18n";
import { usePlatformText } from "../../platforms/messages";
import { jmMessages } from "./messages";
import DetailSection from "../../components/comic-detail/DetailSection.vue";
import DetailShell from "../../components/comic-detail/DetailShell.vue";
import DetailTagGroup from "../../components/comic-detail/DetailTagGroup.vue";

const props = defineProps<{
  detail: JmDetail;
  /** Hide the online chapter list; the downloaded detail injects its own. */
  hideChapters?: boolean;
  /** Local cover file used by the downloaded-detail route. */
  localCoverPath?: string;
}>();

const emit = defineEmits<{
  openChapter: [chapterId: string];
}>();

const { t } = useAppI18n();
const text = usePlatformText(jmMessages);

const extra = computed(() => props.detail.extra);
const chapters = computed(() => extra.value.chapters);

function chapterLabel(chapter: JmDetail["extra"]["chapters"][number]) {
  const title = chapter.title.trim();
  return title.length > 0
    ? title
    : t("detail.chapterFallback", { number: chapter.sort });
}

const infoItems = computed(() =>
  [
    {
      label: t("detail.fields.authors"),
      value: extra.value.authors.join(", "),
    },
    {
      label: t("detail.fields.pages"),
      value: extra.value.pageCount?.toString() ?? "",
    },
    {
      label: text("fieldsPublishedAt"),
      value: extra.value.pubDate ?? "",
    },
    {
      label: text("fieldsUpdatedOn"),
      value: extra.value.updateDate ?? "",
    },
  ].filter((item) => item.value.trim().length > 0),
);

const stats = computed(() =>
  [
    { label: text("fieldsLikes"), value: extra.value.likes ?? "" },
    { label: text("fieldsViews"), value: extra.value.views ?? "" },
    {
      label: text("fieldsComments"),
      value: extra.value.commentCount?.toString() ?? "",
    },
  ].filter((item) => item.value.trim().length > 0),
);
</script>

<template>
  <DetailShell
    :platform="detail.platform"
    :title="detail.title"
    :thumb="detail.thumb"
    :local-path="localCoverPath"
  >
    <template #summary>
      <div v-if="stats.length > 0" class="detail-stats">
        <div v-for="stat in stats" :key="stat.label" class="detail-stat">
          <span>{{ stat.label }}</span>
          <strong>{{ stat.value }}</strong>
        </div>
      </div>
      <slot name="summary-extra" />
    </template>

    <DetailSection v-if="infoItems.length > 0" :title="t('detail.sections.info')">
      <NDescriptions :column="1" size="small" label-placement="left" bordered>
        <NDescriptionsItem
          v-for="item in infoItems"
          :key="item.label"
          :label="item.label"
        >
          {{ item.value }}
        </NDescriptionsItem>
      </NDescriptions>
    </DetailSection>

    <slot name="extra" />

    <DetailSection
      v-if="!hideChapters && chapters.length > 1"
      :title="t('detail.sections.chapters')"
    >
      <div class="chapter-list">
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

    <DetailTagGroup :title="text('sectionsWorks')" :tags="extra.works" />
    <DetailTagGroup :title="text('sectionsActors')" :tags="extra.actors" />
    <DetailTagGroup :title="text('sectionsTags')" :tags="extra.tags" />
  </DetailShell>
</template>

<style scoped>
.detail-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 1.25rem;
  margin-top: 1rem;
}

.detail-stat {
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}

.detail-stat span {
  font-size: 0.75rem;
  opacity: 0.6;
}

.detail-stat strong {
  font-size: 1.1rem;
  font-weight: 600;
}

.chapter-list {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
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
