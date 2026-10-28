<script setup lang="ts">
import { computed } from "vue";
import type { EhentaiSearchItem } from "./types";

import { usePlatformText } from "../../platforms/messages";
import ResultCardShell from "../../components/result-cards/ResultCardShell.vue";
import { ehentaiMessages } from "./messages";

const props = defineProps<{
  item: EhentaiSearchItem;
}>();

const text = usePlatformText(ehentaiMessages);

const extra = computed(() => props.item.extra);

const languages = computed(() => extra.value.languages.slice(0, 2).join(", "));

const statsLine = computed(() =>
  [
    extra.value.category,
    extra.value.pages === null
      ? ""
      : text("searchPages", { count: extra.value.pages }),
    languages.value,
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
