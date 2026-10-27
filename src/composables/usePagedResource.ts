import { computed, getCurrentScope, onScopeDispose, ref } from "vue";
import type { ComputedRef, Ref } from "vue";
import type { AppError } from "../errors";
import { toAppError } from "../errors";

/** One page returned by a list command. */
export interface PagedResult<T> {
  total: number;
  entries: T[];
}

export interface PagedResourceOptions<T> {
  /** Defaults to 50, matching the list commands' default page size. */
  pageSize?: number;
  fetch: (offset: number, limit: number) => Promise<PagedResult<T>>;
}

export interface PagedResource<T> {
  entries: Ref<T[]>;
  total: Ref<number>;
  page: Ref<number>;
  pageCount: ComputedRef<number>;
  loading: Ref<boolean>;
  error: Ref<AppError | null>;
  /** Load the current page. `silent` keeps the current list and error on failure. */
  load: (silent?: boolean) => Promise<void>;
  changePage: (next: number) => Promise<void>;
  reload: () => Promise<void>;
  /** Return to the first page, e.g. after a filter changes, then load it. */
  reset: () => Promise<void>;
}

const DEFAULT_PAGE_SIZE = 50;

/**
 * Shared pagination state for the history, favorites and downloads lists.
 *
 * It owns the request token so a slow page cannot overwrite a newer one, and
 * clamps the page when the total shrinks (for example after deleting the last
 * row). Domain behavior such as group filters, polling or row actions stays in
 * the view.
 */
export function usePagedResource<T>(
  options: PagedResourceOptions<T>,
): PagedResource<T> {
  const pageSize = options.pageSize ?? DEFAULT_PAGE_SIZE;
  const entries = ref<T[]>([]) as Ref<T[]>;
  const total = ref(0);
  const page = ref(1);
  const loading = ref(true);
  const error = ref<AppError | null>(null);
  let requestId = 0;

  if (getCurrentScope()) {
    onScopeDispose(() => {
      requestId += 1;
    });
  }

  const pageCount = computed(() =>
    Math.max(1, Math.ceil(total.value / pageSize)),
  );

  async function load(silent = false): Promise<void> {
    const current = ++requestId;
    if (!silent) {
      loading.value = true;
      error.value = null;
    }

    try {
      const result = await options.fetch(
        (page.value - 1) * pageSize,
        pageSize,
      );
      if (current !== requestId) {
        return;
      }

      // A deletion can leave the current page past the end. Reload the last
      // page instead of showing an empty list with a valid page number.
      const lastPage = Math.max(1, Math.ceil(result.total / pageSize));
      if (page.value > lastPage) {
        page.value = lastPage;
        await load(silent);
        return;
      }

      total.value = result.total;
      entries.value = result.entries;
      error.value = null;
    } catch (cause) {
      if (current !== requestId) {
        return;
      }
      if (!silent) {
        error.value = toAppError(cause);
      }
    } finally {
      if (current === requestId) {
        loading.value = false;
      }
    }
  }

  async function changePage(next: number): Promise<void> {
    const target = Math.min(Math.max(1, next), pageCount.value);
    if (target === page.value) {
      return;
    }
    page.value = target;
    // Pagination sits below a long list; start the new page from the top.
    window.scrollTo({ top: 0, behavior: "smooth" });
    await load();
  }

  function reload(): Promise<void> {
    return load();
  }

  async function reset(): Promise<void> {
    page.value = 1;
    await load();
  }

  return {
    entries,
    total,
    page,
    pageCount,
    loading,
    error,
    load,
    changePage,
    reload,
    reset,
  };
}
