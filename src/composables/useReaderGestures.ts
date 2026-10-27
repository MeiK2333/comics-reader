import { onScopeDispose } from "vue";
import type { ComputedRef, Ref } from "vue";
import {
  atPageBoundary,
  dragLimit,
  isForwardGesture,
  swipeTarget,
  tapTurnsForward,
  tapZone,
} from "../lib/reader-gestures";
import type { TapSide } from "../lib/reader-gestures";
import type { ReaderDirection } from "../stores/settings";

const MIN_MOVE_PX = 4;
const SWIPE_MIN_PX = 48;
const SWIPE_SPAN_RATIO = 0.12;

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

export interface ReaderGestureOptions {
  stage: Ref<HTMLElement | null>;
  isAnimating: () => boolean;
  direction: () => ReaderDirection;
  horizontal: ComputedRef<boolean>;
  total: ComputedRef<number>;
  displayPage: Ref<number>;
  trackOffset: Ref<number>;
  stageSpan: () => number;
  scale: Ref<number>;
  panBy: (dx: number, dy: number) => void;
  setScale: (scale: number, clientX?: number, clientY?: number) => void;
  cancelTurn: () => void;
  resetTrack: () => void;
  snapBack: () => void;
  turnTo: (page: number) => void;
  nextPage: () => void;
  previousPage: () => void;
  toggleChrome: () => void;
}

function isInteractiveTarget(target: EventTarget | null) {
  return (
    target instanceof Element &&
    target.closest(
      "button, a, input, textarea, select, [contenteditable='true']",
    ) !== null
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

/**
 * Pointer gestures for the reader stage: horizontal/vertical swipes turn
 * pages, a drag panned while zoomed, two touch pointers pinch-zoom, and a tap
 * in the left/right third turns a page while the middle third toggles chrome.
 *
 * The caller owns the stage element and the page-turn state; this hook only
 * translates pointer events into those operations.
 */
export function useReaderGestures(options: ReaderGestureOptions) {
  let drag: DragState | null = null;
  let pinch: PinchState | null = null;
  const activePointers = new Map<number, PointerPoint>();

  function startPinch() {
    const points = [...activePointers.values()];
    if (
      points.length !== 2 ||
      !points.every((point) => point.type === "touch")
    ) {
      return;
    }

    const [first, second] = points;
    const distance = pointerDistance(first, second);
    if (distance <= 0) {
      return;
    }

    const midpoint = pointerMidpoint(first, second);
    drag = null;
    options.cancelTurn();
    options.resetTrack();
    pinch = {
      startDistance: distance,
      startScale: options.scale.value,
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
    if (options.isAnimating()) {
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

    const element = options.stage.value;
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
        mode: options.scale.value > 1 ? "pan" : "swipe",
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
    if (options.isAnimating()) {
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
      options.panBy(midpoint.x - pinch.lastX, midpoint.y - pinch.lastY);
      options.setScale(
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

    if (
      Math.abs(drag.totalX) > MIN_MOVE_PX ||
      Math.abs(drag.totalY) > MIN_MOVE_PX
    ) {
      drag.moved = true;
    }

    if (drag.mode === "pan") {
      options.panBy(dx, dy);
      return;
    }

    if (options.total.value > 1) {
      const primary = options.horizontal.value ? drag.totalX : drag.totalY;
      const forward = isForwardGesture(
        options.direction(),
        options.horizontal.value,
        primary,
      );
      const limit = dragLimit(
        options.stageSpan(),
        atPageBoundary(
          options.displayPage.value,
          options.total.value,
          forward,
        ),
      );
      options.trackOffset.value = Math.max(-limit, Math.min(limit, primary));
    }
  }

  function releasePointer(event: PointerEvent) {
    const element = options.stage.value;
    if (element?.hasPointerCapture(event.pointerId)) {
      element.releasePointerCapture(event.pointerId);
    }
  }

  function turnFromTap(side: TapSide) {
    if (tapTurnsForward(options.direction(), side)) {
      options.nextPage();
    } else {
      options.previousPage();
    }
  }

  function onStageTap(event: PointerEvent) {
    const element = options.stage.value;
    if (!element) {
      return;
    }

    const rect = element.getBoundingClientRect();
    if (rect.width === 0) {
      return;
    }

    const zone = tapZone((event.clientX - rect.left) / rect.width);
    if (zone === "middle") {
      options.toggleChrome();
      return;
    }
    turnFromTap(zone);
  }

  function finishPinch() {
    pinch = null;
    const remaining = activePointers.entries().next();
    if (!remaining.done) {
      const [pointerId, point] = remaining.value;
      startPan(pointerId, point);
    }
  }

  function onStagePointerUp(event: PointerEvent) {
    if (!activePointers.delete(event.pointerId)) {
      return;
    }
    releasePointer(event);

    if (options.isAnimating()) {
      drag = null;
      pinch = null;
      return;
    }

    if (pinch) {
      if (activePointers.size >= 2) {
        return;
      }
      finishPinch();
      return;
    }

    if (!drag || drag.id !== event.pointerId) {
      return;
    }

    const current = drag;
    drag = null;

    if (current.mode === "swipe" && current.moved) {
      const primary = options.horizontal.value
        ? current.totalX
        : current.totalY;
      const threshold = Math.max(SWIPE_MIN_PX, options.stageSpan() * SWIPE_SPAN_RATIO);

      if (Math.abs(primary) >= threshold) {
        const forward = isForwardGesture(
          options.direction(),
          options.horizontal.value,
          primary,
        );
        const target = swipeTarget(
          options.displayPage.value,
          options.total.value,
          forward,
        );
        if (target !== null) {
          options.turnTo(target);
          return;
        }
        if (forward) {
          options.nextPage();
        } else {
          options.previousPage();
        }
        return;
      }
      options.snapBack();
      return;
    }

    if (!current.moved) {
      options.resetTrack();
      onStageTap(event);
      return;
    }

    options.snapBack();
  }

  function onStagePointerCancel(event: PointerEvent) {
    if (!activePointers.delete(event.pointerId)) {
      return;
    }
    releasePointer(event);

    if (options.isAnimating()) {
      drag = null;
      pinch = null;
      return;
    }

    if (pinch) {
      if (activePointers.size >= 2) {
        return;
      }
      finishPinch();
      return;
    }

    if (!drag || drag.id !== event.pointerId) {
      return;
    }
    drag = null;
    options.snapBack();
  }

  onScopeDispose(() => {
    activePointers.clear();
    drag = null;
    pinch = null;
  });

  return {
    onStagePointerDown,
    onStagePointerMove,
    onStagePointerUp,
    onStagePointerCancel,
  };
}
