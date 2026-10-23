<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useMessage } from "naive-ui";
import { useSettingsStore } from "../stores/settings";
import type { ThemeMode } from "../stores/settings";

const message = useMessage();
const settings = useSettingsStore();

const greetMsg = ref("");
const name = ref("");

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
      <NInput
        v-model:value="name"
        class="greet-input"
        placeholder="Enter a name..."
      />
      <NButton attr-type="submit" type="primary">Greet</NButton>
    </form>
    <p>{{ greetMsg }}</p>

    <NCard class="settings" title="Settings" size="small">
      <div class="setting">
        <span>Theme</span>
        <NSelect
          v-model:value="settings.theme"
          class="theme-select"
          :options="themeOptions"
        />
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
