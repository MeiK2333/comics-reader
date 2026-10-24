<script setup lang="ts">
import { computed } from "vue";
import type { JmSearchItem } from "../../api/comics";
import { useAppI18n } from "../../i18n";
import ResultCardShell from "./ResultCardShell.vue";

const props = defineProps<{
  item: JmSearchItem;
}>();

const { t } = useAppI18n();

const maxVisibleTags = 6;
const visibleTags = computed(() =>
  props.item.extra.tags.slice(0, maxVisibleTags),
);
const hiddenCount = computed(() =>
  Math.max(0, props.item.extra.tags.length - maxVisibleTags),
);
</script>

<template>
  <ResultCardShell
    :platform="item.platform"
    :thumb="item.thumb"
    :title="item.title"
  >
    <p v-if="visibleTags.length === 0" class="meta-line">
      {{ t("home.search.noTags") }}
    </p>

    <div v-else class="tag-row">
      <NTag
        v-for="tag in visibleTags"
        :key="tag"
        size="small"
        :bordered="false"
        type="info"
      >
        {{ tag }}
      </NTag>
      <NTag v-if="hiddenCount > 0" size="small" :bordered="false">
        {{ t("home.search.moreTags", { count: hiddenCount }) }}
      </NTag>
    </div>
  </ResultCardShell>
</template>

<style scoped>
.meta-line {
  margin: 0;
  overflow: hidden;
  font-size: 0.8rem;
  line-height: 1.4;
  opacity: 0.6;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.tag-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem;
}
</style>
