<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { ComicPlatform } from "../api/comics";

const props = defineProps<{
  platform: ComicPlatform;
  thumb: string;
  title: string;
}>();

interface CachedImage {
  path: string;
}

const root = ref<HTMLElement | null>(null);
const src = ref("");
const failed = ref(false);
let observer: IntersectionObserver | null = null;
let requestId = 0;

async function loadThumb() {
  const url = props.thumb.trim();
  if (!url || failed.value) {
    return;
  }

  const id = ++requestId;
  try {
    // The platform headers and on-disk cache live in Rust, so thumbnails use
    // the same path as full-page images instead of hotlinking the CDN.
    const cached = await invoke<CachedImage>("fetch_image", {
      url,
      platform: props.platform,
    });
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
  if (src.value || failed.value || !props.thumb.trim()) {
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

watch(() => [props.platform, props.thumb], reset);
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
