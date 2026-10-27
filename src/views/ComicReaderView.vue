<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import type { ComicPlatform } from "../api/comics";
import { getComic, isComicPlatform } from "../api/comics";
import type { ReaderChapter } from "../api/reader";
import {
  getCachedReaderChapter,
  getPendingReaderChapter,
  getReaderChapter,
} from "../api/reader";
import ComicViewer from "../components/reader/ComicViewer.vue";
import { useBackNavigation } from "../composables/useBackNavigation";
import { useErrorMessage } from "../composables/useErrorMessage";
import { useLatestRequest } from "../composables/useLatestRequest";
import { useNextChapterPreload } from "../composables/useNextChapterPreload";
import { useReadingHistory } from "../composables/useReadingHistory";
import { AppError, toAppError } from "../errors";
import { useAppI18n } from "../i18n";
import {
  readerBoundaryAction,
  resolveReaderPage,
} from "../lib/reader-navigation";
import { setWindowTitle, updateWindowTitle } from "../router";
import {
  clampReaderPreloadCount,
  useSettingsStore,
} from "../stores/settings";

/** Internal query value that lands on the last page after a chapter switch. */
const LAST_PAGE_QUERY = "last";

const route = useRoute();
const router = useRouter();
const { goBack: back } = useBackNavigation();
const { t } = useAppI18n();
const formatError = useErrorMessage();
const settings = useSettingsStore();

const chapter = ref<ReaderChapter | null>(null);
const loading = ref(true);
const error = ref<AppError | null>(null);
const page = ref(1);
const preloadCount = computed(() =>
  clampReaderPreloadCount(settings.readerPreloadCount),
);
const latest = useLatestRequest();
let previousBodyOverflow = "";
let recordedPage = 0;

const platform = computed<ComicPlatform | null>(() => {
  const value = route.params.platform;
  return isComicPlatform(value) ? value : null;
});

const id = computed(() => {
  const value = route.params.id;
  return typeof value === "string" ? value : "";
});

const chapterId = computed(() => {
  const value = route.params.chapterId;
  return typeof value === "string" && value.length > 0 ? value : undefined;
});

useNextChapterPreload(
  chapter,
  page,
  platform,
  id,
  computed(() => settings.readerPreloadNextChapter),
  preloadCount,
);

// The reader owns the page counter, so history simply mirrors it. The target
// is snapshotted when a write is queued, which keeps a pending page from the
// previous chapter out of the next chapter's record.
const readingHistory = useReadingHistory(() => {
  const current = chapter.value;
  const currentPlatform = platform.value;
  if (!current || current.pages.length === 0 || !currentPlatform || !id.value) {
    return null;
  }
  return {
    platform: currentPlatform,
    id: id.value,
    title: current.title,
    thumb: current.thumb?.trim() || null,
    chapterId: current.chapterId,
    chapterTitle: current.chapterTitle,
    totalPages: current.pages.length,
  };
});

const errorTitle = computed(() => t("reader.loadFailed"));
const errorText = computed(() => {
  const current = error.value;
  return current ? formatError(current) : "";
});

function syncPageQuery(value: number) {
  const query = { ...route.query };
  if (value > 1) {
    query.page = String(value);
  } else {
    delete query.page;
  }

  const current = typeof route.query.page === "string" ? route.query.page : "";
  const next = typeof query.page === "string" ? query.page : "";
  if (current === next) {
    return;
  }

  void router.replace({ path: route.path, query });
}

function goBack() {
  if (platform.value && id.value) {
    back({
      name: "comic-detail",
      params: { platform: platform.value, id: id.value },
    });
    return;
  }
  back({ name: "home" });
}

