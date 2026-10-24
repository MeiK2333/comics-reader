<script setup lang="ts">
import { computed } from "vue";
import type { ManhuaguiSearchItem } from "../../api/comics";
import { useAppI18n } from "../../i18n";
import ResultCardShell from "./ResultCardShell.vue";

const props = defineProps<{
  item: ManhuaguiSearchItem;
}>();

const { t } = useAppI18n();

const extra = computed(() => props.item.extra);

const authors = computed(() => extra.value.authors.join(", "));

const status = computed(() => {
  const raw = extra.value.status;
  if (raw === "连载中") {
    return t("home.search.statusOngoing");
  }
  if (raw === "已完结") {
    return t("home.search.statusCompleted");
  }
  return raw;
});

const statusType = computed<"success" | "default">(() =>
  extra.value.status === "连载中" ? "success" : "default",
);

const scoreLine = computed(() => {
  const score = extra.value.score;
  if (score === null) {
    return "";
  }

  const formatted = t("home.search.score", { score: score.toFixed(1) });
  const count = extra.value.ratingCount;
  return count === null
    ? formatted
    : `${formatted} · ${t("home.search.ratings", { count })}`;
});

const chapterLine = computed(() => {
  const { latestChapter, updatedAt } = extra.value;
  const latest = latestChapter
    ? t("home.search.latestChapter", { chapter: latestChapter })
    : "";
  const updated = updatedAt
    ? t("home.search.updatedAt", { date: updatedAt })
    : "";
  return [latest, updated].filter(Boolean).join(" · ");
});

const locationLine = computed(() =>
  [extra.value.year, extra.value.region]
    .filter((value): value is string => Boolean(value))
    .join(" · "),
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
      {{ t("home.search.authors", { names: authors }) }}
    </p>

    <div v-if="status || scoreLine" class="summary-row">
      <NTag v-if="status" size="small" :bordered="false" :type="statusType">
        {{ status }}
      </NTag>
      <span v-if="scoreLine" class="score">{{ scoreLine }}</span>
    </div>

    <p v-if="chapterLine" class="meta-line" :title="chapterLine">
      {{ chapterLine }}
    </p>

    <p v-if="locationLine" class="meta-line" :title="locationLine">
      {{ locationLine }}
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

.score {
  font-size: 0.8rem;
  font-weight: 600;
}

.tag-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem;
}
</style>
