import { onBeforeUnmount } from "vue";

const DEFAULT_DELAY_MS = 500;
const DEFAULT_MOVE_THRESHOLD_PX = 10;
const SUPPRESS_CLICK_MS = 800;

interface LongPressOptions {
  delay?: number;
  moveThreshold?: number;
  disabled?: () => boolean;
  onLongPress: () => void;
}

/**
 * Long-press handling for a button that still needs a normal click.
 *
 * The action fires as soon as the press threshold is reached instead of
 * waiting for the release, so a desktop mouse does not have to be let go
 * first. Because the overlay it opens then sits under the pointer, the click
 * produced by the eventual release is swallowed once; otherwise releasing
 * after a long press would also run the button's normal short-press action.
 * Moving the pointer cancels a press that has not fired yet, but cannot undo
 * one that already opened the overlay.
 */
export function useLongPress(options: LongPressOptions) {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let suppressTimer: ReturnType<typeof setTimeout> | null = null;
  let startX = 0;
  let startY = 0;
  let activePointer: number | null = null;
  let fired = false;
  let suppressClick = false;

  function clearTimer() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
  }

  function clearSuppressTimer() {
    if (suppressTimer !== null) {
      clearTimeout(suppressTimer);
      suppressTimer = null;
    }
  }

  function reset() {
    clearTimer();
    activePointer = null;
    fired = false;
  }

  function suppressNextClick() {
    suppressClick = true;
    clearSuppressTimer();
    suppressTimer = setTimeout(() => {
      suppressClick = false;
      suppressTimer = null;
    }, SUPPRESS_CLICK_MS);
  }

  function fire() {
    fired = true;
    // The pointer is still down, so the release will produce a click. Swallow
    // it before the overlay that now covers the button can act on it.
    suppressNextClick();
    options.onLongPress();
  }

  function onPointerDown(event: PointerEvent) {
    if (options.disabled?.() || !event.isPrimary) {
      return;
    }
    if (event.pointerType === "mouse" && event.button !== 0) {
      return;
    }

    reset();
    clearSuppressTimer();
    suppressClick = false;
    startX = event.clientX;
    startY = event.clientY;
    activePointer = event.pointerId;
    timer = setTimeout(() => {
      timer = null;
      fire();
    }, options.delay ?? DEFAULT_DELAY_MS);
  }

  function onPointerMove(event: PointerEvent) {
    if (activePointer === null || event.pointerId !== activePointer) {
      return;
    }
    // A fired press already opened the overlay; dragging must not close it.
    if (fired) {
      return;
    }

    const distance = Math.hypot(event.clientX - startX, event.clientY - startY);
    if (distance > (options.moveThreshold ?? DEFAULT_MOVE_THRESHOLD_PX)) {
      reset();
    }
  }

  function onPointerUp(event: PointerEvent) {
    if (activePointer === null || event.pointerId !== activePointer) {
      return;
    }

    // Firing is the timer's job; releasing only ends the press.
    reset();
  }

  function onPointerCancel(event: PointerEvent) {
    if (activePointer !== null && event.pointerId !== activePointer) {
      return;
    }
    reset();
  }

  function onContextMenu(event: MouseEvent) {
    event.preventDefault();
    if (options.disabled?.()) {
      return;
    }
    // Context menu is the desktop/keyboard-adjacent fallback for a long press.
    reset();
    options.onLongPress();
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key !== "Enter" || !event.shiftKey) {
      return;
    }
    event.preventDefault();
    if (options.disabled?.()) {
      return;
    }
    suppressNextClick();
    options.onLongPress();
  }

  /** Call from the button's click handler; `true` means the click was a long press. */
  function consumeClick(): boolean {
    if (!suppressClick) {
      return false;
    }
    suppressClick = false;
    clearSuppressTimer();
    return true;
  }

  onBeforeUnmount(() => {
    reset();
    clearSuppressTimer();
  });

  return {
    handlers: {
      pointerdown: onPointerDown,
      pointermove: onPointerMove,
      pointerup: onPointerUp,
      pointercancel: onPointerCancel,
      pointerleave: onPointerCancel,
      contextmenu: onContextMenu,
      keydown: onKeyDown,
    },
    consumeClick,
  };
}
