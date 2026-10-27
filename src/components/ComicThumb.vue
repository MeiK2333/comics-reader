<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { ComicPlatform } from "../platforms";
import { fetchCachedImage } from "../api/images";

const props = defineProps<{
  platform: ComicPlatform;
  thumb: string;
  title: string;
  /** Downloaded cover path; when present it wins over the remote thumbnail. */
  localPath?: string;
}>();

const root = ref<HTMLElement | null>(null);
const src = ref("");
const failed = ref(false);
let observer: IntersectionObserver | null = null;
let requestId = 0;

async function loadThumb() {
  const local = props.localPath?.trim() ?? "";
  const url = props.thumb.trim();
  if ((!local && !url) || failed.value) {
    return;
  }

  const id = ++requestId;
  if (local) {
    src.value = convertFileSrc(local);
    return;
  }

  try {
    // The platform headers and on-disk cache live in Rust, so thumbnails use
    // the same cache/in-flight pipeline as full-page images.
    const cached = await fetchCachedImage(props.platform, url);
    if (id === requestId) {
      src.value = convertFileSrc(cached.path);
    }
  } catch {
    if (id === requestId) {
      failed.value = true;
    }
  }
}

function onImageError() {
  failed.value = true;
  src.value = "";
}

function startLoading() {
  if (
    src.value ||
    failed.value ||
    (!props.thumb.trim() && !props.localPath?.trim())
  ) {
    return;
  }

  if (typeof IntersectionObserver === "undefined") {
    void loadThumb();
    return;
  }

  observer?.disconnect();
  observer = new IntersectionObserver(
    (entries) => {
      if (entries.some((entry) => entry.isIntersecting)) {
        observer?.disconnect();
        observer = null;
        void loadThumb();
      }
    },
    { rootMargin: "240px" },
  );

  if (root.value) {
    observer.observe(root.value);
  }
}

function reset() {
  requestId++;
  src.value = "";
  failed.value = false;
  observer?.disconnect();
  observer = null;
  startLoading();
}

watch(() => [props.platform, props.thumb, props.localPath], reset);
onMounted(startLoading);
onBeforeUnmount(() => {
  requestId++;
  observer?.disconnect();
});
</script>

<template>
  <div ref="root" class="comic-thumb">
    <img
      v-if="src"
      :src="src"
      :alt="title"
      loading="lazy"
      decoding="async"
      @error="onImageError"
    />
    <span v-else class="comic-thumb__placeholder" aria-hidden="true">
      {{ title.trim().slice(0, 1) }}
    </span>
  </div>
</template>

<style scoped>
.comic-thumb {
  width: 100%;
  overflow: hidden;
  background: rgba(127, 127, 127, 0.14);
}

/* Let the cached cover keep its intrinsic ratio; cropping it to a fixed frame
   cut off the top and bottom of non-3:4 artwork. */
.comic-thumb img {
  display: block;
  width: 100%;
  height: auto;
}

.comic-thumb__placeholder {
  display: grid;
  place-items: center;
  width: 100%;
  aspect-ratio: 3 / 4;
  font-size: 2.5rem;
  font-weight: 600;
  color: inherit;
  opacity: 0.35;
  text-transform: uppercase;
}
</style>
