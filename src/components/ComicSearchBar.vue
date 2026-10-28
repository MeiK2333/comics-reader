<script setup lang="ts">
import { computed } from "vue";
import type { ComicPlatform } from "../platforms";
import { platformFor } from "../platforms";
import { useAppI18n } from "../i18n";
import DirectOpenIcon from "./DirectOpenIcon.vue";
import PlatformPicker from "./PlatformPicker.vue";
import SearchIcon from "./SearchIcon.vue";

const props = defineProps<{
  platform: ComicPlatform;
  query: string;
  loading: boolean;
}>();

const emit = defineEmits<{
  "update:query": [query: string];
  "select-platform": [platform: ComicPlatform];
  search: [];
  "direct-open": [id: string];
}>();

const { t } = useAppI18n();

// A platform whose work ids are plain numbers can open the detail route from
// the query itself instead of spending a search request.
const directId = computed(() => {
  if (!platformFor(props.platform).numericIdDirectOpen) {
    return "";
  }
  const trimmed = props.query.trim();
  return /^\d+$/.test(trimmed) ? trimmed : "";
});

function submit() {
  if (!props.query.trim() || props.loading) {
    return;
  }
  emit("search");
}
</script>

<template>
  <form class="search-form" @submit.prevent="submit">
    <PlatformPicker
      :platform="platform"
      @select="emit('select-platform', $event)"
    />
    <NInput
      :value="query"
      class="search-input"
      size="large"
      :bordered="false"
      placeholder=""
      :input-props="{ 'aria-label': t('home.search.submit') }"
      :disabled="loading"
      clearable
      autofocus
      @update:value="emit('update:query', $event)"
    />
    <NButton
      v-if="directId"
      attr-type="button"
      type="primary"
      quaternary
      circle
      size="large"
      class="direct-action"
      :aria-label="t('home.search.directOpen', { id: directId })"
      @click="emit('direct-open', directId)"
    >
      <template #icon>
        <DirectOpenIcon />
      </template>
    </NButton>
    <NButton
      attr-type="submit"
      type="primary"
      quaternary
      circle
      size="large"
      class="search-action"
      :loading="loading"
      :disabled="!query.trim()"
      :aria-label="t('home.search.submit')"
    >
      <template #icon>
        <SearchIcon />
      </template>
    </NButton>
    <slot name="trailing" />
  </form>
</template>

<style scoped>
.search-form {
  display: flex;
  gap: 0.1rem;
  align-items: center;
  width: min(100%, 760px);
  margin-top: 0.5rem;
  padding: 0.35rem;
  border: 1px solid rgba(127, 127, 127, 0.28);
  border-radius: 999px;
  background: rgba(127, 127, 127, 0.07);
  /* Keep the pill inside its parent width; otherwise the padding and border
     push the right edge past the page gutter. */
  box-sizing: border-box;
  view-transition-name: search-form;
}

.search-input {
  flex: 1;
  min-width: 0;
  text-align: left;
  background-color: transparent !important;
  box-shadow: none !important;
  --n-color: transparent !important;
  --n-color-focus: transparent !important;
  --n-padding-left: 3px !important;
  --n-padding-right: 3px !important;
}

.search-input :deep(.n-input-wrapper) {
  padding-left: 3px !important;
  padding-right: 3px !important;
}

.search-input :deep(.n-input__border),
.search-input :deep(.n-input__state-border) {
  display: none !important;
}

.search-input :deep(.n-input__input-el) {
  text-align: left;
}

.direct-action {
  flex: none;
}

.direct-action :deep(.n-button__icon),
.search-action :deep(.n-button__icon) {
  margin-right: 0;
}

.search-action {
  flex: none;
  view-transition-name: search-action;
}
</style>
