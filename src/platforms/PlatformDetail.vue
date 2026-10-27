<script setup lang="ts">
import type { ComicDetail } from "./index";
import JmDetail from "./jm/Detail.vue";
import ManhuaguiDetail from "./manhuagui/Detail.vue";
import NhentaiDetail from "./nhentai/Detail.vue";

defineProps<{
  detail: ComicDetail;
  resumeChapterId?: string | null;
  /** Hide the online chapter list on the downloaded detail. */
  hideChapters?: boolean;
  /** Local cover file used by the downloaded-detail route. */
  localCoverPath?: string;
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
    :hide-chapters="hideChapters"
    :local-cover-path="localCoverPath"
    @open-chapter="emit('openChapter', $event)"
  >
    <template #summary-extra>
      <slot name="summary-extra" />
    </template>
    <template #extra>
      <slot name="extra" />
    </template>
  </ManhuaguiDetail>
  <JmDetail
    v-else-if="detail.platform === 'jm'"
    :detail="detail"
    :hide-chapters="hideChapters"
    :local-cover-path="localCoverPath"
    @open-chapter="emit('openChapter', $event)"
  >
    <template #summary-extra>
      <slot name="summary-extra" />
    </template>
    <template #extra>
      <slot name="extra" />
    </template>
  </JmDetail>
  <NhentaiDetail
    v-else
    :detail="detail"
    :local-cover-path="localCoverPath"
  >
    <template #summary-extra>
      <slot name="summary-extra" />
    </template>
    <template #extra>
      <slot name="extra" />
    </template>
  </NhentaiDetail>
</template>