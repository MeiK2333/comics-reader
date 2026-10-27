<script setup lang="ts">
import type { ComicDetail } from "./index";
import JmDetail from "./jm/Detail.vue";
import ManhuaguiDetail from "./manhuagui/Detail.vue";
import NhentaiDetail from "./nhentai/Detail.vue";

defineProps<{
  detail: ComicDetail;
  resumeChapterId?: string | null;
}>();

const emit = defineEmits<{
  openChapter: [chapterId: string];
}>();
</script>

<template>
  <ManhuaguiDetail
    v-if="detail.platform === 'manhuagui'"
    :detail="detail"
    :resume-chapter-id="resumeChapterId"
    @open-chapter="emit('openChapter', $event)"
  />
  <JmDetail
    v-else-if="detail.platform === 'jm'"
    :detail="detail"
    @open-chapter="emit('openChapter', $event)"
  />
  <NhentaiDetail v-else :detail="detail" />
</template>
