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
 * The picker opens on pointerup rather than while the finger is down, so a
 * modal cannot appear under the pointer. The click that follows a long press is
 * swallowed once, otherwise releasing after a long press would also run the
 * button's normal short-press action.
 */
export function useLongPress(options: LongPressOptions) {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let suppressTimer: ReturnType<typeof setTimeout> | null = null;
  let startX = 0;
  let startY = 0;
  let activePointer: number | null = null;
  let longPressReady = false;
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
    longPressReady = false;
  }

  function suppressNextClick() {
    suppressClick = true;
    clearSuppressTimer();
    suppressTimer = setTimeout(() => {
      suppressClick = false;
      suppressTimer = null;
    }, SUPPRESS_CLICK_MS);
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
      longPressReady = true;
    }, options.delay ?? DEFAULT_DELAY_MS);
  }

  function onPointerMove(event: PointerEvent) {
    if (activePointer === null || event.pointerId !== activePointer) {
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

    const shouldOpen = longPressReady;
    reset();
    if (shouldOpen) {
      // The release also produces a click; consume it before the button's
      // normal short-press handler sees it.
      suppressNextClick();
      options.onLongPress();
    }
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
