<script setup lang="ts">
defineProps<{
  name: "back" | "refresh" | "clear" | "export";
}>();
</script>

<template>
  <svg
    class="log-icon"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    focusable="false"
  >
    <template v-if="name === 'refresh'">
      <path d="M20 11a8 8 0 1 0-2.3 6.5" />
      <path d="M20 5v6h-6" />
    </template>
    <template v-else-if="name === 'clear'">
      <path d="M4 7h16" />
      <path d="M9 7V4h6v3" />
      <path d="m6 7 1 13h10l1-13" />
      <path d="M10 11v5M14 11v5" />
    </template>
    <template v-else-if="name === 'export'">
      <path d="M12 15V3" />
      <path d="m7 8 5-5 5 5" />
      <path d="M5 14v6h14v-6" />
    </template>
    <template v-else>
      <path d="M15 5l-7 7 7 7" />
    </template>
  </svg>
</template>

<style scoped>
.log-icon {
  display: block;
  width: 20px;
  height: 20px;
}
</style>
