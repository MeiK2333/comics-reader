<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import type { CSSProperties } from "vue";
import type { ComicPlatform } from "../../platforms";
import type { ReaderPage } from "../../api/reader";
import { useCachedImage } from "../../composables/useCachedImage";
import type { ReaderImageSize } from "../../composables/useReaderZoom";
import { useAppI18n } from "../../i18n";

const props = defineProps<{
  platform: ComicPlatform;
  page: ReaderPage;
  pageNumber: number;
  total: number;
  imageStyle: CSSProperties;
}>();

const emit = defineEmits<{
  size: [size: ReaderImageSize];
}>();

const { t } = useAppI18n();
const platform = computed(() => props.platform);
const url = computed(() => props.page.url);
const localPath = computed(() => props.page.localPath);
const { src, loading, failed, load, retry, fail } = useCachedImage(
  platform,
  url,
  localPath,
);

const image = ref<HTMLImageElement | null>(null);
const alt = computed(() =>
  t("reader.pageAlt", { current: props.pageNumber, total: props.total }),
);
let observer: ResizeObserver | null = null;

function emitSize() {
  const element = image.value;
  if (!element || element.naturalWidth === 0) {
    return;
  }
  emit("size", {
    width: element.offsetWidth,
    height: element.offsetHeight,
  });
}

watch(
  [platform, url, localPath],
  () => {
    void load();
  },
  { immediate: true },
);

watch(image, (element) => {
  observer?.disconnect();
  observer = null;

  if (!element || typeof ResizeObserver === "undefined") {
    return;
  }
  observer = new ResizeObserver(emitSize);
  observer.observe(element);
});

onBeforeUnmount(() => {
  observer?.disconnect();
});
</script>

<template>
  <div class="reader-page">
    <img
      v-if="src"
      ref="image"
      :src="src"
      :alt="alt"
      :width="page.width ?? undefined"
      :height="page.height ?? undefined"
      :style="imageStyle"
      decoding="async"
      draggable="false"
      @load="emitSize"
      @error="fail"
    />

    <div v-if="loading" class="reader-page__status" role="status">
      <NSpin size="large" />
    </div>

    <div v-else-if="failed || !src" class="reader-page__error" role="alert">
      <p>{{ t("reader.imageFailed") }}</p>
      <NButton size="small" type="primary" @click="retry">
        {{ t("reader.retry") }}
      </NButton>
    </div>
  </div>
</template>

<style scoped>
.reader-page {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  overflow: hidden;
  user-select: none;
}

.reader-page img {
  display: block;
  width: auto;
  height: auto;
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  transform-origin: center center;
  will-change: transform;
  user-select: none;
  -webkit-user-drag: none;
  pointer-events: none;
}

.reader-page__status {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  pointer-events: none;
}

.reader-page__error {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  padding: 1.5rem;
  text-align: center;
}

.reader-page__error p {
  margin: 0;
  opacity: 0.75;
}
</style>
