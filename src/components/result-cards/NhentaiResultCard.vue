<script setup lang="ts">
import { computed } from "vue";
import type { NhentaiSearchItem } from "../../api/comics";
import { useAppI18n } from "../../i18n";
import ResultCardShell from "./ResultCardShell.vue";

const props = defineProps<{
  item: NhentaiSearchItem;
}>();

const { t } = useAppI18n();

const extra = computed(() => props.item.extra);

const japaneseTitle = computed(() => {
  const title = extra.value.japaneseTitle;
  return title && title !== props.item.title ? title : "";
});

const statsLine = computed(() =>
  [
    extra.value.numPages === null
      ? ""
      : t("home.search.pages", { count: extra.value.numPages }),
    extra.value.numFavorites === null
      ? ""
      : t("home.search.favorites", { count: extra.value.numFavorites }),
  ]
    .filter(Boolean)
    .join(" · "),
);
</script>

<template>
  <ResultCardShell
    :platform="item.platform"
    :thumb="item.thumb"
    :title="item.title"
  >
    <p v-if="japaneseTitle" lang="ja" class="meta-line" :title="japaneseTitle">
      {{ japaneseTitle }}
    </p>

    <p v-if="statsLine" class="meta-line" :title="statsLine">
      {{ statsLine }}
    </p>
  </ResultCardShell>
</template>

<style scoped>
.meta-line {
  margin: 0;
  overflow: hidden;
  font-size: 0.8rem;
  line-height: 1.4;
  opacity: 0.68;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