function goToChapter(
  targetId: string,
  position: "start" | "end" = "start",
) {
  if (!platform.value || !id.value) {
    return;
  }
  // Persist the last page before the route params switch to another chapter.
  readingHistory.flush();
  // Chapter switches replace the current entry: the reader stays one history
  // step away from the detail page instead of stacking every chapter.
  void router.replace({
    name: "comic-reader",
    params: {
      platform: platform.value,
      id: id.value,
      chapterId: targetId,
    },
    // A previous chapter reached from the first page opens at its end so
    // backward reading is continuous; the explicit chapter buttons still
    // open the beginning.
    query: position === "end" ? { page: LAST_PAGE_QUERY } : {},
  });
}

function openPreviousChapter() {
  const target = chapter.value?.previous;
  if (target) {
    goToChapter(target.id, "start");
    return;
  }
  goBack();
}

function openNextChapter() {
  const target = chapter.value?.next;
  if (target) {
    goToChapter(target.id, "start");
    return;
  }
  goBack();
}

function goFromPageBoundary(direction: "previous" | "next") {
  const current = chapter.value;
  if (!current) {
    goBack();
    return;
  }

  const action = readerBoundaryAction({
    direction,
    page: page.value,
    total: current.pages.length,
    previousChapterId: current.previous?.id ?? null,
    nextChapterId: current.next?.id ?? null,
  });

  if (action.kind === "chapter") {
    goToChapter(action.chapterId, action.position);
    return;
  }
  if (action.kind === "turn") {
    page.value = action.page;
    return;
  }
  // The first/last chapter has nowhere else to go on that side.
  goBack();
}

function applyChapter(result: ReaderChapter, startAtEnd: boolean) {
  chapter.value = result;
  if (result.pages.length === 0) {
    page.value = 1;
    setWindowTitle(
      result.chapterTitle
        ? `${result.title} · ${result.chapterTitle} · ${t("app.name")}`
        : `${result.title} · ${t("app.name")}`,
    );
    syncPageQuery(1);
    return;
  }

  page.value = startAtEnd
    ? result.pages.length
    : resolveReaderPage(route.query.page, result.pages.length).page;
  setWindowTitle(
    result.chapterTitle
      ? `${result.title} · ${result.chapterTitle} · ${t("app.name")}`
      : `${result.title} · ${t("app.name")}`,
  );
  syncPageQuery(page.value);
  recordedPage = page.value;
  readingHistory.record(page.value);
}

async function load() {
  // Capture the outgoing chapter's last page before clearing its state.
  readingHistory.flush();
  const currentRequest = latest.begin();
  const currentPlatform = platform.value;
  const currentId = id.value;
  const currentChapterId = chapterId.value;
  const startAtEnd = route.query.page === LAST_PAGE_QUERY;

  if (!currentPlatform || !currentId) {
    loading.value = false;
    chapter.value = null;
    error.value = new AppError("invalidInput", "invalid reader route", {
      field: "id",
    });
    updateWindowTitle();
    return;
  }

  const cached = currentChapterId
    ? getCachedReaderChapter(currentPlatform, currentId, currentChapterId)
    : null;
  if (cached) {
    error.value = null;
    loading.value = false;
    applyChapter(cached, startAtEnd);
    return;
  }

  const pending = currentChapterId
    ? getPendingReaderChapter(currentPlatform, currentId, currentChapterId)
    : null;

  loading.value = true;
  error.value = null;
  chapter.value = null;
  page.value = 1;
  updateWindowTitle();

  try {
    const loadChapterId = currentChapterId;
    if (currentPlatform === "manhuagui" && !loadChapterId) {
      // A deep link without a chapter still has a valid work id. Resolve the
      // first chapter from the detail page, then replace the URL so history and
      // sharing keep the chapter id.
      const detail = await getComic(currentPlatform, currentId);
      if (!latest.isCurrent(currentRequest)) {
        return;
      }
      const first =
        detail.platform === "manhuagui"
          ? detail.extra.chapterGroups.flatMap((group) => group.chapters)[0]
          : undefined;
      if (!first) {
        error.value = new AppError(
          "notFound",
          "manhuagui work has no readable chapters",
          {},
        );
        loading.value = false;
        return;
      }
      await router.replace({
        name: "comic-reader",
        params: {
          platform: currentPlatform,
          id: currentId,
          chapterId: first.id,
        },
        query: route.query,
      });
      return;
    }

    const result = pending
      ? await pending.catch(() =>
          getReaderChapter(currentPlatform, currentId, loadChapterId),
        )
      : await getReaderChapter(currentPlatform, currentId, loadChapterId);
    if (!latest.isCurrent(currentRequest)) {
      return;
    }

    applyChapter(result, startAtEnd);
  } catch (cause) {
    if (!latest.isCurrent(currentRequest)) {
      return;
    }
    error.value = toAppError(cause);
    updateWindowTitle();
  } finally {
    if (latest.isCurrent(currentRequest)) {
      loading.value = false;
    }
  }
}

