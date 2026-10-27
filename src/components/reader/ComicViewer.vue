<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { CSSProperties } from "vue";
import type { ComicPlatform } from "../../api/comics";
import type { ReaderPage } from "../../api/reader";
import { usePageTurn, TURN_DURATION_MS } from "../../composables/usePageTurn";
import { useReaderGestures } from "../../composables/useReaderGestures";
import { useReaderKeyboard } from "../../composables/useReaderKeyboard";
import { useReaderPreload } from "../../composables/useReaderPreload";
import { useReaderWheel } from "../../composables/useReaderWheel";
import { useReaderZoom } from "../../composables/useReaderZoom";
import type { ReaderImageSize } from "../../composables/useReaderZoom";
import { useAppI18n } from "../../i18n";
import type { ReaderDirection } from "../../stores/settings";
import ReaderChrome from "./ReaderChrome.vue";
import ReaderTrack from "./ReaderTrack.vue";

const props = withDefaults(
  defineProps<{
    pages: ReaderPage[];
    platform: ComicPlatform;
    direction: ReaderDirection;
    page: number;
    title: string;
    chapterTitle?: string;
    hasPreviousChapter?: boolean;
    hasNextChapter?: boolean;
    preloadCount?: number;
  }>(),
  {
    chapterTitle: "",
    hasPreviousChapter: false,
    hasNextChapter: false,
    preloadCount: 3,
  },
);

const emit = defineEmits<{
  "update:page": [page: number];
  "previous-chapter": [];
  "next-chapter": [];
  "page-boundary": [direction: "previous" | "next"];
  close: [];
}>();

const { t } = useAppI18n();
const root = ref<HTMLElement | null>(null);
const stage = ref<HTMLElement | null>(null);
const imageSize = ref<ReaderImageSize | null>(null);
const pageSizes = new Map<number, ReaderImageSize>();
const chromeVisible = ref(false);
const isFullscreen = ref(false);

const total = computed(() => props.pages.length);
const horizontal = computed(() => props.direction !== "ttb");
const identityStyle: CSSProperties = {};

const {
  scale,
  offset,
  percent,
  zoomBy,
  zoomStep,
  setScale,
  panBy,
  reset,
  reflow,
} = useReaderZoom(stage, imageSize);

const {
  displayPage,
  turnTarget,
  trackOffset,
  trackAnimating,
  boundaryLeaving,
  turnTo,
  goToPage,
  previousPage,
  nextPage,
  snapBack,
  cancelTurn,
  resetTrack,
  resetBoundary,
  stageSpan,
} = usePageTurn({
  page: () => props.page,
  total,
  direction: () => props.direction,
  horizontal,
  stage,
  onPageChange: (page) => emit("update:page", page),
  onBoundary: (direction) => emit("page-boundary", direction),
  onSettled: (page) => {
    reset();
    imageSize.value = pageSizes.get(page) ?? null;
    reflow();
  },
  isZoomed: () => scale.value > 1,
  resetZoom: reset,
});

const currentPage = computed(
  () => props.pages[displayPage.value - 1] ?? null,
);
const counterPage = computed(() => turnTarget.value ?? displayPage.value);
// At a chapter boundary the buttons stay enabled: the page-turn attempt is
// what switches chapter (or returns to the detail page).
const canPrevious = computed(
  () => !trackAnimating.value && !boundaryLeaving.value,
);
const canNext = computed(
  () => !trackAnimating.value && !boundaryLeaving.value,
);

const imageStyle = computed<CSSProperties>(() => ({
  transform: `translate3d(${offset.value.x}px, ${offset.value.y}px, 0) scale(${scale.value})`,
}));

const trackStyle = computed<CSSProperties>(() => ({
  // The track is three viewport-spans long. Adjacent pages are placed directly
  // before and after the center page, so a turn never exposes a background gap.
  transform: horizontal.value
    ? `translate3d(calc(-33.333333% + ${trackOffset.value}px), 0, 0)`
    : `translate3d(0, calc(-33.333333% + ${trackOffset.value}px), 0)`,
  transition: trackAnimating.value
    ? `transform ${TURN_DURATION_MS}ms cubic-bezier(0.2, 0.8, 0.2, 1)`
    : "none",
}));

useReaderPreload(
  computed(() => props.platform),
  computed(() => props.pages),
  displayPage,
  computed(() => props.preloadCount),
);

function zoomIn() {
  zoomStep(1);
}

function zoomOut() {
  zoomStep(-1);
}

function resetZoom() {
  reset();
}

function onSlotSize(number: number, size: ReaderImageSize) {
  pageSizes.set(number, size);
  if (number === displayPage.value) {
    imageSize.value = size;
    reflow();
  }
}

