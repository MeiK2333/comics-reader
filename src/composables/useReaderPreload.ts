import { onScopeDispose, watch } from "vue";
import type { Ref } from "vue";
import type { ComicPlatform } from "../platforms";
import { prefetchCachedImage } from "../api/images";
import type { ReaderPage } from "../api/reader";
import {
  READER_PRELOAD_MAX,
  READER_PRELOAD_MIN,
} from "../stores/settings";

const PRELOAD_DELAY_MS = 200;
const PRELOAD_CONCURRENCY = 2;

/**
 * Warm the next few pages of the current chapter.
 *
 * The viewer already keeps the previous/current/next slots mounted, so this is
 * purely a read-ahead queue. It is deliberately best effort: a failed prefetch
 * must never surface in the reader, and switching pages or chapters stops the
 * old queue instead of letting it download stale pages.
 */
export function useReaderPreload(
  platform: Ref<ComicPlatform>,
  pages: Ref<ReaderPage[]>,
  page: Ref<number>,
  count: Ref<number>,
) {
  const warmed = new Set<string>();
  let generation = 0;
  let timer: ReturnType<typeof setTimeout> | undefined;

  function clearTimer() {
    if (timer !== undefined) {
      clearTimeout(timer);
      timer = undefined;
    }
  }

  function schedule() {
    generation += 1;
    const current = generation;
    clearTimer();
    timer = setTimeout(() => {
      timer = undefined;
      void run(current);
    }, PRELOAD_DELAY_MS);
  }

  async function run(current: number) {
    const upcoming = Math.min(
      READER_PRELOAD_MAX,
      Math.max(READER_PRELOAD_MIN, Math.round(count.value)),
    );
    const start = page.value - 1;
    const urls: string[] = [];

    for (let offset = 1; offset <= upcoming; offset += 1) {
      const page = pages.value[start + offset];
      if (!page || page.localPath) {
        continue;
      }
      const url = page.url.trim();
      if (url && !warmed.has(url)) {
        urls.push(url);
      }
    }

    let next = 0;
    async function worker() {
      while (current === generation) {
        const index = next;
        next += 1;
        if (index >= urls.length) {
          return;
        }

        const url = urls[index];
        try {
          await prefetchCachedImage(platform.value, url);
        } catch {
          // The visible page still has its own retry path; prefetch failures
          // only mean the next turn will not be instant.
        }
        if (current === generation) {
          warmed.add(url);
        }
      }
    }

    await Promise.all(
      Array.from({ length: PRELOAD_CONCURRENCY }, () => worker()),
    );
  }

  watch(
    [platform, pages],
    () => {
      warmed.clear();
      schedule();
    },
  );
  watch([page, count], schedule, { immediate: true });

  onScopeDispose(() => {
    generation += 1;
    clearTimer();
  });
}
