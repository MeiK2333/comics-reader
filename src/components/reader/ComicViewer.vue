<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { CSSProperties } from "vue";
import type { ComicPlatform } from "../../api/comics";
import type { ReaderPage } from "../../api/reader";
import { usePageTurn, TURN_DURATION_MS } from "../../composables/usePageTurn";
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

// The stage is split into three tap zones: the middle one toggles the overlay
// chrome, while the sides turn pages. A third of the width is wide enough for
// thumbs and leaves the image itself unobstructed.
const TAP_ZONE_RATIO = 1 / 3;

function toggleChrome() {
  chromeVisible.value = !chromeVisible.value;
}

function turnFromTap(side: "left" | "right") {
  const forward =
    props.direction === "rtl" ? side === "left" : side === "right";
  if (forward) {
    nextPage();
  } else {
    previousPage();
  }
}

function onStageTap(event: PointerEvent) {
  const element = stage.value;
  if (!element) {
    return;
  }

  const rect = element.getBoundingClientRect();
  if (rect.width === 0) {
    return;
  }

  const ratio = (event.clientX - rect.left) / rect.width;
  if (ratio < TAP_ZONE_RATIO) {
    turnFromTap("left");
    return;
  }
  if (ratio > 1 - TAP_ZONE_RATIO) {
    turnFromTap("right");
    return;
  }

  toggleChrome();
}

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

interface DragState {
  id: number;
  startX: number;
  startY: number;
  lastX: number;
  lastY: number;
  totalX: number;
  totalY: number;
  mode: "swipe" | "pan";
  moved: boolean;
}

interface PointerPoint {
  x: number;
  y: number;
  type: string;
}

interface PinchState {
  startDistance: number;
  startScale: number;
  lastX: number;
  lastY: number;
}

let drag: DragState | null = null;
let pinch: PinchState | null = null;
const activePointers = new Map<number, PointerPoint>();

function isInteractiveTarget(target: EventTarget | null) {
  return (
    target instanceof Element &&
    target.closest("button, a, input, textarea, select, [contenteditable='true']") !==
      null
  );
}

function pointerDistance(first: PointerPoint, second: PointerPoint) {
  return Math.hypot(first.x - second.x, first.y - second.y);
}

function pointerMidpoint(first: PointerPoint, second: PointerPoint) {
  return {
    x: (first.x + second.x) / 2,
    y: (first.y + second.y) / 2,
  };
}

function startPinch() {
  const points = [...activePointers.values()];
  if (points.length !== 2 || !points.every((point) => point.type === "touch")) {
    return;
  }

  const [first, second] = points;
  const distance = pointerDistance(first, second);
  if (distance <= 0) {
    return;
  }

  const midpoint = pointerMidpoint(first, second);
  drag = null;
  cancelTurn();
  trackOffset.value = 0;
  pinch = {
    startDistance: distance,
    startScale: scale.value,
    lastX: midpoint.x,
    lastY: midpoint.y,
  };
}

function startPan(pointerId: number, point: PointerPoint) {
  drag = {
    id: pointerId,
    startX: point.x,
    startY: point.y,
    lastX: point.x,
    lastY: point.y,
    totalX: 0,
    totalY: 0,
    mode: "pan",
    // A pinch just ended; the remaining finger must not become a tap.
    moved: true,
  };
}

function onStagePointerDown(event: PointerEvent) {
  if (trackAnimating.value) {
    return;
  }
  if (event.pointerType === "mouse" && event.button !== 0) {
    return;
  }
  if (
    activePointers.has(event.pointerId) ||
    activePointers.size >= 2 ||
    isInteractiveTarget(event.target)
  ) {
    return;
  }

  const element = stage.value;
  if (!element) {
    return;
  }

  const point: PointerPoint = {
    x: event.clientX,
    y: event.clientY,
    type: event.pointerType,
  };
  activePointers.set(event.pointerId, point);
  element.setPointerCapture(event.pointerId);
  event.preventDefault();

  if (activePointers.size === 1) {
    drag = {
      id: event.pointerId,
      startX: event.clientX,
      startY: event.clientY,
      lastX: event.clientX,
      lastY: event.clientY,
      totalX: 0,
      totalY: 0,
      mode: scale.value > 1 ? "pan" : "swipe",
      moved: false,
    };
    return;
  }

  startPinch();
}