watch(
  () => props.pages,
  () => {
    // Chapter or page-source changes invalidate cached sizes for old pages.
    resetBoundary();
    pageSizes.clear();
  },
);

watch(
  () => props.direction,
  () => {
    // A direction change swaps the track axis and navigation mapping; do not
    // carry a half-finished turn across the layout change.
    cancelTurn();
    reset();
  },
);

// The stage is split into three tap zones inside useReaderGestures: the middle
// one toggles the overlay chrome, while the sides turn pages.
function toggleChrome() {
  chromeVisible.value = !chromeVisible.value;
}

const {
  onStagePointerDown,
  onStagePointerMove,
  onStagePointerUp,
  onStagePointerCancel,
} = useReaderGestures({
  stage,
  isAnimating: () => trackAnimating.value,
  direction: () => props.direction,
  horizontal,
  total,
  displayPage,
  trackOffset,
  stageSpan,
  scale,
  panBy,
  setScale,
  cancelTurn,
  resetTrack,
  snapBack,
  turnTo,
  nextPage,
  previousPage,
  toggleChrome,
});

function onFullscreenChange() {
  isFullscreen.value = document.fullscreenElement === root.value;
}

async function toggleFullscreen() {
  try {
    if (document.fullscreenElement) {
      await document.exitFullscreen();
    } else if (root.value?.requestFullscreen) {
      await root.value.requestFullscreen();
    }
  } catch {
    // Fullscreen is optional; a rejected request must not break the reader.
  }
}

const { onWheel } = useReaderWheel({
  stage,
  isAnimating: () => trackAnimating.value,
  nextPage,
  previousPage,
  zoomBy,
});

const { onKeydown } = useReaderKeyboard({
  direction: () => props.direction,
  total: () => total.value,
  isAnimating: () => trackAnimating.value,
  nextPage,
  previousPage,
  goToPage,
  zoomIn,
  zoomOut,
  resetZoom,
  toggleFullscreen,
  close: () => emit("close"),
});

function onResize() {
  if (!trackAnimating.value) {
    resetTrack();
  }
  reflow();
}

onMounted(() => {
  stage.value?.addEventListener("wheel", onWheel, { passive: false });
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("resize", onResize);
  document.addEventListener("fullscreenchange", onFullscreenChange);
});

onBeforeUnmount(() => {
  stage.value?.removeEventListener("wheel", onWheel);
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("resize", onResize);
  document.removeEventListener("fullscreenchange", onFullscreenChange);
  cancelTurn();
  if (document.fullscreenElement === root.value) {
    void document.exitFullscreen().catch(() => undefined);
  }
});
</script>

<template>
  <section ref="root" class="viewer" :aria-label="title">
    <ReaderChrome
      :chrome-visible="chromeVisible"
      :title="title"
      :chapter-title="chapterTitle"
      :counter-page="counterPage"
      :total="total"
      :can-previous="canPrevious"
      :can-next="canNext"
      :has-previous-chapter="hasPreviousChapter"
      :has-next-chapter="hasNextChapter"
      :track-animating="trackAnimating"
      :is-fullscreen="isFullscreen"
      :percent="percent"
      @close="emit('close')"
      @previous-page="previousPage"
      @next-page="nextPage"
      @previous-chapter="emit('previous-chapter')"
      @next-chapter="emit('next-chapter')"
      @zoom-in="zoomIn"
      @zoom-out="zoomOut"
      @reset-zoom="resetZoom"
      @toggle-fullscreen="toggleFullscreen"
    />

    <div
      ref="stage"
      class="viewer__stage"
      @pointerdown="onStagePointerDown"
      @pointermove="onStagePointerMove"
      @pointerup="onStagePointerUp"
      @pointercancel="onStagePointerCancel"
      @dragstart.prevent
    >
      <ReaderTrack
        :pages="pages"
        :platform="platform"
        :direction="direction"
        :display-page="displayPage"
        :total="total"
        :track-style="trackStyle"
        :image-style="imageStyle"
        :identity-style="identityStyle"
        @size="onSlotSize"
      />
      <div v-if="!currentPage" class="viewer__empty">
        {{ t("reader.empty") }}
      </div>
    </div>
  </section>
</template>

<style scoped>
.viewer {
  position: relative;
  display: flex;
  flex-direction: column;
  width: 100%;
  height: 100%;
  overflow: hidden;
  background: var(--app-background);
}

.viewer:fullscreen {
  width: 100vw;
  height: 100vh;
}

.viewer__stage {
  position: relative;
  flex: 1;
  min-height: 0;
  overflow: hidden;
  background: var(--app-background);
  touch-action: none;
  user-select: none;
}

.viewer__empty {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 1.5rem;
  text-align: center;
  opacity: 0.7;
}
</style>