watch([platform, id, chapterId], () => void load(), { immediate: true });

watch(page, (value) => {
  if (chapter.value) {
    syncPageQuery(value);
    // The explicit record after loading already covers the first page.
    if (!loading.value && value !== recordedPage) {
      recordedPage = value;
      readingHistory.schedule(value);
    }
  }
});

watch(
  () => route.query.page,
  (value) => {
    if (!chapter.value) {
      return;
    }
    // `last` belongs to the pending chapter switch. Resolving it here would
    // write page 1 into the URL before load() can use it as a start position.
    if (value === LAST_PAGE_QUERY) {
      return;
    }
    const next = resolveReaderPage(value, chapter.value.pages.length).page;
    if (next !== page.value) {
      page.value = next;
    }
  },
);

onMounted(() => {
  // The reader is a fixed full-screen surface; stop the page underneath from
  // scrolling while it is open.
  previousBodyOverflow = document.body.style.overflow;
  document.body.style.overflow = "hidden";
});

onBeforeUnmount(() => {
  readingHistory.flush();
  document.body.style.overflow = previousBodyOverflow;
});
</script>

<template>
  <main class="reader-route">
    <div v-if="loading" class="reader-state" aria-busy="true">
      <NSpin size="large" />
      <p>{{ t("reader.loading") }}</p>
    </div>

    <div v-else-if="error" class="reader-state">
      <NResult
        status="error"
        :title="errorTitle"
        :description="errorText"
      >
        <template #footer>
          <div class="reader-state__actions">
            <NButton type="primary" @click="load">
              {{ t("reader.retry") }}
            </NButton>
            <NButton quaternary @click="goBack">
              {{ t("reader.back") }}
            </NButton>
          </div>
        </template>
      </NResult>
    </div>

    <ComicViewer
      v-else-if="chapter && chapter.pages.length > 0 && platform"
      v-model:page="page"
      :pages="chapter.pages"
      :platform="platform"
      :direction="settings.readerDirection"
      :title="chapter.title"
      :chapter-title="chapter.chapterTitle ?? undefined"
      :has-previous-chapter="chapter.previous !== null"
      :has-next-chapter="chapter.next !== null"
      :preload-count="preloadCount"
      @close="goBack"
      @previous-chapter="openPreviousChapter"
      @next-chapter="openNextChapter"
      @page-boundary="goFromPageBoundary"
    />

    <div v-else class="reader-state">
      <NResult
        status="404"
        :title="t('reader.empty')"
        :description="t('reader.emptyDescription')"
      >
        <template #footer>
          <NButton quaternary @click="goBack">
            {{ t("reader.back") }}
          </NButton>
        </template>
      </NResult>
    </div>
  </main>
</template>

<style scoped>
.reader-route {
  position: fixed;
  z-index: 20;
  top: var(--app-safe-area-top, 0px);
  right: 0;
  bottom: 0;
  left: 0;
  overflow: hidden;
  background: var(--app-background);
}

.reader-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  height: 100%;
  padding: 1.5rem;
  text-align: center;
}

.reader-state p {
  margin: 0;
  opacity: 0.7;
}

.reader-state__actions {
  display: flex;
  justify-content: center;
  gap: 0.75rem;
}
</style>
