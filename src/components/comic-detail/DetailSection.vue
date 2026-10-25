<script setup lang="ts">
defineProps<{
  title: string;
}>();
</script>

<template>
  <section class="detail-section">
    <h2>{{ title }}</h2>
    <slot />
  </section>
</template>

<style scoped>
.detail-section {
  margin-top: 1.5rem;
}

.detail-section:first-child {
  margin-top: 0;
}

.detail-section h2 {
  margin: 0 0 0.6rem;
  font-size: 1rem;
  font-weight: 600;
}
</style>
