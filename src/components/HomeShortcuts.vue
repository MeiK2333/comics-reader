<script setup lang="ts">
import { RouterLink } from "vue-router";
import { useAppI18n } from "../i18n";
import { SHORTCUT_LABEL_KEYS, SHORTCUTS } from "../shortcuts";
import ShortcutIcon from "./ShortcutIcon.vue";

const { t } = useAppI18n();
</script>

<template>
  <nav class="shortcuts" :aria-label="t('home.shortcuts.label')">
    <RouterLink
      v-for="item in SHORTCUTS"
      :key="item.feature"
      :to="item.path"
      class="shortcut"
    >
      <span class="shortcut-icon">
        <ShortcutIcon :name="item.feature" />
      </span>
      <span class="shortcut-label">
        {{ t(SHORTCUT_LABEL_KEYS[item.feature]) }}
      </span>
    </RouterLink>
  </nav>
</template>

<style scoped>
.shortcuts {
  display: flex;
  justify-content: center;
  gap: 0.4rem;
  margin-top: 0.35rem;
}

.shortcut {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.4rem;
  min-width: 4.1rem;
  padding: 0.55rem 0.5rem 0.45rem;
  border-radius: 12px;
  color: inherit;
  text-decoration: none;
  transition: background-color 150ms ease;
}

.shortcut:hover {
  background-color: rgba(127, 127, 127, 0.1);
}

.shortcut:focus-visible {
  outline: 2px solid currentColor;
  outline-offset: 2px;
}

.shortcut-icon {
  display: grid;
  place-items: center;
  width: 42px;
  height: 42px;
  border-radius: 50%;
  background-color: rgba(127, 127, 127, 0.08);
}

.shortcut-label {
  font-size: 0.8rem;
  line-height: 1;
  white-space: nowrap;
  opacity: 0.72;
}

@media (max-width: 560px) {
  .shortcuts {
    gap: 0;
  }

  .shortcut {
    min-width: 3.6rem;
    padding-inline: 0.25rem;
  }

  .shortcut-icon {
    width: 38px;
    height: 38px;
  }
}
</style>
