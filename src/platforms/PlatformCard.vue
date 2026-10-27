<script setup lang="ts">
import type { ComicSearchItem } from "./index";
import CopymangaResultCard from "./copymanga/ResultCard.vue";
import JmResultCard from "./jm/ResultCard.vue";
import ManhuaguiResultCard from "./manhuagui/ResultCard.vue";
import NhentaiResultCard from "./nhentai/ResultCard.vue";

defineProps<{
  item: ComicSearchItem;
}>();
</script>

<template>
  <ManhuaguiResultCard v-if="item.platform === 'manhuagui'" :item="item" />
  <JmResultCard v-else-if="item.platform === 'jm'" :item="item" />
  <CopymangaResultCard
    v-else-if="item.platform === 'copymanga'"
    :item="item"
  />
  <NhentaiResultCard v-else :item="item" />
</template>