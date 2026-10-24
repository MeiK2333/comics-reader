<script setup lang="ts">
import { RouterLink } from "vue-router";
import { useAppI18n } from "../i18n";

const { t } = useAppI18n();
</script>

<template>
  <main class="not-found">
    <h1>{{ t("notFound.title") }}</h1>
    <p>{{ t("notFound.description") }}</p>
    <RouterLink to="/">{{ t("notFound.back") }}</RouterLink>
  </main>
</template>

<style scoped></style>
