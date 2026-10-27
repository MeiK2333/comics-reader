<script setup lang="ts">
import { useBackNavigation } from "../composables/useBackNavigation";
import { useAppI18n } from "../i18n";

const { goBack } = useBackNavigation();
const { t } = useAppI18n();

function goHome() {
  goBack({ name: "home" });
}
</script>

<template>
  <main class="not-found">
    <h1>{{ t("notFound.title") }}</h1>
    <p>{{ t("notFound.description") }}</p>
    <NButton text type="primary" class="not-found__link" @click="goHome">
      {{ t("notFound.back") }}
    </NButton>
  </main>
</template>

<style scoped>
.not-found__link {
  font-size: inherit;
}
</style>
