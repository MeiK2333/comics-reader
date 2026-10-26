<script setup lang="ts">
import { computed } from "vue";
import type { ManhuaguiDetail } from "../../api/comics";
import { useAppI18n } from "../../i18n";
import DetailSection from "./DetailSection.vue";
import DetailShell from "./DetailShell.vue";
import DetailTagGroup from "./DetailTagGroup.vue";

const props = defineProps<{
  detail: ManhuaguiDetail;
}>();

const emit = defineEmits<{
  openChapter: [chapterId: string];
}>();

const { t } = useAppI18n();

const extra = computed(() => props.detail.extra);
const chapters = computed(() => extra.value.chapters);

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

    <DetailSection
      v-if="chapters.length > 0"
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
          <span class="chapter-name">{{ chapter.title }}</span>
        </NButton>
      </div>
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

.chapter-list {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
  max-height: 320px;
  overflow-y: auto;
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
