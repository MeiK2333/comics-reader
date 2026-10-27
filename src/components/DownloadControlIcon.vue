<script setup lang="ts">
defineProps<{
  name: "pause" | "resume";
}>();
</script>

<template>
  <svg
    class="download-control-icon"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    focusable="false"
  >
    <template v-if="name === 'pause'">
      <path d="M9 5v14" />
      <path d="M15 5v14" />
    </template>
    <template v-else>
      <path d="M8 5.5 19 12 8 18.5Z" />
    </template>
  </svg>
</template>

<style scoped>
.download-control-icon {
  display: block;
  width: 18px;
  height: 18px;
}
</style>
