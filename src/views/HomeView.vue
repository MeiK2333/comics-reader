<script setup lang="ts">
import { computed, ref } from "vue";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { useErrorMessage } from "../composables/useErrorMessage";
import { toAppError } from "../errors";
import type { AppError } from "../errors";
import { useAppI18n } from "../i18n";
import type { LocalePreference } from "../i18n";
import { useSettingsStore } from "../stores/settings";
import type { ThemeMode } from "../stores/settings";

const { t } = useAppI18n();
const errorText = useErrorMessage();
const settings = useSettingsStore();

const imageUrl = ref("");
const imageLoading = ref(false);
const imageSrc = ref("");

interface CachedImage {
  path: string;
  contentType: string;
  byteLength: number;
  fromCache: boolean;
}

// Keep the structured error so the alert follows language changes; the
// display failure has no Rust code because it happens inside the webview.
const imageError = ref<AppError | null>(null);
const imageDisplayError = ref(false);

const themeOptions = computed<{ label: string; value: ThemeMode }[]>(() => [
  { label: t("home.settings.theme.options.system"), value: "system" },
  { label: t("home.settings.theme.options.light"), value: "light" },
  { label: t("home.settings.theme.options.dark"), value: "dark" },
]);

const localeOptions = computed<{ label: string; value: LocalePreference }[]>(
  () => [
    { label: t("home.settings.language.options.system"), value: "system" },
    { label: t("home.settings.language.options.en"), value: "en" },
    { label: t("home.settings.language.options.zh-CN"), value: "zh-CN" },
  ],
);

const imageErrorMessage = computed(() => {
  if (imageDisplayError.value) {
    return t("home.image.errors.display");
  }
  return imageError.value ? errorText(imageError.value) : "";
});

async function loadImage() {
  const url = imageUrl.value.trim();
  if (!url || imageLoading.value) {
    return;
  }

  imageLoading.value = true;
  imageError.value = null;
  imageDisplayError.value = false;
  imageSrc.value = "";
  try {
    // Rust downloads and caches the image; the asset protocol then serves the
    // cached file, so the webview never requests the remote host directly.
    const cached = await invoke<CachedImage>("fetch_image", { url });
    imageSrc.value = convertFileSrc(cached.path);
  } catch (error) {
    imageError.value = toAppError(error);
  } finally {
    imageLoading.value = false;
  }
}

function onImageError() {
  imageDisplayError.value = true;
  imageSrc.value = "";
}
</script>

<template>
  <main class="container">
    <NCard class="image-tester" :title="t('home.image.title')" size="small">
      <form class="image-form" @submit.prevent="loadImage">
        <NInput v-model:value="imageUrl" class="image-url" :placeholder="t('home.image.placeholder')" />
        <NButton attr-type="submit" type="primary" :loading="imageLoading">
          {{ t("home.image.load") }}
        </NButton>
      </form>
      <p class="hint">
        {{ t("home.image.hint") }}
      </p>
      <NAlert v-if="imageErrorMessage" class="image-error" type="error" :bordered="false">
        {{ imageErrorMessage }}
      </NAlert>
      <div v-if="imageSrc" class="image-preview">
        <img :src="imageSrc" :alt="t('home.image.alt')" @error="onImageError" />
      </div>
    </NCard>

    <NCard class="settings" :title="t('home.settings.title')" size="small">
      <div class="setting">
        <span>{{ t("home.settings.theme.label") }}</span>
        <NSelect v-model:value="settings.theme" class="theme-select" :options="themeOptions" />
      </div>
      <div class="setting">
        <span>{{ t("home.settings.language.label") }}</span>
        <NSelect v-model:value="settings.locale" class="theme-select" :options="localeOptions" />
      </div>
      <p class="hint">
        {{ t("home.settings.hint") }}
      </p>
    </NCard>
  </main>
</template>

<style scoped></style>
