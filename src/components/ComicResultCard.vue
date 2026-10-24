<script setup lang="ts">
import { computed } from "vue";
import type {
  ComicSearchItem,
  JmSearchItem,
  ManhuaguiSearchItem,
  NhentaiSearchItem,
} from "../api/comics";
import JmResultCard from "./result-cards/JmResultCard.vue";
import ManhuaguiResultCard from "./result-cards/ManhuaguiResultCard.vue";
import NhentaiResultCard from "./result-cards/NhentaiResultCard.vue";

const props = defineProps<{
  item: ComicSearchItem;
}>();

const manhuaguiItem = computed<ManhuaguiSearchItem | null>(() =>
  props.item.platform === "manhuagui" ? props.item : null,
);

const jmItem = computed<JmSearchItem | null>(() =>
  props.item.platform === "jm" ? props.item : null,
);

const nhentaiItem = computed<NhentaiSearchItem | null>(() =>
  props.item.platform === "nhentai" ? props.item : null,
);
</script>

<template>
  <ManhuaguiResultCard v-if="manhuaguiItem" :item="manhuaguiItem" />
  <JmResultCard v-else-if="jmItem" :item="jmItem" />
  <NhentaiResultCard v-else-if="nhentaiItem" :item="nhentaiItem" />
</template>
