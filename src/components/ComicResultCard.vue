<script setup lang="ts">
import { computed } from "vue";
import type { ComicPlatform, ComicSearchItem } from "../api/comics";
import { useAppI18n } from "../i18n";
import ComicThumb from "./ComicThumb.vue";

const props = defineProps<{
  platform: ComicPlatform;
  item: ComicSearchItem;
}>();

const { t } = useAppI18n();

function stringItems(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((entry): entry is string => typeof entry === "string")
    : [];
}

/**
 * `extra` is platform-specific wire data. Each platform contributes its own
 * metadata lines, so an unknown or malformed field simply stays hidden.
 */
const meta = computed(() => {
  const extra = props.item.extra;
  const lines: string[] = [];

  if (props.platform === "manhuagui") {
    const authors = stringItems(extra.authors);
    if (authors.length > 0) {
      lines.push(authors.join(", "));
    }
    if (typeof extra.latestChapter === "string" && extra.latestChapter) {
      lines.push(extra.latestChapter);
    }
    return lines;
  }

  if (props.platform === "jm") {
    const tags = stringItems(extra.tags);
    if (tags.length > 0) {
      lines.push(tags.slice(0, 4).join(" · "));
    }
    return lines;
  }

  if (typeof extra.numPages === "number") {
    lines.push(t("home.search.pages", { count: extra.numPages }));
  }
  if (typeof extra.numFavorites === "number") {
    lines.push(t("home.search.favorites", { count: extra.numFavorites }));
  }
  return lines;
});
</script>

<template>
  <NCard class="result-card" size="small" hoverable>
    <template #cover>
      <ComicThumb
        :platform="platform"
        :thumb="item.thumb"
        :title="item.title"
      />
    </template>
    <h2 class="result-title" :title="item.title">
      {{ item.title }}
    </h2>
    <p v-for="(line, index) in meta" :key="index" class="result-meta">
      {{ line }}
    </p>
  </NCard>
</template>

<style scoped>
.result-card {
  overflow: hidden;
}

.result-title {
  display: -webkit-box;
  overflow: hidden;
  margin: 0.6rem 0 0.35rem;
  font-size: 0.95rem;
  font-weight: 600;
  line-height: 1.35;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.result-meta {
  margin: 0 0 0.2rem;
  overflow: hidden;
  font-size: 0.8rem;
  opacity: 0.65;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
