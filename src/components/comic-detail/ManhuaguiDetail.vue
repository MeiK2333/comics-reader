<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { ManhuaguiChapterGroup, ManhuaguiDetail } from "../../api/comics";
import { useAppI18n } from "../../i18n";
import DetailSection from "./DetailSection.vue";
import DetailShell from "./DetailShell.vue";
import DetailTagGroup from "./DetailTagGroup.vue";

const props = defineProps<{
  detail: ManhuaguiDetail;
  resumeChapterId?: string | null;
}>();

const emit = defineEmits<{
  openChapter: [chapterId: string];
}>();

const { t } = useAppI18n();

const extra = computed(() => props.detail.extra);
const groups = computed(() =>
  extra.value.chapterGroups.filter((group) => group.chapters.length > 0),
);
const hasToolbar = computed(() => groups.value.length > 1);
const expandedNames = ref<string[]>([]);

function groupName(index: number) {
  return `chapter-group-${index}`;
}

function initialExpandedNames() {
  if (groups.value.length === 0) {
    return [];
  }
  const resume = props.resumeChapterId;
  const resumeIndex = resume
    ? groups.value.findIndex((group) =>
        group.chapters.some((chapter) => chapter.id === resume),
      )
    : -1;
  return [groupName(resumeIndex >= 0 ? resumeIndex : 0)];
}

function groupTitle(group: ManhuaguiChapterGroup, index: number) {
  const title = group.title.trim();
  if (title) {
    return title;
  }
  if (groups.value.length === 1) {
    return t("detail.chapterGroups.all");
  }
  const unlabelledIndex = groups.value
    .slice(0, index + 1)
    .filter((item) => item.title.trim().length === 0).length;
  return t("detail.chapterGroups.other", { index: unlabelledIndex });
}

function expandAll() {
  expandedNames.value = groups.value.map((_, index) => groupName(index));
}

function collapseAll() {
  expandedNames.value = [];
}

watch(
  () => [props.detail, props.resumeChapterId],
  () => {
    expandedNames.value = initialExpandedNames();
  },
  { immediate: true },
);

const subtitle = computed(() => {
  const original = extra.value.originalTitle;
  return original && original !== props.detail.title ? original : "";
});

const status = computed(() => {
  if (extra.value.status === "连载中") {
    return t("home.search.statusOngoing");
  }
  if (extra.value.status === "已完结") {
    return t("home.search.statusCompleted");
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
      label: t("detail.fields.latestChapter"),
      value: extra.value.latestChapter ?? "",
    },
    {
      label: t("detail.fields.updatedAt"),
      value: extra.value.updatedAt ?? "",
    },
    { label: t("detail.fields.year"), value: extra.value.year ?? "" },
    { label: t("detail.fields.region"), value: extra.value.region ?? "" },
  ].filter((item) => item.value.trim().length > 0),
);
</script>

<template>
  <DetailShell
    :platform="detail.platform"
    :title="detail.title"
    :thumb="detail.thumb"
    :subtitle="subtitle"
  >
    <template #summary>
      <div v-if="status" class="summary-row">
        <NTag size="small" :bordered="false" :type="statusType">
          {{ status }}
        </NTag>
      </div>
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

    <DetailSection v-if="groups.length > 0" :title="t('detail.sections.chapters')">
      <div v-if="hasToolbar" class="chapter-toolbar">
        <NButton size="small" quaternary @click="expandAll">
          {{ t("detail.chapterGroups.expandAll") }}
        </NButton>
        <NButton size="small" quaternary @click="collapseAll">
          {{ t("detail.chapterGroups.collapseAll") }}
        </NButton>
      </div>

      <NCollapse
        v-model:expanded-names="expandedNames"
        arrow-placement="right"
        class="chapter-collapse"
      >
        <NCollapseItem
          v-for="(group, index) in groups"
          :key="groupName(index)"
          :name="groupName(index)"
        >
          <template #header>
            <span class="chapter-group-title">{{ groupTitle(group, index) }}</span>
          </template>
          <template #header-extra>
            <NTag size="small" :bordered="false">
              {{ t("detail.chapterGroups.count", { count: group.chapters.length }) }}
            </NTag>
          </template>

          <div class="chapter-grid">
            <NButton
              v-for="chapter in group.chapters"
              :key="chapter.id"
              size="small"
              type="primary"
              secondary
              class="chapter-item"
              @click="emit('openChapter', chapter.id)"
            >
              <span class="chapter-name">{{ chapter.title }}</span>
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
      :title="t('detail.sections.categories')"
      :tags="extra.categories"
    />
    <DetailTagGroup
      :title="t('detail.sections.aliases')"
      :tags="extra.aliases"
    />
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

.chapter-toolbar {
  display: flex;
  justify-content: flex-end;
  gap: 0.25rem;
  margin-bottom: 0.35rem;
}

.chapter-collapse {
  margin-top: 0.1rem;
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
