<script setup lang="ts">
defineProps<{
  filled?: boolean;
}>();
</script>

<template>
  <svg
    class="favorite-icon"
    viewBox="0 0 24 24"
    :fill="filled ? 'currentColor' : 'none'"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    focusable="false"
  >
    <path d="M6.5 4h11a1 1 0 0 1 1 1v15.5l-6.5-4-6.5 4V5a1 1 0 0 1 1-1Z" />
  </svg>
</template>

<style scoped>
.favorite-icon {
  display: block;
  width: 18px;
  height: 18px;
}
</style>
