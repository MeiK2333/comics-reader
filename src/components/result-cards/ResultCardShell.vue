<script setup lang="ts">
import type { ComicPlatform } from "../../api/comics";
import ComicThumb from "../ComicThumb.vue";

defineProps<{
  platform: ComicPlatform;
  thumb: string;
  title: string;
}>();
</script>

<template>
  <NCard class="result-card" size="small" hoverable>
    <template #cover>
      <ComicThumb :platform="platform" :thumb="thumb" :title="title" />
    </template>
    <h2 class="result-title" :title="title">
      {{ title }}
    </h2>
    <div class="result-body">
      <slot />
    </div>
  </NCard>
</template>

<style scoped>
.result-card {
  overflow: hidden;
}

.result-title {
  display: -webkit-box;
  overflow: hidden;
  margin: 0.6rem 0 0.4rem;
  font-size: 0.95rem;
  font-weight: 600;
  line-height: 1.35;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.result-body {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}
</style>
