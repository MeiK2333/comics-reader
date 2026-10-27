<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { CSSProperties } from "vue";
import type { ComicPlatform } from "../../api/comics";
import type { ReaderPage } from "../../api/reader";
import { useBattery } from "../../composables/useBattery";
import { usePageTurn, TURN_DURATION_MS } from "../../composables/usePageTurn";
import { useReaderPreload } from "../../composables/useReaderPreload";
import { useReaderZoom } from "../../composables/useReaderZoom";
import type { ReaderImageSize } from "../../composables/useReaderZoom";
import { useAppI18n } from "../../i18n";
import type { ReaderDirection } from "../../stores/settings";
import ReaderBattery from "./ReaderBattery.vue";
import ReaderPageImage from "./ReaderPageImage.vue";

const NEXT_KEYS: Record<ReaderDirection, readonly string[]> = {
  ltr: ["ArrowRight", "ArrowDown"],
  rtl: ["ArrowLeft", "ArrowDown"],
  ttb: ["ArrowDown", "ArrowRight"],
};

const PREVIOUS_KEYS: Record<ReaderDirection, readonly string[]> = {
  ltr: ["ArrowLeft", "ArrowUp"],
  rtl: ["ArrowRight", "ArrowUp"],
  ttb: ["ArrowUp", "ArrowLeft"],
};

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

interface PageSlot {
  page: ReaderPage;
  number: number;
}

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

