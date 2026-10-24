<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { fetch as httpFetch } from "@tauri-apps/plugin-http";
import { useMessage } from "naive-ui";
import { useErrorMessage } from "../composables/useErrorMessage";
import { toAppError } from "../errors";
import { useAppI18n } from "../i18n";
import type { LocalePreference } from "../i18n";
import { useSettingsStore } from "../stores/settings";
import type { ThemeMode } from "../stores/settings";

const { t } = useAppI18n();
const message = useMessage();
const errorText = useErrorMessage();
const settings = useSettingsStore();

const greetName = ref("");
const name = ref("");

const imageUrl = ref("");
const imageLoading = ref(false);
const imageSrc = ref("");
let imageObjectUrl = "";

type ImageErrorDetail =
  | { code: "http"; status: number }
  | { code: "empty" }
  | { code: "notImage"; contentType: string }
  | { code: "display" }
  | { code: "unknown"; message: string };

class ImageLoadError extends Error {
  constructor(readonly detail: ImageErrorDetail) {
    super(detail.code);
  }
}

// Store the failure reason instead of a rendered string so the alert follows
// language changes.
const imageError = ref<ImageErrorDetail | null>(null);

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
  const detail = imageError.value;
  if (!detail) {
    return "";
  }

  switch (detail.code) {
    case "http":
      return t("home.image.errors.http", { status: detail.status });
    case "empty":
      return t("home.image.errors.empty");
    case "notImage":
      return t("home.image.errors.notImage", { type: detail.contentType });
    case "display":
      return t("home.image.errors.display");
    case "unknown":
      return t("home.image.errors.unknown", { message: detail.message });
  }
});

async function greet() {
  try {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    greetName.value = await invoke<string>("greet", { name: name.value });
  } catch (error) {
    message.error(errorText(error));
  }
}

function revokeImage() {
  if (imageObjectUrl) {
    URL.revokeObjectURL(imageObjectUrl);
    imageObjectUrl = "";
  }
}

async function showImage(blob: Blob) {
  const previous = imageObjectUrl;
  imageObjectUrl = URL.createObjectURL(blob);
  imageSrc.value = imageObjectUrl;

  // Let Vue swap the <img> source before releasing the previous blob URL,
  // otherwise the browser can report a load error for the old image.
  await nextTick();
  if (previous) {
    URL.revokeObjectURL(previous);
  }
}

async function fetchImage(url: string) {
  // Use the Tauri HTTP plugin: it is not subject to CORS and its allowed
  // origins are configured in the capability file.
  return httpFetch(url, {
    headers: {
      Referer: "https://www.manhuagui.com/",
      "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36"
    }
  });
}

async function loadImage() {
  const url = imageUrl.value.trim();
  if (!url || imageLoading.value) {
    return;
  }

  imageLoading.value = true;
  imageError.value = null;
  try {
    const response = await fetchImage(url);
    if (!response.ok) {
      throw new ImageLoadError({ code: "http", status: response.status });
    }

    const blob = await response.blob();
    if (blob.size === 0) {
      throw new ImageLoadError({ code: "empty" });
    }
    if (blob.type && !blob.type.startsWith("image/")) {
      throw new ImageLoadError({ code: "notImage", contentType: blob.type });
    }

    await showImage(blob);
  } catch (error) {
    imageError.value =
      error instanceof ImageLoadError
        ? error.detail
        : { code: "unknown", message: toAppError(error).message };
  } finally {
    imageLoading.value = false;
  }
}

function onImageError() {
  imageError.value = { code: "display" };
  revokeImage();
  imageSrc.value = "";
}

onBeforeUnmount(revokeImage);
</script>

<template>
  <main class="container">
    <form class="greet" @submit.prevent="greet">
      <NInput v-model:value="name" class="greet-input" :placeholder="t('home.greet.placeholder')" />
      <NButton attr-type="submit" type="primary">
        {{ t("home.greet.submit") }}
      </NButton>
    </form>
    <p>{{ greetName ? t("home.greet.result", { name: greetName }) : "" }}</p>

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
