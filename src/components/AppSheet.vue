<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { isMobileWebView } from "../device";
import { useAppI18n } from "../i18n";

const props = withDefaults(
  defineProps<{
    show: boolean;
    title: string;
    /** Desktop card width in pixels; the mobile sheet always spans the viewport. */
    width?: number;
    /** Blocks closing while a request is in flight. */
    busy?: boolean;
  }>(),
  { width: 440, busy: false },
);

const emit = defineEmits<{
  "update:show": [value: boolean];
}>();

const { t } = useAppI18n();

// Device, not viewport width: a narrow desktop window keeps the dialog.
const sheet = isMobileWebView();
const closable = computed(() => !props.busy);

/**
 * A long press opens this overlay while the pointer is still down, so the
 * release that follows must not dismiss what it just opened.
 */
const justOpened = ref(false);
let guardTimer: ReturnType<typeof setTimeout> | null = null;

function clearGuard() {
  if (guardTimer !== null) {
    clearTimeout(guardTimer);
    guardTimer = null;
  }
}

watch(
  () => props.show,
  (show) => {
    clearGuard();
    if (!show) {
      justOpened.value = false;
      return;
    }
    justOpened.value = true;
    guardTimer = setTimeout(() => {
      justOpened.value = false;
      guardTimer = null;
    }, 400);
  },
  { immediate: true },
);

onBeforeUnmount(clearGuard);

const dismissable = computed(() => !props.busy && !justOpened.value);
const dialogStyle = computed(() => ({
  width: `min(${props.width}px, calc(100vw - var(--app-safe-area-left) - var(--app-safe-area-right) - 2rem))`,
}));
</script>

<template>
  <NDrawer
    v-if="sheet"
    class="app-sheet"
    placement="bottom"
    height="auto"
    :show="show"
    :close-on-esc="closable"
    :mask-closable="dismissable"
    @update:show="emit('update:show', $event)"
  >
    <div class="app-sheet__panel">
      <header class="app-sheet__header">
        <h2 class="app-sheet__title">{{ title }}</h2>
        <NButton
          v-if="closable"
          attr-type="button"
          quaternary
          circle
          class="app-sheet__close"
          :aria-label="t('common.close')"
          :title="t('common.close')"
          @click="emit('update:show', false)"
        >
          <template #icon>
            <svg
              class="app-sheet__close-icon"
              viewBox="0 0 24 24"
              aria-hidden="true"
              focusable="false"
            >
              <path
                d="M6 6l12 12M18 6L6 18"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
              />
            </svg>
          </template>
        </NButton>
      </header>

      <div class="app-sheet__body">
        <slot />
      </div>

      <footer
        v-if="$slots.footer"
        class="app-sheet__footer app-sheet__footer--sheet"
      >
        <slot name="footer" />
      </footer>
    </div>
  </NDrawer>

  <NModal
    v-else
    preset="card"
    :show="show"
    :title="title"
    :style="dialogStyle"
    :closable="closable"
    :mask-closable="dismissable"
    :close-on-esc="closable"
    @update:show="emit('update:show', $event)"
  >
    <slot />
    <template v-if="$slots.footer" #footer>
      <div class="app-sheet__footer app-sheet__footer--dialog">
        <slot name="footer" />
      </div>
    </template>
  </NModal>
</template>

<style scoped>
.app-sheet__panel {
  display: flex;
  flex-direction: column;
  max-height: min(86dvh, 640px);
}

.app-sheet__header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex: none;
  padding: 0.75rem 0.75rem 0.5rem 1rem;
}

.app-sheet__title {
  flex: 1;
  min-width: 0;
  margin: 0;
  overflow: hidden;
  font-size: 1rem;
  font-weight: 600;
  line-height: 1.4;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.app-sheet__close {
  flex: none;
}

.app-sheet__close-icon {
  display: block;
  width: 18px;
  height: 18px;
}

.app-sheet__body {
  flex: 1 1 auto;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  padding: 0.25rem 1rem 0.5rem;
}

.app-sheet__footer {
  display: flex;
  flex: none;
  gap: 0.5rem;
  width: 100%;
}

.app-sheet__footer--sheet {
  padding: 0.6rem 1rem calc(0.6rem + var(--app-safe-area-bottom));
  border-top: 1px solid rgba(128, 128, 128, 0.18);
}

.app-sheet__footer--dialog {
  justify-content: flex-end;
}
</style>
