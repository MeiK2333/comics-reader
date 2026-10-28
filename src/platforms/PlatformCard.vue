<script setup lang="ts">
import type { ComicSearchItem } from "./index";
import CopymangaResultCard from "./copymanga/ResultCard.vue";
import EhentaiResultCard from "./ehentai/ResultCard.vue";
import JmResultCard from "./jm/ResultCard.vue";
import ManhuaguiResultCard from "./manhuagui/ResultCard.vue";
import ManwaResultCard from "./manwa/ResultCard.vue";
import NhentaiResultCard from "./nhentai/ResultCard.vue";
import PixivResultCard from "./pixiv/ResultCard.vue";

defineProps<{
  item: ComicSearchItem;
}>();
</script>

<template>
  <ManhuaguiResultCard v-if="item.platform === 'manhuagui'" :item="item" />
  <ManwaResultCard v-else-if="item.platform === 'manwa'" :item="item" />
  <JmResultCard v-else-if="item.platform === 'jm'" :item="item" />
  <CopymangaResultCard
    v-else-if="item.platform === 'copymanga'"
    :item="item"
  />
  <EhentaiResultCard v-else-if="item.platform === 'ehentai'" :item="item" />
  <NhentaiResultCard v-else-if="item.platform === 'nhentai'" :item="item" />
  <PixivResultCard v-else :item="item" />
</template>
