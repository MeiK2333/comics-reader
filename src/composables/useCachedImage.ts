import { onScopeDispose, ref, watch } from "vue";
import type { Ref } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { ComicPlatform } from "../platforms";
import {
  fetchCachedImage,
  forgetCachedImage,
  getCachedImage,
} from "../api/images";

/**
 * Resolve one remote image through the Rust cache and expose the asset-protocol
 * URL the webview can render. Loading is explicit so thumbnails can wait for an
 * IntersectionObserver while the reader starts immediately.
 */
export function useCachedImage(
  platform: Ref<ComicPlatform>,
  url: Ref<string>,
  localPath: Ref<string | null>,
) {
  const src = ref("");
  const loading = ref(false);
  const failed = ref(false);
  let requestId = 0;

  async function load() {
    const target = url.value.trim();
    const local = localPath.value?.trim() ?? "";
    const current = ++requestId;

    // A downloaded page is already on disk; loading it must not touch the
    // network or wait for the image cache.
    if (local) {
      src.value = convertFileSrc(local);
      loading.value = false;
      failed.value = false;
      return;
    }

    if (!target) {
      src.value = "";
      loading.value = false;
      failed.value = false;
      return;
    }

    const resolved = getCachedImage(platform.value, target);
    if (resolved) {
      src.value = convertFileSrc(resolved.path);
      loading.value = false;
      failed.value = false;
      return;
    }

    loading.value = true;
    failed.value = false;

    try {
      const cached = await fetchCachedImage(platform.value, target);
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
    if (!localPath.value?.trim()) {
      forgetCachedImage(platform.value, url.value.trim());
    }
    src.value = "";
    loading.value = false;
    failed.value = true;
  }

  watch([platform, url, localPath], reset);
  onScopeDispose(() => {
    requestId++;
  });

  return { src, loading, failed, load, reset, retry, fail };
}
