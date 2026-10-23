<script setup lang="ts">
import { RouterView } from "vue-router";
import { useTheme } from "./composables/useTheme";

useTheme();
</script>

<template>
  <RouterView />
</template>

<style>
:root {
  color-scheme: light;

  --color-text: #0f0f0f;
  --color-bg: #f6f6f6;
  --color-surface: #ffffff;
  --color-surface-active: #e8e8e8;
  --color-accent: #396cd8;
  --color-link-hover: #535bf2;

  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;

  color: var(--color-text);
  background-color: var(--color-bg);

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

:root[data-theme="dark"] {
  color-scheme: dark;

  --color-text: #f6f6f6;
  --color-bg: #2f2f2f;
  --color-surface: #0f0f0f98;
  --color-surface-active: #0f0f0f69;
  --color-link-hover: #24c8db;
}

.container {
  margin: 0;
  padding-top: 10vh;
  display: flex;
  flex-direction: column;
  justify-content: center;
  text-align: center;
}

.logo {
  height: 6em;
  padding: 1.5em;
  will-change: filter;
  transition: 0.75s;
}

.logo.tauri:hover {
  filter: drop-shadow(0 0 2em #24c8db);
}

.row {
  display: flex;
  justify-content: center;
}

a {
  font-weight: 500;
  color: #646cff;
  text-decoration: inherit;
}

a:hover {
  color: var(--color-link-hover);
}

h1 {
  text-align: center;
}

input,
button,
select {
  border-radius: 8px;
  border: 1px solid transparent;
  padding: 0.6em 1.2em;
  font-size: 1em;
  font-weight: 500;
  font-family: inherit;
  color: var(--color-text);
  background-color: var(--color-surface);
  transition: border-color 0.25s;
  box-shadow: 0 2px 2px rgba(0, 0, 0, 0.2);
}

button,
select {
  cursor: pointer;
}

button:hover {
  border-color: var(--color-accent);
}
button:active {
  border-color: var(--color-accent);
  background-color: var(--color-surface-active);
}

input,
button,
select {
  outline: none;
}
</style>
