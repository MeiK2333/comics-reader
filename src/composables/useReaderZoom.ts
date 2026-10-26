import { computed, ref } from "vue";
import type { Ref } from "vue";

export interface ReaderImageSize {
  width: number;
  height: number;
}

const MIN_SCALE = 1;
const MAX_SCALE = 4;

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

/**
 * CSS-transform zoom and pan for a single page image. The image stays centered
 * in the stage at offset (0, 0); offsets are clamped so the viewport cannot be
 * panned past the image edges.
 */
export function useReaderZoom(
  stage: Ref<HTMLElement | null>,
  imageSize: Ref<ReaderImageSize | null>,
) {
  const scale = ref(1);
  const offset = ref({ x: 0, y: 0 });

  function clampOffset(
    nextX = offset.value.x,
    nextY = offset.value.y,
    nextScale = scale.value,
  ) {
    const element = stage.value;
    const image = imageSize.value;
    if (!element || !image) {
      offset.value = { x: 0, y: 0 };
      return;
    }

    const maxX = Math.max(
      0,
      (image.width * nextScale - element.clientWidth) / 2,
    );
    const maxY = Math.max(
      0,
      (image.height * nextScale - element.clientHeight) / 2,
    );
    offset.value = {
      x: clamp(nextX, -maxX, maxX),
      y: clamp(nextY, -maxY, maxY),
    };
  }

  function setScale(nextScale: number, clientX?: number, clientY?: number) {
    const target = clamp(nextScale, MIN_SCALE, MAX_SCALE);
    const element = stage.value;

    if (target === MIN_SCALE) {
      scale.value = MIN_SCALE;
      offset.value = { x: 0, y: 0 };
      return;
    }
    if (!element || target === scale.value) {
      return;
    }

    const rect = element.getBoundingClientRect();
    const originX = clientX ?? rect.left + rect.width / 2;
    const originY = clientY ?? rect.top + rect.height / 2;
    const relativeX = originX - rect.left - rect.width / 2;
    const relativeY = originY - rect.top - rect.height / 2;
    // Keep the image point under the cursor fixed while the scale changes.
    const imageX = (relativeX - offset.value.x) / scale.value;
    const imageY = (relativeY - offset.value.y) / scale.value;

    scale.value = target;
    offset.value = {
      x: relativeX - imageX * target,
      y: relativeY - imageY * target,
    };
    clampOffset();
  }

  function zoomBy(factor: number, clientX?: number, clientY?: number) {
    setScale(scale.value * factor, clientX, clientY);
  }

  function zoomStep(direction: 1 | -1) {
    setScale(scale.value + direction * 0.25);
  }

  function panBy(dx: number, dy: number) {
    offset.value = {
      x: offset.value.x + dx,
      y: offset.value.y + dy,
    };
    clampOffset();
  }

  function reset() {
    scale.value = MIN_SCALE;
    offset.value = { x: 0, y: 0 };
  }

  function reflow() {
    clampOffset();
  }

  const percent = computed(() => Math.round(scale.value * 100));

  return {
    scale,
    offset,
    percent,
    zoomBy,
    zoomStep,
    setScale,
    panBy,
    reset,
    reflow,
  };
}
