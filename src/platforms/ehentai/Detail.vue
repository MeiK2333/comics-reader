<script setup lang="ts">
import { computed } from "vue";
import type { EhentaiDetail } from "./types";
import { useAppI18n } from "../../i18n";
import { usePlatformText } from "../../platforms/messages";
import { ehentaiMessages } from "./messages";

import DetailSection from "../../components/comic-detail/DetailSection.vue";
import DetailShell from "../../components/comic-detail/DetailShell.vue";
import DetailTagGroup from "../../components/comic-detail/DetailTagGroup.vue";

const props = defineProps<{
  detail: EhentaiDetail;
  /** Local cover file used by the downloaded-detail route. */
  localCoverPath?: string;
}>();

const { t } = useAppI18n();
const text = usePlatformText(ehentaiMessages);

const extra = computed(() => props.detail.extra);

const subtitle = computed(() => {
  const title = extra.value.titleJpn;
  return title && title !== props.detail.title ? title : "";
});

const languages = computed(() => extra.value.languages.join(", "));

function formatBytes(bytes: number | null): string {
  if (bytes === null || bytes <= 0) {
    return "";
  }
  const units = ["B", "KiB", "MiB", "GiB"];
  let value = bytes;
  let index = 0;
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024;
    index += 1;
  }
  return `${value.toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

function formatPosted(seconds: number | null): string {
  if (seconds === null || seconds <= 0) {
    return "";
  }
  const date = new Date(seconds * 1000);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  const locale = document.documentElement.lang.trim() || "en";
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "medium",
  }).format(date);
}

const infoItems = computed(() =>
  [
    { label: text("fieldsCategory"), value: extra.value.category },
    { label: text("fieldsLanguage"), value: languages.value },
    {
      label: t("detail.fields.pages"),
      value: extra.value.pages?.toString() ?? "",
    },
    {
      label: text("fieldsRating"),
      value:
        extra.value.rating === null ? "" : extra.value.rating.toFixed(2),
    },
    { label: text("fieldsUploader"), value: extra.value.uploader ?? "" },
    {
      label: text("fieldsPosted"),
      value: formatPosted(extra.value.posted),
    },
    {
      label: text("fieldsFileSize"),
      value: formatBytes(extra.value.fileSize),
    },
  ].filter((item) => item.value.trim().length > 0),
);

const TAG_GROUP_KEYS = {
  language: "tagGroupsLanguage",
  artist: "tagGroupsArtist",
  character: "tagGroupsCharacter",
  parody: "tagGroupsParody",
  group: "tagGroupsGroup",
  female: "tagGroupsFemale",
  male: "tagGroupsMale",
  mixed: "tagGroupsMixed",
  other: "tagGroupsOther",
} as const;

type TagGroupKey = keyof typeof TAG_GROUP_KEYS;

const TAG_GROUP_ORDER: readonly TagGroupKey[] = [
  "language",
  "artist",
  "character",
  "parody",
  "group",
  "female",
  "male",
  "mixed",
  "other",
];

function tagGroupKey(namespace: string): TagGroupKey {
  return namespace in TAG_GROUP_KEYS
    ? (namespace as TagGroupKey)
    : "other";
}

const tagGroups = computed(() => {
  const buckets = new Map<TagGroupKey, string[]>();

  for (const tag of extra.value.tags) {
    const key = tagGroupKey(tag.namespace);
    const names = buckets.get(key) ?? [];
    names.push(tag.name);
    buckets.set(key, names);
  }

  return TAG_GROUP_ORDER.filter(
    (key) => (buckets.get(key)?.length ?? 0) > 0,
  ).map((key) => ({
    key,
    title: text(TAG_GROUP_KEYS[key]),
    tags: buckets.get(key) ?? [],
  }));
});
</script>

<template>
  <DetailShell
    :platform="detail.platform"
    :title="detail.title"
    :thumb="detail.thumb"
    :subtitle="subtitle"
    :local-path="localCoverPath"
  >
    <template #summary>
      <div v-if="extra.expunged" class="summary-row">
        <NTag size="small" :bordered="false" type="warning">
          {{ text("expunged") }}
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

    <DetailTagGroup
      v-for="group in tagGroups"
      :key="group.key"
      :title="group.title"
      :tags="group.tags"
    />
  </DetailShell>
</template>

<style scoped>
.summary-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
  margin-top: 0.5rem;
}
</style>