const { percent: batteryPercent } = useBattery();
const batteryLabel = computed(() =>
  batteryPercent.value === null
    ? ""
    : t("reader.batteryLevel", { level: batteryPercent.value }),
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

const pageSlots = computed<(PageSlot | null)[]>(() => {
  const slots: (PageSlot | null)[] = [];
  for (
    let number = displayPage.value - 1;
    number <= displayPage.value + 1;
    number += 1
  ) {
    const page = props.pages[number - 1];
    slots.push(page ? { page, number } : null);
  }
  return slots;
});

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

let wheelAccumulator = 0;
let lastWheelAt = 0;
let lastNavigationAt = 0;

function onWheel(event: WheelEvent) {
  event.preventDefault();

  if (trackAnimating.value) {
    wheelAccumulator = 0;
    return;
  }

  if (event.ctrlKey) {
    zoomBy(Math.exp(-event.deltaY * 0.002), event.clientX, event.clientY);
    wheelAccumulator = 0;
    return;
  }

  const modeScale =
    event.deltaMode === 1
      ? 16
      : event.deltaMode === 2
        ? (stage.value?.clientHeight ?? 600)
        : 1;
  const delta = event.deltaY * modeScale;
  if (delta === 0) {
    return;
  }

  const now = performance.now();
  if (
    now - lastWheelAt > 220 ||
    Math.sign(delta) !== Math.sign(wheelAccumulator)
  ) {
    wheelAccumulator = 0;
  }
  wheelAccumulator += delta;
  lastWheelAt = now;

  if (Math.abs(wheelAccumulator) >= 60 && now - lastNavigationAt >= 280) {
    wheelAccumulator = 0;
    lastNavigationAt = now;
    if (delta > 0) {
      nextPage();
    } else {
      previousPage();
    }
  }
}

function isEditableTarget(target: EventTarget | null) {
  return (
    target instanceof Element &&
    target.closest(
      "input, textarea, select, [contenteditable='true']",
    ) !== null
  );
}

function onKeydown(event: KeyboardEvent) {
  if (
    event.defaultPrevented ||
    isEditableTarget(event.target) ||
    event.ctrlKey ||
    event.metaKey ||
    event.altKey
  ) {
    return;
  }

  if (trackAnimating.value && event.key !== "Escape") {
    return;
  }

  if (NEXT_KEYS[props.direction].includes(event.key)) {
    event.preventDefault();
    nextPage();
    return;
  }
  if (PREVIOUS_KEYS[props.direction].includes(event.key)) {
    event.preventDefault();
    previousPage();
    return;
  }

  switch (event.key) {
    case "Home":
      event.preventDefault();
      goToPage(1);
      break;
    case "End":
      event.preventDefault();
      goToPage(total.value);
      break;
    case "+":
    case "=":
      event.preventDefault();
      zoomIn();
      break;
    case "-":
    case "_":
      event.preventDefault();
      zoomOut();
      break;
    case "0":
      event.preventDefault();
      resetZoom();
      break;
    case "f":
    case "F":
      event.preventDefault();
      void toggleFullscreen();
      break;
    case "Escape":
      // Let the browser leave fullscreen first; the next Escape closes.
      if (document.fullscreenElement) {
        return;
      }
      event.preventDefault();
      emit("close");
      break;
    default:
      break;
  }
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
  <section
    ref="root"
    class="viewer"
    :class="{ 'viewer--chrome-hidden': !chromeVisible }"
    :aria-label="title"
  >
    <header
      class="viewer__bar viewer__bar--top"
      :aria-hidden="!chromeVisible"
      :inert="!chromeVisible || undefined"
    >
      <NButton
        attr-type="button"
        quaternary
        circle
        size="large"
        :aria-label="t('reader.back')"
        :title="t('reader.back')"
        @click="emit('close')"
      >
        <template #icon>
          <svg
            class="viewer__icon"
            viewBox="0 0 24 24"
            aria-hidden="true"
            focusable="false"
          >
            <path
              d="M15 5l-7 7 7 7"
              fill="none"
              stroke="currentColor"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </template>
      </NButton>

      <div class="viewer__heading">
        <span class="viewer__title">{{ title }}</span>
        <span v-if="chapterTitle" class="viewer__chapter">
          {{ chapterTitle }}
        </span>
      </div>

      <ReaderBattery
        v-if="batteryPercent !== null"
        :level="batteryPercent"
        :label="batteryLabel"
      />

      <span class="viewer__counter">
        {{ t("reader.pageIndicator", { current: counterPage, total }) }}
      </span>
    </header>

    <div
      ref="stage"
      class="viewer__stage"
      @pointerdown="onStagePointerDown"
      @pointermove="onStagePointerMove"
      @pointerup="onStagePointerUp"
      @pointercancel="onStagePointerCancel"
      @dragstart.prevent
    >
      <div
        class="viewer__track"
        :class="`viewer__track--${direction}`"
        :style="trackStyle"
      >
        <div
          v-for="(slot, index) in pageSlots"
          :key="
            slot
              ? `${slot.number}:${slot.page.localPath ?? slot.page.url}`
              : `empty:${index}`
          "
          class="viewer__slot"
          :class="{ 'viewer__slot--empty': slot === null }"
          :aria-hidden="slot?.number !== displayPage"
        >
          <ReaderPageImage
            v-if="slot"
            :platform="platform"
            :page="slot.page"
            :page-number="slot.number"
            :total="total"
            :image-style="
              slot.number === displayPage ? imageStyle : identityStyle
            "
            @size="onSlotSize(slot.number, $event)"
          />
        </div>
      </div>
      <div v-if="!currentPage" class="viewer__empty">
        {{ t("reader.empty") }}
      </div>
    </div>

    <footer
      class="viewer__bar viewer__bar--bottom"
      :aria-hidden="!chromeVisible"
      :inert="!chromeVisible || undefined"
    >
      <div class="viewer__nav">
        <NButton
          attr-type="button"
          quaternary
          circle
          :disabled="!canPrevious"
          :aria-label="t('reader.previousPage')"
          :title="t('reader.previousPage')"
          @click="previousPage"
        >
          <template #icon>
            <svg
              class="viewer__icon"
              viewBox="0 0 24 24"
              aria-hidden="true"
              focusable="false"
            >
              <path
                d="M15 5l-7 7 7 7"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </template>
        </NButton>

        <span class="viewer__counter viewer__counter--bottom">
          {{ t("reader.pageIndicator", { current: counterPage, total }) }}
        </span>

        <NButton
          attr-type="button"
          quaternary
          circle
          :disabled="!canNext"
          :aria-label="t('reader.nextPage')"
          :title="t('reader.nextPage')"
          @click="nextPage"
        >
          <template #icon>
            <svg
              class="viewer__icon"
              viewBox="0 0 24 24"
              aria-hidden="true"
              focusable="false"
            >
              <path
                d="M9 5l7 7-7 7"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </template>
        </NButton>

        <NButton
          v-if="hasPreviousChapter"
          attr-type="button"
          quaternary
          size="small"
          @click="emit('previous-chapter')"
        >
          {{ t("reader.previousChapter") }}
        </NButton>
        <NButton
          v-if="hasNextChapter"
          attr-type="button"
          quaternary
          size="small"
          @click="emit('next-chapter')"
        >
          {{ t("reader.nextChapter") }}
        </NButton>
      </div>

      <div class="viewer__tools">
        <NButton
          attr-type="button"
          quaternary
          circle
          :disabled="trackAnimating"
          :aria-label="t('reader.zoomOut')"
          :title="t('reader.zoomOut')"
          @click="zoomOut"
        >
          <template #icon>
            <span aria-hidden="true">−</span>
          </template>
        </NButton>
        <span class="viewer__zoom">{{ percent }}%</span>
        <NButton
          attr-type="button"
          quaternary
          circle
          :disabled="trackAnimating"
          :aria-label="t('reader.zoomIn')"
          :title="t('reader.zoomIn')"
          @click="zoomIn"
        >
          <template #icon>
            <span aria-hidden="true">+</span>
          </template>
        </NButton>
        <NButton
          attr-type="button"
          quaternary
          size="small"
          :disabled="trackAnimating"
          :aria-label="t('reader.resetZoom')"
          :title="t('reader.resetZoom')"
          @click="resetZoom"
        >
          1:1
        </NButton>
        <NButton
          attr-type="button"
          quaternary
          circle
          :aria-label="
            isFullscreen
              ? t('reader.exitFullscreen')
              : t('reader.fullscreen')
          "
          :title="
            isFullscreen
              ? t('reader.exitFullscreen')
              : t('reader.fullscreen')
          "
          @click="toggleFullscreen"
        >
          <template #icon>
            <svg
              class="viewer__icon"
              viewBox="0 0 24 24"
              aria-hidden="true"
              focusable="false"
            >
              <path
                d="M8 3H3v5M16 3h5v5M8 21H3v-5M16 21h5v-5"
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
              />
            </svg>
          </template>
        </NButton>
      </div>
    </footer>

    <div v-show="!chromeVisible" class="viewer__hud" aria-hidden="true">
      <ReaderBattery
        v-if="batteryPercent !== null"
        :level="batteryPercent"
        :label="batteryLabel"
      />
      <span class="viewer__hud-page">
        {{ t("reader.pageIndicator", { current: counterPage, total }) }}
      </span>
    </div>

    <p class="sr-only" aria-live="polite">
      {{
        t("reader.pageAnnouncement", {
          current: counterPage,
          total,
        })
      }}
    </p>
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

.viewer__bar {
  position: absolute;
  right: 0;
  left: 0;
  z-index: 3;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 0.4rem 0.75rem;
  background: var(--app-background);
  transition:
    opacity 180ms ease,
    transform 180ms ease;
}

.viewer__bar--top {
  top: 0;
  min-height: 56px;
  border-bottom: 1px solid rgba(127, 127, 127, 0.12);
}

.viewer__bar--bottom {
  bottom: 0;
  min-height: 60px;
  padding-bottom: calc(0.4rem + env(safe-area-inset-bottom, 0px));
  border-top: 1px solid rgba(127, 127, 127, 0.12);
}

.viewer--chrome-hidden .viewer__bar {
  opacity: 0;
  pointer-events: none;
}

.viewer--chrome-hidden .viewer__bar--top {
  transform: translateY(-100%);
}

.viewer--chrome-hidden .viewer__bar--bottom {
  transform: translateY(100%);
}

.viewer__icon {
  display: block;
  width: 20px;
  height: 20px;
}

.viewer__heading {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  line-height: 1.3;
}

.viewer__title,
.viewer__chapter {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.viewer__title {
  font-size: 0.95rem;
  font-weight: 600;
}

.viewer__chapter {
  font-size: 0.78rem;
  opacity: 0.65;
}

.viewer__counter {
  flex: none;
  font-size: 0.85rem;
  font-variant-numeric: tabular-nums;
  opacity: 0.7;
}

.viewer__hud {
  position: absolute;
  top: 0.5rem;
  right: calc(0.75rem + var(--app-safe-area-right, 0px));
  z-index: 2;
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.3rem 0.6rem;
  border: 1px solid rgba(127, 127, 127, 0.18);
  border-radius: 999px;
  background: var(--app-background);
  background: color-mix(in srgb, var(--app-background) 82%, transparent);
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.12);
  backdrop-filter: blur(8px);
  font-size: 0.82rem;
  font-variant-numeric: tabular-nums;
  pointer-events: none;
}

.viewer__hud-page {
  opacity: 0.82;
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

.viewer__track {
  position: absolute;
  top: 0;
  left: 0;
  display: flex;
  will-change: transform;
}

.viewer__track--ltr,
.viewer__track--rtl {
  width: 300%;
  height: 100%;
  flex-direction: row;
}

.viewer__track--rtl {
  flex-direction: row-reverse;
}

.viewer__track--ttb {
  width: 100%;
  height: 300%;
  flex-direction: column;
}

.viewer__slot {
  position: relative;
  flex: 0 0 33.333333%;
  overflow: hidden;
}

.viewer__track--ltr .viewer__slot,
.viewer__track--rtl .viewer__slot {
  width: 33.333333%;
  height: 100%;
}

.viewer__track--ttb .viewer__slot {
  width: 100%;
  height: 33.333333%;
}

.viewer__slot--empty {
  background: var(--app-background);
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

.viewer__nav,
.viewer__tools {
  display: flex;
  align-items: center;
  gap: 0.3rem;
}

.viewer__nav {
  flex: 1;
  justify-content: flex-start;
}

.viewer__tools {
  flex: none;
  justify-content: flex-end;
}

.viewer__counter--bottom {
  min-width: 4.5rem;
  text-align: center;
}

.viewer__zoom {
  min-width: 3.1rem;
  text-align: center;
  font-size: 0.85rem;
  font-variant-numeric: tabular-nums;
  opacity: 0.7;
}

.sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
}

@media (max-width: 620px) {
  .viewer__bar {
    padding-inline: 0.45rem;
  }

  .viewer__title {
    font-size: 0.88rem;
  }

  .viewer__chapter {
    font-size: 0.72rem;
  }

  .viewer__counter {
    font-size: 0.78rem;
  }

  .viewer__tools {
    gap: 0.1rem;
  }

  .viewer__zoom {
    min-width: 2.6rem;
  }
}

@media (max-width: 430px) {
  .viewer__bar--bottom {
    flex-wrap: wrap;
    row-gap: 0.15rem;
  }

  .viewer__nav,
  .viewer__tools {
    flex: 1 1 100%;
    justify-content: center;
  }
}
</style>
