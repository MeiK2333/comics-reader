import type { Ref } from "vue";

export interface ReaderWheelOptions {
  stage: Ref<HTMLElement | null>;
  isAnimating: () => boolean;
  nextPage: () => void;
  previousPage: () => void;
  zoomBy: (factor: number, clientX?: number, clientY?: number) => void;
}

/**
 * Wheel handling for the reader stage: Ctrl+wheel zooms around the cursor,
 * a plain wheel turns pages with a small accumulator so trackpads do not skip
 * pages. The caller owns the listener so it can keep the `passive: false`
 * registration next to the other stage listeners.
 */
export function useReaderWheel(options: ReaderWheelOptions) {
  let wheelAccumulator = 0;
  let lastWheelAt = 0;
  let lastNavigationAt = 0;

  function onWheel(event: WheelEvent) {
    event.preventDefault();

    if (options.isAnimating()) {
      wheelAccumulator = 0;
      return;
    }

    if (event.ctrlKey) {
      options.zoomBy(
        Math.exp(-event.deltaY * 0.002),
        event.clientX,
        event.clientY,
      );
      wheelAccumulator = 0;
      return;
    }

    const modeScale =
      event.deltaMode === 1
        ? 16
        : event.deltaMode === 2
          ? (options.stage.value?.clientHeight ?? 600)
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
        options.nextPage();
      } else {
        options.previousPage();
      }
    }
  }

  return { onWheel };
}
