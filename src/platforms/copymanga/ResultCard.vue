<script setup lang="ts">
import { computed } from "vue";
import type { CopymangaSearchItem } from "./types";

import { usePlatformText } from "../../platforms/messages";
import ResultCardShell from "../../components/result-cards/ResultCardShell.vue";
import { copymangaMessages } from "./messages";

const props = defineProps<{
  item: CopymangaSearchItem;
}>();

const text = usePlatformText(copymangaMessages);

const extra = computed(() => props.item.extra);

const authors = computed(() => extra.value.authors.join(", "));

const aliases = computed(() =>
  extra.value.aliases
    .filter((alias) => alias !== props.item.title)
    .slice(0, 2)
    .join(", "),
);

const popularity = computed(() => {
  const value = extra.value.popularity;
  return value === null ? "" : value.toLocaleString();
});

const status = computed(() => {
  const raw = extra.value.status;
  if (raw === "連載中" || raw === "连载中") {
    return text("statusOngoing");
  }
  if (raw === "已完結" || raw === "已完结") {
    return text("statusCompleted");
  }
  return raw;
});

const statusType = computed<"success" | "default">(() =>
  extra.value.status === "連載中" || extra.value.status === "连载中"
    ? "success"
    : "default",
);

const categories = computed(() => extra.value.categories.slice(0, 3));
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

    <div v-if="status || popularity" class="summary-row">
      <NTag v-if="status" size="small" :bordered="false" :type="statusType">
        {{ status }}
      </NTag>
      <span v-if="popularity" class="popularity">
        {{ text("popularity", { count: popularity }) }}
      </span>
    </div>

    <p v-if="aliases" class="meta-line" :title="aliases">
      {{ text("aliases", { names: aliases }) }}
    </p>

    <div v-if="categories.length > 0" class="tag-row">
      <NTag
        v-for="category in categories"
        :key="category"
        size="small"
        :bordered="false"
        type="info"
      >
        {{ category }}
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

.popularity {
  font-size: 0.8rem;
  font-weight: 600;
}

.tag-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem;
}
</style>
