<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { ManwaDetail } from "./types";
import { useAppI18n } from "../../i18n";
import { usePlatformText } from "../../platforms/messages";
import { manwaMessages } from "./messages";
import DetailSection from "../../components/comic-detail/DetailSection.vue";
import DetailShell from "../../components/comic-detail/DetailShell.vue";
import DetailTagGroup from "../../components/comic-detail/DetailTagGroup.vue";

const props = defineProps<{
  detail: ManwaDetail;
  resumeChapterId?: string | null;
  /** Hide the online chapter list; the downloaded detail injects its own. */
  hideChapters?: boolean;
  /** Local cover file used by the downloaded-detail route. */
  localCoverPath?: string;
}>();

const emit = defineEmits<{
  openChapter: [chapterId: string];
}>();

const { t } = useAppI18n();
const text = usePlatformText(manwaMessages);

const GROUP_NAME = "chapters";
/** Long-running works list close to a thousand chapters; collapse those. */
const EXPAND_LIMIT = 100;

const extra = computed(() => props.detail.extra);
const chapters = computed(() =>
  props.detail.chapters.filter(
    (chapter): chapter is { id: string; title: string | null } =>
      chapter.id !== null,
  ),
);
const collapsedByDefault = computed(
  () => chapters.value.length > EXPAND_LIMIT,
);
const expandedNames = ref<string[]>([]);

watch(
  () => [props.detail, props.resumeChapterId],
  () => {
    expandedNames.value = collapsedByDefault.value ? [] : [GROUP_NAME];
  },
  { immediate: true },
);

function chapterLabel(chapter: { title: string | null }, index: number) {
  const title = chapter.title?.trim();
  return title && title.length > 0
    ? title
    : t("detail.chapterFallback", { number: index + 1 });
}

const status = computed(() => {
  if (extra.value.status === "连载中") {
    return text("statusOngoing");
  }
  if (extra.value.status === "已完结") {
    return text("statusCompleted");
  }
  return extra.value.status ?? "";
});

const statusType = computed<"success" | "default">(() =>
  extra.value.status === "连载中" ? "success" : "default",
);

const infoItems = computed(() =>
  [
    {
      label: t("detail.fields.authors"),
      value: extra.value.authors.join(", "),
    },
    {
      label: text("fieldsLatestChapter"),
      value: extra.value.latestChapter ?? "",
    },
    {
      label: text("fieldsUpdatedAt"),
      value: extra.value.updatedAt ?? "",
    },
    { label: text("fieldsRegion"), value: extra.value.region ?? "" },
    {
      label: text("fieldsSubscribers"),
      value: extra.value.subscribers ?? "",
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
      <div v-if="status" class="summary-row">
        <NTag size="small" :bordered="false" :type="statusType">
          {{ status }}
        </NTag>
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
      v-if="!hideChapters && chapters.length > 0"
      :title="t('detail.sections.chapters')"
    >
      <NCollapse v-model:expanded-names="expandedNames" arrow-placement="right">
        <NCollapseItem :name="GROUP_NAME">
          <template #header>
            <span class="chapter-group-title">
              {{ text("chapterGroupsAll") }}
            </span>
          </template>
          <template #header-extra>
            <NTag size="small" :bordered="false">
              {{ text("chapterGroupsCount", { count: chapters.length }) }}
            </NTag>
          </template>

          <div class="chapter-grid">
            <NButton
              v-for="(chapter, index) in chapters"
              :key="chapter.id"
              size="small"
              type="primary"
              secondary
              class="chapter-item"
              @click="emit('openChapter', chapter.id)"
            >
              <span class="chapter-name">
                {{ chapterLabel(chapter, index) }}
              </span>
            </NButton>
          </div>
        </NCollapseItem>
      </NCollapse>
    </DetailSection>

    <DetailSection
      v-if="extra.description"
      :title="t('detail.sections.description')"
    >
      <p class="detail-description">{{ extra.description }}</p>
    </DetailSection>

    <DetailTagGroup
      :title="text('sectionsCategories')"
      :tags="extra.categories"
    />
    <DetailTagGroup :title="text('sectionsTags')" :tags="extra.tags" />
    <DetailTagGroup :title="text('sectionsAliases')" :tags="extra.aliases" />
  </DetailShell>
</template>

<style scoped>
.summary-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
  margin-top: 0.6rem;
}

.detail-description {
  margin: 0;
  line-height: 1.7;
  opacity: 0.85;
  white-space: pre-line;
}

.chapter-group-title {
  display: block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
