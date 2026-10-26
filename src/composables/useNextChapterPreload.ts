import { onScopeDispose, watch } from "vue";
import type { Ref } from "vue";
import type { ComicPlatform } from "../api/comics";
import { prefetchCachedImages } from "../api/images";
import type { ReaderChapter } from "../api/reader";
import { prefetchReaderChapter } from "../api/reader";
import { clampReaderPreloadCount } from "../stores/settings";

/**
 * When the reader reaches the last page of a chapter, fetch the next chapter's
 * page list and warm its first few images. This is only a cache warm-up: the
 * normal chapter navigation still owns loading and history, and every request
 * is best effort.
 */
export function useNextChapterPreload(
  chapter: Ref<ReaderChapter | null>,
  page: Ref<number>,
  platform: Ref<ComicPlatform | null>,
  id: Ref<string>,
  enabled: Ref<boolean>,
  count: Ref<number>,
) {
  const warmed = new Set<string>();
  const inFlight = new Set<string>();
  let disposed = false;

  async function warm(
    key: string,
    currentPlatform: ComicPlatform,
    currentId: string,
    nextChapterId: string,
    imageCount: number,
  ) {
    try {
      const next = await prefetchReaderChapter(
        currentPlatform,
        currentId,
        nextChapterId,
      );
      if (disposed) {
        return;
      }
      const urls = next.pages
        .slice(0, imageCount)
        .filter((item) => !item.localPath)
        .map((item) => item.url.trim())
        .filter((url) => url.length > 0);
      await prefetchCachedImages(currentPlatform, urls);
      if (!disposed) {
        warmed.add(key);
      }
    } catch {
      // A failed warm-up must not affect reading; the next chapter still loads
      // through the normal reader path.
    } finally {
      inFlight.delete(key);
    }
  }

  watch(
    [chapter, page, enabled, count],
    () => {
      const current = chapter.value;
      if (!enabled.value || !current || current.pages.length === 0) {
        return;
      }
      if (page.value < current.pages.length) {
        return;
      }

      const next = current.next;
      const currentPlatform = platform.value;
      const currentId = id.value;
      if (!next || !currentPlatform || !currentId) {
        return;
      }

      const imageCount = clampReaderPreloadCount(count.value);
      const key = `${currentPlatform}\u0000${currentId}\u0000${next.id}\u0000${imageCount}`;
      if (warmed.has(key) || inFlight.has(key)) {
        return;
      }

      inFlight.add(key);
      void warm(key, currentPlatform, currentId, next.id, imageCount);
    },
    { immediate: true },
  );

  onScopeDispose(() => {
    disposed = true;
  });
}
