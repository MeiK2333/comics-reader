<script setup lang="ts">
import type { ShortcutFeature } from "../shortcuts";

defineProps<{
  name: ShortcutFeature;
}>();
</script>

<template>
  <svg
    class="shortcut-icon-svg"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="2"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    focusable="false"
  >
    <template v-if="name === 'favorites'">
      <path d="M6.5 4h11a1 1 0 0 1 1 1v15.5l-6.5-4-6.5 4V5a1 1 0 0 1 1-1Z" />
    </template>
    <template v-else-if="name === 'downloads'">
      <path d="M12 3v11" />
      <path d="m7.5 10 4.5 4 4.5-4" />
      <path d="M5 20h14" />
    </template>
    <template v-else-if="name === 'history'">
      <path d="M3 12a9 9 0 1 0 3-6.7L3 8" />
      <path d="M3 3v5h5" />
      <path d="M12 7v5l4 2" />
    </template>
    <template v-else>
      <path
        d="M10.3 4.3c.4-1.8 2.9-1.8 3.3 0a1.7 1.7 0 0 0 2.6 1.1c1.5-.9 3.3.8 2.4 2.4a1.7 1.7 0 0 0 1 2.6c1.8.4 1.8 2.9 0 3.3a1.7 1.7 0 0 0-1 2.6c.9 1.5-.8 3.3-2.4 2.4a1.7 1.7 0 0 0-2.6 1c-.4 1.8-2.9 1.8-3.3 0a1.7 1.7 0 0 0-2.6-1c-1.5.9-3.3-.8-2.4-2.4a1.7 1.7 0 0 0-1-2.6c-1.8-.4-1.8-2.9 0-3.3a1.7 1.7 0 0 0 1-2.6c-.9-1.5.8-3.3 2.4-2.4.9.6 2.2.1 2.6-1Z"
      />
      <circle cx="12" cy="12" r="3" />
    </template>
  </svg>
</template>

<style scoped>
.shortcut-icon-svg {
  display: block;
  width: 24px;
  height: 24px;
}
</style>
