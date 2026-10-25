import { onScopeDispose, ref, watch } from "vue";
import type { Ref } from "vue";
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type { ComicPlatform } from "../api/comics";
import { waitForEchStartup } from "../api/ech";

interface CachedImage {
  path: string;
}

/**
 * Resolve one remote image through the Rust cache and expose the asset-protocol
 * URL the webview can render. Loading is explicit so thumbnails can wait for an
 * IntersectionObserver while the reader starts immediately.
 */
export function useCachedImage(
  platform: Ref<ComicPlatform>,
  url: Ref<string>,
) {
  const src = ref("");
  const loading = ref(false);
  const failed = ref(false);
  let requestId = 0;

  async function load() {
    const target = url.value.trim();
    const current = ++requestId;

    if (!target) {
      src.value = "";
      loading.value = false;
      failed.value = false;
      return;
    }

    loading.value = true;
    failed.value = false;

    try {
      await waitForEchStartup();
      const cached = await invoke<CachedImage>("fetch_image", {
        url: target,
        platform: platform.value,
      });
      if (current === requestId) {
        src.value = convertFileSrc(cached.path);
      }
    } catch {
      if (current === requestId) {
        src.value = "";
        failed.value = true;
      }
    } finally {
      if (current === requestId) {
        loading.value = false;
      }
    }
  }

  function reset() {
    requestId++;
    src.value = "";
    loading.value = false;
    failed.value = false;
  }

  function retry() {
    reset();
    void load();
  }

  function fail() {
    src.value = "";
    loading.value = false;
    failed.value = true;
  }

  watch([platform, url], reset);
  onScopeDispose(() => {
    requestId++;
  });

  return { src, loading, failed, load, reset, retry, fail };
}
