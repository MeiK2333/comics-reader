<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import ShortcutIcon from "../components/ShortcutIcon.vue";
import { useAppI18n } from "../i18n";
import { SHORTCUT_LABEL_KEYS, type ShortcutFeature } from "../shortcuts";

const props = defineProps<{
  feature: ShortcutFeature;
}>();

const router = useRouter();
const { t } = useAppI18n();

const title = computed(() => t(SHORTCUT_LABEL_KEYS[props.feature]));

function goHome() {
  void router.push({ name: "home" });
}
</script>

<template>
  <main class="shortcut-page">
    <span class="shortcut-page-icon">
      <ShortcutIcon :name="feature" />
    </span>
    <h1>{{ title }}</h1>
    <p>{{ t("shortcut.placeholder.description") }}</p>
    <NButton type="primary" quaternary @click="goHome">
      {{ t("shortcut.placeholder.back") }}
    </NButton>
  </main>
</template>

<style scoped>
.shortcut-page {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.6rem;
  min-height: 70vh;
  padding: 2rem 1.25rem 4rem;
  text-align: center;
}

.shortcut-page-icon {
  display: grid;
  place-items: center;
  width: 72px;
  height: 72px;
  margin-bottom: 0.25rem;
  border-radius: 50%;
  background-color: rgba(127, 127, 127, 0.08);
}

.shortcut-page-icon :deep(.shortcut-icon-svg) {
  width: 30px;
  height: 30px;
}

h1 {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 600;
}

p {
  margin: 0 0 0.75rem;
  opacity: 0.65;
}

@media (max-width: 560px) {
  .shortcut-page {
    min-height: 60vh;
    padding-top: 1rem;
  }
}
</style>
