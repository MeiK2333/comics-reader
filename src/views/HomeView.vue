<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref } from "vue";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { fetch as httpFetch } from "@tauri-apps/plugin-http";
import { useMessage } from "naive-ui";
import { useSettingsStore } from "../stores/settings";
import type { ThemeMode } from "../stores/settings";

const message = useMessage();
const settings = useSettingsStore();

const greetMsg = ref("");
const name = ref("");

const imageUrl = ref("");
const imageLoading = ref(false);
const imageError = ref("");
const imageSrc = ref("");
let imageObjectUrl = "";

const themeOptions: { label: string; value: ThemeMode }[] = [
  { label: "System", value: "system" },
  { label: "Light", value: "light" },
  { label: "Dark", value: "dark" },
];

async function greet() {
  try {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    greetMsg.value = await invoke<string>("greet", { name: name.value });
  } catch (error) {
    message.error(`Greet failed: ${String(error)}`);
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

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

async function fetchImage(url: string) {
  // Use the Tauri HTTP plugin in the desktop app. It is not subject to CORS
  // and its allowed origins are configured in the capability file. The browser
  // fallback keeps this tester usable with `pnpm dev`.
  return isTauri() ? httpFetch(url, {
    headers: {
      Referer: "https://www.manhuagui.com/",
      "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36"
    }
  }) : window.fetch(url);
}

async function loadImage() {
  const url = imageUrl.value.trim();
  if (!url || imageLoading.value) {
    return;
  }

  imageLoading.value = true;
  imageError.value = "";
  try {
    const response = await fetchImage(url);
    if (!response.ok) {
      throw new Error(`Request failed with HTTP ${response.status}`);
    }

    const blob = await response.blob();
    if (blob.size === 0) {
      throw new Error("The server returned an empty response.");
    }
    if (blob.type && !blob.type.startsWith("image/")) {
      throw new Error(`The server returned ${blob.type} instead of an image.`);
    }

    await showImage(blob);
  } catch (error) {
    imageError.value = errorMessage(error);
  } finally {
    imageLoading.value = false;
  }
}

function onImageError() {
  imageError.value = "The fetched data could not be displayed as an image.";
  revokeImage();
  imageSrc.value = "";
}

onBeforeUnmount(revokeImage);
</script>

<template>
  <main class="container">
    <h1>Welcome to Tauri + Vue</h1>

    <div class="row">
      <a href="https://vite.dev" target="_blank">
        <img src="/vite.svg" class="logo vite" alt="Vite logo" />
      </a>
      <a href="https://tauri.app" target="_blank">
        <img src="/icon.svg" class="logo tauri" alt="Tauri logo" />
      </a>
      <a href="https://vuejs.org/" target="_blank">
        <img src="../assets/vue.svg" class="logo vue" alt="Vue logo" />
      </a>
    </div>
    <p>Click on the Tauri, Vite, and Vue logos to learn more.</p>

    <form class="greet" @submit.prevent="greet">
      <NInput v-model:value="name" class="greet-input" placeholder="Enter a name..." />
      <NButton attr-type="submit" type="primary">Greet</NButton>
    </form>
    <p>{{ greetMsg }}</p>

    <NCard class="image-tester" title="Image tester" size="small">
      <form class="image-form" @submit.prevent="loadImage">
        <NInput v-model:value="imageUrl" class="image-url" placeholder="Paste an image URL..." />
        <NButton attr-type="submit" type="primary" :loading="imageLoading">
          Load
        </NButton>
      </form>
      <p class="hint">
        Fetches the image through the Tauri HTTP plugin in the desktop app.
      </p>
      <NAlert v-if="imageError" class="image-error" type="error" :bordered="false">
        {{ imageError }}
      </NAlert>
      <div v-if="imageSrc" class="image-preview">
        <img :src="imageSrc" alt="Fetched preview" @error="onImageError" />
      </div>
    </NCard>

    <NCard class="settings" title="Settings" size="small">
      <div class="setting">
        <span>Theme</span>
        <NSelect v-model:value="settings.theme" class="theme-select" :options="themeOptions" />
      </div>
      <p class="hint">Saved to this device and restored on the next launch.</p>
    </NCard>
  </main>
</template>

<style scoped>
.logo.vite:hover {
  filter: drop-shadow(0 0 2em #747bff);
}

.logo.vue:hover {
  filter: drop-shadow(0 0 2em #249b73);
}

.greet {
  display: flex;
  justify-content: center;
  gap: 0.5rem;
}

.greet-input {
  width: 14rem;
}

.image-tester {
  margin: 2rem auto 0;
  max-width: 24rem;
  text-align: left;
}

.image-form {
  display: flex;
  gap: 0.5rem;
}

.image-url {
  flex: 1;
  min-width: 0;
}

.image-error {
  margin-top: 0.75rem;
}

.image-preview {
  margin-top: 0.75rem;
  padding: 0.5rem;
  border: 1px solid rgba(127, 127, 127, 0.3);
  border-radius: 8px;
  text-align: center;
}

.image-preview img {
  display: block;
  max-width: 100%;
  max-height: 20rem;
  margin: 0 auto;
  border-radius: 4px;
}

.settings {
  margin: 2rem auto 0;
  max-width: 20rem;
  text-align: left;
}

.setting {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

.theme-select {
  width: 8.5rem;
}

.hint {
  margin: 0.75rem 0 0;
  font-size: 0.8rem;
  line-height: 1.4;
  opacity: 0.65;
}
</style>
