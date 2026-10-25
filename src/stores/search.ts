import { computed, ref } from "vue";
import { defineStore } from "pinia";
import {
  searchComics,
  type ComicPlatform,
  type ComicSearchResult,
} from "../api/comics";
import type { AppError } from "../errors";
import { toAppError } from "../errors";

/**
 * In-memory search session shared by the home and results views.
 *
 * The results route deliberately has no query parameters, so the active
 * query, page and response live here. A page reload starts a fresh session
 * and the route guard sends `/search` back to the home page.
 */
export const useSearchStore = defineStore("search", () => {
  const platform = ref<ComicPlatform>("manhuagui");
  const query = ref("");
  const page = ref(1);
  const loading = ref(false);
  const response = ref<ComicSearchResult | null>(null);
  const error = ref<AppError | null>(null);
  let requestId = 0;

  const active = computed(() => query.value.trim().length > 0);

  async function runSearch(targetPage: number) {
    const text = query.value.trim();
    if (!text) {
      return;
    }

    const id = ++requestId;
    loading.value = true;
    error.value = null;
    // Drop the previous query's results immediately so the new URL-free
    // search page never shows stale cards under a different keyword.
    response.value = null;
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

  function beginSearch(nextPlatform: ComicPlatform, text: string) {
    const trimmed = text.trim();
    if (!trimmed) {
      return;
    }

    platform.value = nextPlatform;
    query.value = trimmed;
    page.value = 1;
    void runSearch(1);
  }

  function changePage(next: number) {
    if (next < 1 || next === page.value) {
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
    active,
    beginSearch,
    changePage,
  };
});
