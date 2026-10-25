<script setup lang="ts">
import { computed } from "vue";
import DetailSection from "./DetailSection.vue";

const props = defineProps<{
  title: string;
  tags: string[];
}>();

const visibleTags = computed(() => [
  ...new Set(
    props.tags
      .map((tag) => tag.trim())
      .filter((tag) => tag.length > 0),
  ),
]);
</script>

<template>
  <DetailSection v-if="visibleTags.length > 0" :title="title">
    <div class="tag-row">
      <NTag
        v-for="tag in visibleTags"
        :key="tag"
        size="small"
        :bordered="false"
        type="info"
      >
        {{ tag }}
      </NTag>
    </div>
  </DetailSection>
</template>

<style scoped>
.tag-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
}
</style>
