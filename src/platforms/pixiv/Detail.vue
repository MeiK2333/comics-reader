<script setup lang="ts">
import { computed } from "vue";
import type { PixivDetail, PixivTag } from "./types";
import { useAppI18n } from "../../i18n";
import { usePlatformText } from "../../platforms/messages";
import { pixivMessages } from "./messages";

import DetailSection from "../../components/comic-detail/DetailSection.vue";
import DetailShell from "../../components/comic-detail/DetailShell.vue";
import DetailTagGroup from "../../components/comic-detail/DetailTagGroup.vue";

const props = defineProps<{
  detail: PixivDetail;
  /** Local cover file used by the downloaded-detail route. */
  localCoverPath?: string;
}>();

const { t } = useAppI18n();
const text = usePlatformText(pixivMessages);

const extra = computed(() => props.detail.extra);

const subtitle = computed(() => {
  const name = extra.value.userName.trim();
  const account = extra.value.userAccount.trim();
  if (!name) {
    return account ? `@${account}` : "";
  }
  return account ? `${name} (@${account})` : name;
});

const typeKey = computed<"typeIllust" | "typeManga" | "typeUgoira">(() => {
  if (extra.value.illustType === 1) {
    return "typeManga";
  }
  if (extra.value.illustType === 2) {
    return "typeUgoira";
  }
  return "typeIllust";
});

function formatCount(value: number | null): string {
  return value === null || value <= 0 ? "" : value.toLocaleString();
}

function formatDate(value: string): string {
  if (!value.trim()) {
    return "";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  const locale = document.documentElement.lang.trim() || "en";
  return new Intl.DateTimeFormat(locale, {
    dateStyle: "medium",
  }).format(date);
}

const size = computed(() => {
  const { width, height } = extra.value;
  return width > 0 && height > 0 ? `${width} × ${height}` : "";
});

const infoItems = computed(() =>
  [
    { label: text("fieldsType"), value: text(typeKey.value) },
    {
      label: t("detail.fields.pages"),
      value: extra.value.pageCount > 0 ? String(extra.value.pageCount) : "",
    },
    { label: text("fieldsSize"), value: size.value },
    {
      label: text("fieldsBookmarks"),
      value: formatCount(extra.value.bookmarkCount),
    },
    { label: text("fieldsLikes"), value: formatCount(extra.value.likeCount) },
    { label: text("fieldsViews"), value: formatCount(extra.value.viewCount) },
    { label: text("fieldsPosted"), value: formatDate(extra.value.createDate) },
    { label: text("fieldsSeries"), value: extra.value.seriesTitle ?? "" },
  ].filter((item) => item.value.trim().length > 0),
);

const tagLabels = computed(() =>
  extra.value.tags
    .map((tag: PixivTag) => {
      const translated = tag.translated?.trim() ?? "";
      return translated && translated !== tag.name
        ? `${tag.name} (${translated})`
        : tag.name;
    })
    .filter((label) => label.trim().length > 0),
);
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

    <DetailSection
      v-if="extra.description"
      :title="t('detail.sections.description')"
    >
      <p class="description">{{ extra.description }}</p>
    </DetailSection>

    <slot name="extra" />

    <DetailTagGroup
      v-if="tagLabels.length > 0"
      :title="text('tagGroup')"
      :tags="tagLabels"
    />
  </DetailShell>
</template>

<style scoped>
.description {
  margin: 0;
  line-height: 1.6;
  overflow-wrap: anywhere;
  white-space: pre-line;
}
</style>
