<script setup lang="ts">
import type { InputInst } from "naive-ui";
import { computed, nextTick, ref } from "vue";
import { useAppI18n } from "../../i18n";

const props = withDefaults(
  defineProps<{
    page: number;
    total: number;
    disabled?: boolean;
  }>(),
  {
    disabled: false,
  },
);

const emit = defineEmits<{
  submit: [page: number];
}>();

const { t } = useAppI18n();
const editing = ref(false);
const draft = ref("");
const input = ref<InputInst | null>(null);

const label = computed(() =>
  t("reader.pageIndicator", { current: props.page, total: props.total }),
);

function startEditing() {
  if (props.disabled || props.total < 1) {
    return;
  }
  draft.value = String(props.page);
  editing.value = true;
  void nextTick(() => {
    input.value?.focus();
    input.value?.select();
  });
}

function commit() {
  if (!editing.value) {
    return;
  }
  // Leave edit mode first so the blur that follows unmounting cannot commit
  // twice.
  editing.value = false;
  const parsed = Number.parseInt(draft.value, 10);
  if (!Number.isFinite(parsed)) {
    return;
  }
  const target = Math.min(Math.max(parsed, 1), Math.max(props.total, 1));
  if (target !== props.page) {
    emit("submit", target);
  }
}

function cancel() {
  editing.value = false;
}

function onKeydown(event: KeyboardEvent) {
  // The reader listens for page turns, Home/End and Escape on window; a key
  // typed in this field must not reach it.
  event.stopPropagation();
  if (event.key === "Enter") {
    event.preventDefault();
    commit();
  } else if (event.key === "Escape") {
    event.preventDefault();
    cancel();
  }
}

function digitsOnly(value: string) {
  return /^\d*$/.test(value);
}
</script>

<template>
  <NInput
    v-if="editing"
    ref="input"
    v-model:value="draft"
    class="reader-page-jump__input"
    size="tiny"
    autofocus
    :maxlength="9"
    :allow-input="digitsOnly"
    :input-props="{
      inputmode: 'numeric',
      enterKeyHint: 'go',
      'aria-label': t('reader.jumpToPage'),
    }"
    @keydown="onKeydown"
    @blur="commit"
  />
  <button
    v-else
    type="button"
    class="reader-page-jump__button"
    :disabled="disabled"
    :title="t('reader.jumpToPage')"
    :aria-label="
      t('reader.jumpToPageLabel', { current: page, total })
    "
    @click="startEditing"
  >
    {{ label }}
  </button>
</template>

<style scoped>
.reader-page-jump__button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-height: 2rem;
  padding: 0 0.45rem;
  border: 0;
  border-radius: 0.4rem;
  background: transparent;
  color: inherit;
  font: inherit;
  font-variant-numeric: tabular-nums;
  opacity: 0.7;
  cursor: pointer;
  transition:
    background-color 120ms ease,
    opacity 120ms ease;
}

.reader-page-jump__button:hover,
.reader-page-jump__button:focus-visible {
  background: rgba(127, 127, 127, 0.16);
  opacity: 1;
}

.reader-page-jump__button:disabled {
  cursor: default;
  opacity: 0.4;
}

.reader-page-jump__input {
  width: 5.2rem;
}
</style>
