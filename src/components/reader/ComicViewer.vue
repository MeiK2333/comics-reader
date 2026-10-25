<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { CSSProperties } from "vue";
import type { ComicPlatform } from "../../api/comics";
import type { ReaderPage } from "../../api/reader";
import { useReaderZoom } from "../../composables/useReaderZoom";
import type { ReaderImageSize } from "../../composables/useReaderZoom";
import { useAppI18n } from "../../i18n";
import type { ReaderDirection } from "../../stores/settings";
import ReaderPageImage from "./ReaderPageImage.vue";

const TURN_DURATION = 260;
const REDUCED_MOTION_QUERY = "(prefers-reduced-motion: reduce)";

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
  }>(),
  {
    chapterTitle: "",
    hasPreviousChapter: false,
    hasNextChapter: false,
  },
);

const emit = defineEmits<{
  "update:page": [page: number];
  "previous-chapter": [];
  "next-chapter": [];
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
const chromeVisible = ref(true);
const chromeHover = ref(false);
const isFullscreen = ref(false);

const total = computed(() => props.pages.length);
const horizontal = computed(() => props.direction !== "ttb");
const displayPage = ref(1);
const turnTarget = ref<number | null>(null);
const trackOffset = ref(0);
const trackAnimating = ref(false);
const identityStyle: CSSProperties = {};

const currentPage = computed(
  () => props.pages[displayPage.value - 1] ?? null,
);
const counterPage = computed(() => turnTarget.value ?? displayPage.value);
const canPrevious = computed(
  () => !trackAnimating.value && displayPage.value > 1,
);
const canNext = computed(
  () => !trackAnimating.value && displayPage.value < total.value,
);

const {
  scale,
  offset,
  percent,
  zoomBy,
  zoomStep,
  panBy,
  reset,
  reflow,
} = useReaderZoom(stage, imageSize);

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
    ? `transform ${TURN_DURATION}ms cubic-bezier(0.2, 0.8, 0.2, 1)`
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

function clampPage(value: number) {
  const last = Math.max(total.value, 1);
  return Math.min(Math.max(value, 1), last);
}

displayPage.value = clampPage(props.page);

function stageHeight() {
  return stage.value?.clientHeight ?? 1;
}

function stageSpan() {
  return horizontal.value
    ? (stage.value?.clientWidth ?? 1)
    : stageHeight();
}

function prefersReducedMotion() {
  return window.matchMedia(REDUCED_MOTION_QUERY).matches;
}

let turnTimer: ReturnType<typeof setTimeout> | undefined;
let turnToken = 0;

function clearTurnTimer() {
  if (turnTimer) {
    clearTimeout(turnTimer);
    turnTimer = undefined;
  }
}

function cancelTurn() {
  turnToken += 1;
  clearTurnTimer();
  trackAnimating.value = false;
  trackOffset.value = 0;
  turnTarget.value = null;
}

function finishTurn(target: number | null) {
  clearTurnTimer();
  trackAnimating.value = false;
  trackOffset.value = 0;
  turnTarget.value = null;

  if (target !== null && target !== displayPage.value) {
    displayPage.value = target;
    emit("update:page", target);
  }
}

function animateTrackTo(targetOffset: number, done: () => void) {
  const token = ++turnToken;
  clearTurnTimer();
  trackAnimating.value = true;

  // The transition has to be active before the transform changes, otherwise
  // drag-release would jump straight to the destination.
  requestAnimationFrame(() => {
    if (token !== turnToken) {
      return;
    }

    trackOffset.value = targetOffset;
    if (prefersReducedMotion()) {
      done();
      return;
    }

    turnTimer = setTimeout(() => {
      if (token === turnToken) {
        done();
      }
    }, TURN_DURATION + 20);
  });
}

function snapBack() {
  if (trackOffset.value === 0) {
    trackAnimating.value = false;
    return;
  }
  turnTarget.value = null;
  animateTrackTo(0, () => finishTurn(null));
}

function turnTo(target: number) {
  if (trackAnimating.value) {
    return;
  }

  const next = clampPage(target);
  if (next === displayPage.value) {
    return;
  }

  // Home/End and chapter jumps skip more than one page; the three-slot track
  // has no intermediate page to show, so jump without faking a one-page turn.
  if (Math.abs(next - displayPage.value) > 1) {
    cancelTurn();
    displayPage.value = next;
    emit("update:page", next);
    showChrome();
    return;
  }

  // A zoomed page is no longer part of the contiguous track; reset before the
  // turn so the two adjacent pages are shown at the same scale.
  if (scale.value > 1) {
    reset();
  }

  turnTarget.value = next;
  const forward = next > displayPage.value;
  const span = stageSpan();
  const targetOffset =
    props.direction === "rtl"
      ? (forward ? span : -span)
      : (forward ? -span : span);
  animateTrackTo(targetOffset, () => finishTurn(next));
  showChrome();
}

function goToPage(next: number) {
  turnTo(next);
}

function previousPage() {
  goToPage(displayPage.value - 1);
}

function nextPage() {
  goToPage(displayPage.value + 1);
}

function zoomIn() {
  zoomStep(1);
  showChrome();
}

function zoomOut() {
  zoomStep(-1);
  showChrome();
}

function resetZoom() {
  reset();
  showChrome();
}

function onSlotSize(number: number, size: ReaderImageSize) {
  pageSizes.set(number, size);
  if (number === displayPage.value) {
    imageSize.value = size;
    reflow();
  }
}

watch(
  () => props.page,
  (value) => {
    const next = clampPage(value);
    if (next === displayPage.value) {
      return;
    }
    cancelTurn();
    displayPage.value = next;
  },
);

watch(
  [displayPage, total],
  () => {
    reset();
    imageSize.value = pageSizes.get(displayPage.value) ?? null;
    trackOffset.value = 0;
    reflow();
  },
  { immediate: true },
);

watch(
  () => props.pages,
  () => {
    // Chapter or page-source changes invalidate cached sizes for old pages.
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

let hideTimer: ReturnType<typeof setTimeout> | undefined;

function clearHideTimer() {
  if (hideTimer) {
    clearTimeout(hideTimer);
    hideTimer = undefined;
  }
}

function scheduleHide() {
  clearHideTimer();
  if (chromeHover.value) {
    return;
  }
  hideTimer = setTimeout(() => {
    chromeVisible.value = false;
  }, 2600);
}

function showChrome() {
  chromeVisible.value = true;
  scheduleHide();
}

function holdChrome() {
  chromeHover.value = true;
  clearHideTimer();
}

function releaseChrome() {
  chromeHover.value = false;
  scheduleHide();
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

let drag: DragState | null = null;

function isInteractiveTarget(target: EventTarget | null) {
  return (
    target instanceof Element &&
    target.closest("button, a, input, textarea, select, [contenteditable='true']") !==
      null
  );
}

function onStagePointerDown(event: PointerEvent) {
  if (trackAnimating.value) {
    return;
  }
  if (event.pointerType === "mouse" && event.button !== 0) {
    return;
  }
  if (drag || isInteractiveTarget(event.target)) {
    return;
  }

  const element = stage.value;
  if (!element) {
    return;
  }

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
  element.setPointerCapture(event.pointerId);
  showChrome();
  event.preventDefault();
}

function onStagePointerMove(event: PointerEvent) {
  showChrome();
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
  if (!drag || drag.id !== event.pointerId) {
    return;
  }

  const current = drag;
  drag = null;
  releasePointer(event);

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
    }
    snapBack();
    return;
  }

  if (!current.moved) {
    trackOffset.value = 0;
    showChrome();
    return;
  }

  snapBack();
}

function onStagePointerCancel(event: PointerEvent) {
  if (!drag || drag.id !== event.pointerId) {
    return;
  }
  drag = null;
  releasePointer(event);
  snapBack();
}

let wheelAccumulator = 0;
let lastWheelAt = 0;
let lastNavigationAt = 0;

function onWheel(event: WheelEvent) {
  event.preventDefault();
  showChrome();

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
    trackOffset.value = 0;
  }
  reflow();
}

onMounted(() => {
  stage.value?.addEventListener("wheel", onWheel, { passive: false });
  window.addEventListener("keydown", onKeydown);
  window.addEventListener("resize", onResize);
  document.addEventListener("fullscreenchange", onFullscreenChange);
  showChrome();
});

onBeforeUnmount(() => {
  stage.value?.removeEventListener("wheel", onWheel);
  window.removeEventListener("keydown", onKeydown);
  window.removeEventListener("resize", onResize);
  document.removeEventListener("fullscreenchange", onFullscreenChange);
  cancelTurn();
  clearHideTimer();
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
      @pointerenter="holdChrome"
      @pointerleave="releaseChrome"
      @focusin="holdChrome"
      @focusout="releaseChrome"
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
          :key="slot ? `${slot.number}:${slot.page.url}` : `empty:${index}`"
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
      @pointerenter="holdChrome"
      @pointerleave="releaseChrome"
      @focusin="holdChrome"
      @focusout="releaseChrome"
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

    <div class="viewer__safe-bottom" aria-hidden="true"></div>

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
  position: relative;
  z-index: 3;
  display: flex;
  flex: none;
  align-items: center;
  gap: 0.75rem;
  padding: 0.4rem 0.75rem;
  background: var(--app-background);
  transition:
    opacity 180ms ease,
    transform 180ms ease;
}

.viewer__bar--top {
  min-height: 56px;
  border-bottom: 1px solid rgba(127, 127, 127, 0.12);
}

.viewer__bar--bottom {
  min-height: 60px;
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

.viewer__safe-bottom {
  flex: none;
  height: env(safe-area-inset-bottom, 0px);
  background: var(--app-background);
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
