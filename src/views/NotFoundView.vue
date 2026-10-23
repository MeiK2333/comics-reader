<script setup lang="ts">
import { RouterLink } from "vue-router";
</script>

<template>
  <main class="not-found">
    <h1>Page not found</h1>
    <p>The page you are looking for does not exist.</p>
    <RouterLink to="/">Back to home</RouterLink>
  </main>
</template>

<style scoped>
.not-found {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  padding: 20vh 24px;
  text-align: center;
}

.not-found h1 {
  margin: 0;
}

.not-found p {
  margin: 0;
  opacity: 0.7;
}
</style>
