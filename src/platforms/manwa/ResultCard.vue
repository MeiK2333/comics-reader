<script setup lang="ts">
import { computed } from "vue";
import type { ManwaSearchItem } from "./types";

import { usePlatformText } from "../../platforms/messages";
import ResultCardShell from "../../components/result-cards/ResultCardShell.vue";
import { manwaMessages } from "./messages";

const props = defineProps<{
  item: ManwaSearchItem;
}>();

const text = usePlatformText(manwaMessages);

const extra = computed(() => props.item.extra);

const authors = computed(() => extra.value.authors.join(", "));

const status = computed(() => {
  if (extra.value.status === "连载中") {
    return text("statusOngoing");
  }
  if (extra.value.status === "已完结") {
    return text("statusCompleted");
  }
  return extra.value.status ?? "";
});

const statusType = computed<"success" | "default">(() =>
  extra.value.status === "连载中" ? "success" : "default",
);
</script>

<template>
  <ResultCardShell
    :platform="item.platform"
    :thumb="item.thumb"
    :title="item.title"
  >
    <p v-if="authors" class="meta-line" :title="authors">
      {{ text("authors", { names: authors }) }}
    </p>

    <div v-if="status" class="summary-row">
      <NTag size="small" :bordered="false" :type="statusType">
        {{ status }}
      </NTag>
    </div>

    <p v-if="extra.description" class="meta-line" :title="extra.description">
      {{ extra.description }}
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

.summary-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.4rem;
}
</style>