function onStagePointerMove(event: PointerEvent) {
  // A page-turn animation owns the track offset while it runs. Letting a
  // pointer update it here makes the two writers fight and can leave a turn
  // half-applied.
  if (trackAnimating.value) {
    return;
  }

  const point = activePointers.get(event.pointerId);
  if (!point) {
    return;
  }
  point.x = event.clientX;
  point.y = event.clientY;

  if (pinch && activePointers.size >= 2) {
    event.preventDefault();
    const [first, second] = [...activePointers.values()];
    const distance = pointerDistance(first, second);
    if (pinch.startDistance <= 0 || distance <= 0) {
      return;
    }

    const midpoint = pointerMidpoint(first, second);
    // Pan by the midpoint movement first; then anchor the new scale at the
    // moving midpoint so the image follows the fingers.
    panBy(midpoint.x - pinch.lastX, midpoint.y - pinch.lastY);
    setScale(
      pinch.startScale * (distance / pinch.startDistance),
      midpoint.x,
      midpoint.y,
    );
    pinch.lastX = midpoint.x;
    pinch.lastY = midpoint.y;
    return;
  }

  if (!drag || drag.id !== event.pointerId) {
    return;
  }

  const dx = event.clientX - drag.lastX;
  const dy = event.clientY - drag.lastY;
  drag.lastX = event.clientX;
  drag.lastY = event.clientY;
  drag.totalX = event.clientX - drag.startX;
  drag.totalY = event.clientY - drag.startY;

  if (Math.abs(drag.totalX) > 4 || Math.abs(drag.totalY) > 4) {
    drag.moved = true;
  }

  if (drag.mode === "pan") {
    panBy(dx, dy);
    return;
  }

  if (total.value > 1) {
    const primary = horizontal.value ? drag.totalX : drag.totalY;
    const wantsNext = horizontal.value
      ? (props.direction === "rtl" ? primary > 0 : primary < 0)
      : primary < 0;
    const atBoundary = wantsNext
      ? displayPage.value >= total.value
      : displayPage.value <= 1;
    const limit = stageSpan() * (atBoundary ? 0.18 : 1);
    trackOffset.value = Math.max(-limit, Math.min(limit, primary));
  }
}

function releasePointer(event: PointerEvent) {
  const element = stage.value;
  if (element?.hasPointerCapture(event.pointerId)) {
    element.releasePointerCapture(event.pointerId);
  }
}

function onStagePointerUp(event: PointerEvent) {
  if (!activePointers.delete(event.pointerId)) {
    return;
  }
  releasePointer(event);

  if (trackAnimating.value) {
    drag = null;
    pinch = null;
    return;
  }

  if (pinch) {
    if (activePointers.size >= 2) {
      return;
    }
    pinch = null;
    const remaining = activePointers.entries().next();
    if (!remaining.done) {
      const [pointerId, point] = remaining.value;
      startPan(pointerId, point);
    }
    return;
  }

  if (!drag || drag.id !== event.pointerId) {
    return;
  }

  const current = drag;
  drag = null;

  if (current.mode === "swipe" && current.moved) {
    const primary = horizontal.value ? current.totalX : current.totalY;
    const threshold = Math.max(48, stageSpan() * 0.12);

    if (Math.abs(primary) >= threshold) {
      const forward = horizontal.value
        ? (props.direction === "rtl" ? primary > 0 : primary < 0)
        : primary < 0;
      const target = displayPage.value + (forward ? 1 : -1);
      if (target >= 1 && target <= total.value) {
        turnTo(target);
        return;
      }
      if (forward) {
        nextPage();
      } else {
        previousPage();
      }
      return;
    }
    snapBack();
    return;
  }

  if (!current.moved) {
    trackOffset.value = 0;
    onStageTap(event);
    return;
  }

  snapBack();
}

function onStagePointerCancel(event: PointerEvent) {
  if (!activePointers.delete(event.pointerId)) {
    return;
  }
  releasePointer(event);

  if (trackAnimating.value) {
    drag = null;
    pinch = null;
    return;
  }

  if (pinch) {
    if (activePointers.size >= 2) {
      return;
    }
    pinch = null;
    const remaining = activePointers.entries().next();
    if (!remaining.done) {
      const [pointerId, point] = remaining.value;
      startPan(pointerId, point);
    }
    return;
  }

  if (!drag || drag.id !== event.pointerId) {
    return;
  }
  drag = null;
  snapBack();
}

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
  activePointers.clear();
  pinch = null;
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
