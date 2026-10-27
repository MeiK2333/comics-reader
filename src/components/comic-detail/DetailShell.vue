<script setup lang="ts">
import { computed } from "vue";
import type { ComicPlatform } from "../../platforms";

import { platformLabel } from "../../platforms";
import ComicThumb from "../ComicThumb.vue";

const props = defineProps<{
  platform: ComicPlatform;
  title: string;
  thumb: string;
  subtitle?: string;
  /** Local cover path, used by the downloaded-detail route. */
  localPath?: string;
}>();



const label = computed(() => platformLabel(props.platform));
</script>

<template>
  <article class="detail-shell">
    <div class="detail-hero">
      <div class="detail-cover">
        <ComicThumb
          :platform="platform"
          :thumb="thumb"
          :title="title"
          :local-path="localPath"
        />
      </div>

      <div class="detail-summary">
        <NTag
          class="detail-platform"
          size="small"
          :bordered="false"
          type="info"
        >
          {{ label }}
        </NTag>
        <h1 class="detail-title">{{ title }}</h1>
        <p v-if="subtitle" class="detail-subtitle">{{ subtitle }}</p>
        <slot name="summary" />
      </div>
    </div>

    <div class="detail-body">
      <slot />
    </div>
  </article>
</template>

<style scoped>
.detail-hero {
  display: grid;
  grid-template-columns: minmax(180px, 240px) minmax(0, 1fr);
  gap: 1.5rem;
  align-items: start;
}

.detail-cover {
  overflow: hidden;
  border-radius: 12px;
}

.detail-summary {
  min-width: 0;
}

.detail-platform {
  margin-bottom: 0.5rem;
}

.detail-title {
  margin: 0;
  font-size: 1.6rem;
  line-height: 1.3;
  overflow-wrap: anywhere;
}

.detail-subtitle {
  margin: 0.4rem 0 0;
  line-height: 1.5;
  opacity: 0.65;
  overflow-wrap: anywhere;
}

.detail-body {
  margin-top: 1.75rem;
}

@media (max-width: 640px) {
  .detail-hero {
    grid-template-columns: 1fr;
    gap: 1rem;
  }

  .detail-cover {
    width: min(220px, 60%);
    margin: 0 auto;
  }
}
</style>
