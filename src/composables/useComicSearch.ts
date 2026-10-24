import { computed, ref } from "vue";
import {
  searchComics,
  type ComicPlatform,
  type ComicSearchResult,
} from "../api/comics";
import type { AppError } from "../errors";
import { toAppError } from "../errors";

/**
 * Own the search form and request lifecycle for the home page.
 *
 * Every request captures a generation id: a slow response from a previous
 * query, page or platform is dropped instead of overwriting newer results.
 */
export function useComicSearch() {
  const platform = ref<ComicPlatform>("manhuagui");
  const query = ref("");
  const page = ref(1);
  const loading = ref(false);
  const response = ref<ComicSearchResult | null>(null);
  const error = ref<AppError | null>(null);
  let requestId = 0;

  const hasSearched = computed(
    () => loading.value || response.value !== null || error.value !== null,
  );

  async function runSearch(targetPage: number) {
    const text = query.value.trim();
    if (!text || loading.value) {
      return;
    }

    const id = ++requestId;
    loading.value = true;
    error.value = null;
    page.value = targetPage;

    try {
      const result = await searchComics(platform.value, text, targetPage);
      if (id === requestId) {
        response.value = result;
      }
    } catch (cause) {
      if (id === requestId) {
        response.value = null;
        error.value = toAppError(cause);
      }
    } finally {
      if (id === requestId) {
        loading.value = false;
      }
    }
  }

  function search() {
    if (!query.value.trim() || loading.value) {
      return;
    }
    void runSearch(1);
  }

  function selectPlatform(next: ComicPlatform) {
    if (platform.value === next) {
      return;
    }

    // Results belong to the old platform, so clear them and invalidate any
    // in-flight response before the next search.
    platform.value = next;
    requestId++;
    response.value = null;
    error.value = null;
    page.value = 1;
    loading.value = false;
  }

  function changePage(next: number) {
    if (next === page.value) {
      return;
    }
    void runSearch(next);
  }

  return {
    platform,
    query,
    page,
    loading,
    response,
    error,
    hasSearched,
    search,
    selectPlatform,
    changePage,
  };
}
