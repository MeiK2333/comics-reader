<script setup lang="ts">
import { computed } from "vue";
import type { NhentaiDetail } from "../../api/comics";
import { useAppI18n } from "../../i18n";
import type { MessageKey } from "../../i18n";
import DetailSection from "./DetailSection.vue";
import DetailShell from "./DetailShell.vue";
import DetailTagGroup from "./DetailTagGroup.vue";

const props = defineProps<{
  detail: NhentaiDetail;
}>();

const { t } = useAppI18n();

const extra = computed(() => props.detail.extra);

// The dedicated cover CDN is not reachable through every proxy, while the
// thumbnail host is; the hero is only 240px wide, so the thumbnail is enough.
const cover = computed(() => props.detail.thumb);

const subtitle = computed(() => {
  const pretty = extra.value.prettyTitle;
  return pretty && pretty !== props.detail.title ? pretty : "";
});

const japaneseTitle = computed(() => {
  const japanese = extra.value.japaneseTitle;
  return japanese &&
    japanese !== props.detail.title &&
    japanese !== extra.value.prettyTitle
    ? japanese
    : "";
});

const uploadDate = computed(() => {
  const seconds = extra.value.uploadDate;
  if (seconds === null || seconds <= 0) {
    return "";
  }
  return new Intl.DateTimeFormat(document.documentElement.lang, {
    dateStyle: "medium",
  }).format(new Date(seconds * 1000));
});

const infoItems = computed(() =>
  [
    {
      label: t("detail.fields.pages"),
      value: extra.value.numPages?.toString() ?? "",
    },
    {
      label: t("detail.fields.favorites"),
      value: extra.value.numFavorites?.toString() ?? "",
    },
    { label: t("detail.fields.uploadedAt"), value: uploadDate.value },
    {
      label: t("detail.fields.scanlator"),
      value: extra.value.scanlator ?? "",
    },
  ].filter((item) => item.value.trim().length > 0),
);

const TAG_GROUP_KEYS = {
  language: "detail.tagGroups.language",
  artist: "detail.tagGroups.artist",
  tag: "detail.tagGroups.tag",
  parody: "detail.tagGroups.parody",
  character: "detail.tagGroups.character",
  group: "detail.tagGroups.group",
  category: "detail.tagGroups.category",
  other: "detail.tagGroups.other",
} as const satisfies Record<string, MessageKey>;

type TagGroupKey = keyof typeof TAG_GROUP_KEYS;

const TAG_GROUP_ORDER: readonly TagGroupKey[] = [
  "language",
  "artist",
  "tag",
  "parody",
  "character",
  "group",
  "category",
  "other",
];

function tagGroupKey(type: string): TagGroupKey {
  return type in TAG_GROUP_KEYS ? (type as TagGroupKey) : "other";
}

const tagGroups = computed(() => {
  const buckets = new Map<TagGroupKey, string[]>();

  for (const tag of extra.value.tags) {
    const key = tagGroupKey(tag.type);
    const names = buckets.get(key) ?? [];
    names.push(tag.name);
    buckets.set(key, names);
  }

  return TAG_GROUP_ORDER.filter(
    (key) => (buckets.get(key)?.length ?? 0) > 0,
  ).map((key) => ({
    key,
    title: t(TAG_GROUP_KEYS[key]),
    tags: buckets.get(key) ?? [],
  }));
});
</script>

<template>
  <DetailShell
    :platform="detail.platform"
    :title="detail.title"
    :thumb="cover"
    :subtitle="subtitle"
  >
    <template #summary>
      <p v-if="japaneseTitle" lang="ja" class="detail-japanese">
        {{ japaneseTitle }}
      </p>
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

    <DetailTagGroup
      v-for="group in tagGroups"
      :key="group.key"
      :title="group.title"
      :tags="group.tags"
    />
  </DetailShell>
</template>

<style scoped>
.detail-japanese {
  margin: 0.35rem 0 0;
  line-height: 1.5;
  opacity: 0.7;
  overflow-wrap: anywhere;
}
</style>
