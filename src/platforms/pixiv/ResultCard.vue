<script setup lang="ts">
import { computed } from "vue";
import type { PixivSearchItem } from "./types";

import { usePlatformText } from "../../platforms/messages";
import ResultCardShell from "../../components/result-cards/ResultCardShell.vue";
import { pixivMessages } from "./messages";

const props = defineProps<{
  item: PixivSearchItem;
}>();

const text = usePlatformText(pixivMessages);

const extra = computed(() => props.item.extra);

const author = computed(() => extra.value.userName.trim());

const typeKey = computed<"typeIllust" | "typeManga" | "typeUgoira">(() => {
  if (extra.value.illustType === 1) {
    return "typeManga";
  }
  if (extra.value.illustType === 2) {
    return "typeUgoira";
  }
  return "typeIllust";
});

const statsLine = computed(() =>
  [
    text(typeKey.value),
    extra.value.pageCount > 0
      ? text("searchPages", { count: extra.value.pageCount })
      : "",
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
    <p v-if="author" class="meta-line" :title="author">
      {{ text("searchBy", { name: author }) }}
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
