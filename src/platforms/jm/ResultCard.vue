<script setup lang="ts">
import { computed } from "vue";
import type { JmSearchItem } from "./types";

import { usePlatformText } from "../../platforms/messages";
import ResultCardShell from "../../components/result-cards/ResultCardShell.vue";
import { jmMessages } from "./messages";

const props = defineProps<{
  item: JmSearchItem;
}>();

const text = usePlatformText(jmMessages);

const maxVisibleTags = 6;
const visibleTags = computed(() =>
  props.item.extra.tags.slice(0, maxVisibleTags),
);
const hiddenCount = computed(() =>
  Math.max(0, props.item.extra.tags.length - maxVisibleTags),
);

// The APP search payload has no tags, so author and category are the only
// metadata a result card can show.
const metaLine = computed(() =>
  [props.item.extra.author, props.item.extra.category]
    .filter((value): value is string => Boolean(value))
    .join(" · "),
);
</script>

<template>
  <ResultCardShell
    :platform="item.platform"
    :thumb="item.thumb"
    :title="item.title"
  >
    <p v-if="metaLine" class="meta-line">
      {{ metaLine }}
    </p>

    <p v-else-if="visibleTags.length === 0" class="meta-line">
      {{ text("noTags") }}
    </p>

    <div v-if="visibleTags.length > 0" class="tag-row">
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
        {{ text("moreTags", { count: hiddenCount }) }}
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
