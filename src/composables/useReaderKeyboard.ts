import type { ReaderDirection } from "../stores/settings";

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

export interface ReaderKeyboardOptions {
  direction: () => ReaderDirection;
  total: () => number;
  isAnimating: () => boolean;
  nextPage: () => void;
  previousPage: () => void;
  goToPage: (page: number) => void;
  zoomIn: () => void;
  zoomOut: () => void;
  resetZoom: () => void;
  toggleFullscreen: () => void | Promise<void>;
  close: () => void;
}

function isEditableTarget(target: EventTarget | null) {
  return (
    target instanceof Element &&
    target.closest("input, textarea, select, [contenteditable='true']") !==
      null
  );
}

/**
 * Direction-aware arrow keys plus Home/End, zoom, fullscreen and Escape.
 * The caller owns the window listener so teardown stays in the component's
 * lifecycle block.
 */
export function useReaderKeyboard(options: ReaderKeyboardOptions) {
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

    if (options.isAnimating() && event.key !== "Escape") {
      return;
    }

    if (NEXT_KEYS[options.direction()].includes(event.key)) {
      event.preventDefault();
      options.nextPage();
      return;
    }
    if (PREVIOUS_KEYS[options.direction()].includes(event.key)) {
      event.preventDefault();
      options.previousPage();
      return;
    }

    switch (event.key) {
      case "Home":
        event.preventDefault();
        options.goToPage(1);
        break;
      case "End":
        event.preventDefault();
        options.goToPage(options.total());
        break;
      case "+":
      case "=":
        event.preventDefault();
        options.zoomIn();
        break;
      case "-":
      case "_":
        event.preventDefault();
        options.zoomOut();
        break;
      case "0":
        event.preventDefault();
        options.resetZoom();
        break;
      case "f":
      case "F":
        event.preventDefault();
        void options.toggleFullscreen();
        break;
      case "Escape":
        // Let the browser leave fullscreen first; the next Escape closes.
        if (document.fullscreenElement) {
          return;
        }
        event.preventDefault();
        options.close();
        break;
      default:
        break;
    }
  }

  return { onKeydown };
}
