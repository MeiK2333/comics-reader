<script setup lang="ts">
import { computed } from "vue";
import { useRouter } from "vue-router";
import type { SelectOption } from "naive-ui";
import HomeIcon from "../components/HomeIcon.vue";
import { useAppI18n } from "../i18n";
import { useSettingsStore } from "../stores/settings";

const router = useRouter();
const { t } = useAppI18n();
const settings = useSettingsStore();

const themeOptions = computed<SelectOption[]>(() => [
  { label: t("settings.theme.options.system"), value: "system" },
  { label: t("settings.theme.options.light"), value: "light" },
  { label: t("settings.theme.options.dark"), value: "dark" },
]);

const localeOptions = computed<SelectOption[]>(() => [
  { label: t("settings.language.options.system"), value: "system" },
  { label: t("settings.language.options.en"), value: "en" },
  { label: t("settings.language.options.zh-CN"), value: "zh-CN" },
]);

function goHome() {
  void router.push({ name: "home" });
}
</script>

<template>
  <main class="settings-page">
    <header class="settings-header">
      <NButton
        attr-type="button"
        quaternary
        circle
        size="large"
        :aria-label="t('settings.back')"
        @click="goHome"
      >
        <template #icon>
          <HomeIcon />
        </template>
      </NButton>
      <h1>{{ t("settings.title") }}</h1>
    </header>

    <NCard class="settings-card" size="small">
      <div class="setting">
        <span>{{ t("settings.theme.label") }}</span>
        <NSelect
          v-model:value="settings.theme"
          class="setting-select"
          :options="themeOptions"
        />
      </div>
      <div class="setting">
        <span>{{ t("settings.language.label") }}</span>
        <NSelect
          v-model:value="settings.locale"
          class="setting-select"
          :options="localeOptions"
        />
      </div>
    </NCard>
  </main>
</template>

<style scoped>
.settings-page {
  width: min(560px, calc(100% - 2.5rem));
  margin: 0 auto;
  padding: 1.5rem 1.25rem 4rem;
}

.settings-header {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  margin-bottom: 1rem;
}

.settings-header h1 {
  margin: 0;
  font-size: 1.25rem;
  font-weight: 600;
}

.settings-card {
  margin: 0 auto;
  text-align: left;
}

.setting {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

.setting + .setting {
  margin-top: 0.75rem;
}

.setting-select {
  width: 9.5rem;
  flex: none;
}
</style>
