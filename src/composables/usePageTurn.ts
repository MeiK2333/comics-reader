import { onScopeDispose, ref, watch } from "vue";
import type { ComputedRef, Ref } from "vue";
import type { ReaderDirection } from "../stores/settings";

export const TURN_DURATION_MS = 260;
const REDUCED_MOTION_QUERY = "(prefers-reduced-motion: reduce)";

export interface PageTurnOptions {
  /** The v-model page prop. Read through a getter so the hook does not own it. */
  page: () => number;
  total: ComputedRef<number>;
  direction: () => ReaderDirection;
  horizontal: ComputedRef<boolean>;
  stage: Ref<HTMLElement | null>;
  onPageChange: (page: number) => void;
  onBoundary: (direction: "previous" | "next") => void;
  /** Called whenever the settled page changes, e.g. to reset zoom and sizes. */
  onSettled?: (page: number) => void;
  isZoomed?: () => boolean;
  resetZoom?: () => void;
}

/**
 * Owns the reader's page position and the animated 3-slot track offset.
 *
 * `displayPage` is the only page value a turn writes; the prop is mirrored in
 * through `watch(page)` and reported back through `onPageChange`. Gestures,
 * wheel, keyboard and the toolbar all call the same turn methods, so they
 * cannot disagree about whether a turn is running.
 */
export function usePageTurn(options: PageTurnOptions) {
  const displayPage = ref(clampPage(options.page()));
  const turnTarget = ref<number | null>(null);
  const trackOffset = ref(0);
  const trackAnimating = ref(false);
  const boundaryLeaving = ref(false);

  let turnTimer: ReturnType<typeof setTimeout> | undefined;
  let turnToken = 0;

  function clampPage(value: number) {
    const last = Math.max(options.total.value, 1);
    return Math.min(Math.max(value, 1), last);
  }

  function stageSpan() {
    return options.horizontal.value
      ? (options.stage.value?.clientWidth ?? 1)
      : (options.stage.value?.clientHeight ?? 1);
  }

  function prefersReducedMotion() {
    return window.matchMedia(REDUCED_MOTION_QUERY).matches;
  }

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
      options.onPageChange(target);
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
      }, TURN_DURATION_MS + 20);
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
      options.onPageChange(next);
      return;
    }

    // A zoomed page is no longer part of the contiguous track; reset before the
    // turn so the two adjacent pages are shown at the same scale.
    if (options.isZoomed?.()) {
      options.resetZoom?.();
    }

    turnTarget.value = next;
    const forward = next > displayPage.value;
    const span = stageSpan();
    const targetOffset =
      options.direction() === "rtl"
        ? (forward ? span : -span)
        : (forward ? -span : span);
    animateTrackTo(targetOffset, () => finishTurn(next));
  }

  function goToPage(next: number) {
    turnTo(next);
  }

  function leaveAtBoundary(direction: "previous" | "next") {
    if (boundaryLeaving.value) {
      return;
    }
    boundaryLeaving.value = true;
    // A boundary swipe leaves the track offset near the drag limit. Clear it
    // before the parent swaps chapters so the old page cannot stay misaligned
    // during the transition.
    cancelTurn();
    options.onBoundary(direction);
  }

  function previousPage() {
    if (displayPage.value <= 1) {
      leaveAtBoundary("previous");
      return;
    }
    goToPage(displayPage.value - 1);
  }

  function nextPage() {
    if (displayPage.value >= options.total.value) {
      leaveAtBoundary("next");
      return;
    }
    goToPage(displayPage.value + 1);
  }

  function syncFromProp(value: number) {
    boundaryLeaving.value = false;
    const next = clampPage(value);
    if (next === displayPage.value) {
      return;
    }
    cancelTurn();
    displayPage.value = next;
  }

  function resetTrack() {
    trackOffset.value = 0;
  }

  function resetBoundary() {
    boundaryLeaving.value = false;
  }

  watch(options.page, syncFromProp);

  watch(
    [displayPage, options.total],
    () => {
      resetTrack();
      options.onSettled?.(displayPage.value);
    },
    { immediate: true },
  );

  onScopeDispose(() => {
    cancelTurn();
  });

  return {
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
  };
}
