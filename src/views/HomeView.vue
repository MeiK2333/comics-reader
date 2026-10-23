<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore } from "../stores/settings";
import type { ThemeMode } from "../stores/settings";

const greetMsg = ref("");
const name = ref("");

const settings = useSettingsStore();

const themeOptions: { label: string; value: ThemeMode }[] = [
  { label: "System", value: "system" },
  { label: "Light", value: "light" },
  { label: "Dark", value: "dark" },
];

function setTheme(event: Event) {
  settings.theme = (event.target as HTMLSelectElement).value as ThemeMode;
}

async function greet() {
  // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
  greetMsg.value = await invoke("greet", { name: name.value });
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

    <form class="row" @submit.prevent="greet">
      <input id="greet-input" v-model="name" placeholder="Enter a name..." />
      <button type="submit">Greet</button>
    </form>
    <p>{{ greetMsg }}</p>

    <section class="settings">
      <h2>Settings</h2>
      <label class="setting">
        <span>Theme</span>
        <select :value="settings.theme" @change="setTheme">
          <option
            v-for="option in themeOptions"
            :key="option.value"
            :value="option.value"
          >
            {{ option.label }}
          </option>
        </select>
      </label>
      <p class="hint">Saved to this device and restored on the next launch.</p>
    </section>
  </main>
</template>

<style scoped>
.logo.vite:hover {
  filter: drop-shadow(0 0 2em #747bff);
}

.logo.vue:hover {
  filter: drop-shadow(0 0 2em #249b73);
}

#greet-input {
  margin-right: 5px;
}

.settings {
  margin: 2rem auto 0;
  max-width: 20rem;
  padding: 1rem 1.25rem;
  border: 1px solid rgba(127, 127, 127, 0.3);
  border-radius: 12px;
  text-align: left;
}

.settings h2 {
  margin: 0 0 0.75rem;
  font-size: 1rem;
}

.setting {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

.hint {
  margin: 0.75rem 0 0;
  font-size: 0.8rem;
  line-height: 1.4;
  opacity: 0.65;
}
</style>
