import { describe, expect, it } from "vitest";
import { ref } from "vue";
import type { ReaderImageSize } from "./useReaderZoom";
import { useReaderZoom } from "./useReaderZoom";

function makeStage(width = 900, height = 600) {
  const element = document.createElement("div");
  Object.defineProperties(element, {
    clientWidth: { value: width },
    clientHeight: { value: height },
  });
  element.getBoundingClientRect = () =>
    ({
      left: 0,
      top: 0,
      width,
      height,
      right: width,
      bottom: height,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    }) as DOMRect;
  return element;
}

function setup(image: ReaderImageSize | null = { width: 600, height: 600 }) {
  const stage = ref<HTMLElement | null>(makeStage());
  const imageSize = ref<ReaderImageSize | null>(image);
  const zoom = useReaderZoom(stage, imageSize);
  return { stage, imageSize, zoom };
}

describe("useReaderZoom", () => {
  it("starts at 1x without an offset", () => {
    const { zoom } = setup();

    expect(zoom.scale.value).toBe(1);
    expect(zoom.offset.value).toEqual({ x: 0, y: 0 });
    expect(zoom.percent.value).toBe(100);
  });

  it("clamps the scale into the 1..4 range", () => {
    const { zoom } = setup();

    zoom.setScale(0.25);
    expect(zoom.scale.value).toBe(1);

    zoom.setScale(10);
    expect(zoom.scale.value).toBe(4);

    zoom.setScale(2);
    expect(zoom.scale.value).toBe(2);
  });

  it("resets the offset when returning to 1x", () => {
    const { zoom } = setup();

    zoom.setScale(2);
    zoom.panBy(50, 50);
    expect(zoom.offset.value).not.toEqual({ x: 0, y: 0 });

    zoom.setScale(1);
    expect(zoom.scale.value).toBe(1);
    expect(zoom.offset.value).toEqual({ x: 0, y: 0 });
  });

  it("steps the scale by a quarter and clamps at both ends", () => {
    const { zoom } = setup();

    zoom.zoomStep(1);
    expect(zoom.scale.value).toBeCloseTo(1.25);

    for (let index = 0; index < 20; index += 1) {
      zoom.zoomStep(1);
    }
    expect(zoom.scale.value).toBe(4);

    zoom.zoomStep(-1);
    expect(zoom.scale.value).toBeCloseTo(3.75);
  });

  it("keeps the point under the cursor fixed while scaling", () => {
    const { zoom } = setup();

    // Center of the 900x600 stage: no offset is needed.
    zoom.setScale(2, 450, 300);
    expect(zoom.scale.value).toBe(2);
    expect(zoom.offset.value).toEqual({ x: 0, y: 0 });

    // 50px right of center: the image point at x=50 moves to x=100 after
    // scaling, so the content must shift left by 50px to stay under the pointer.
    zoom.setScale(4, 500, 300);
    expect(zoom.offset.value.x).toBeCloseTo(-50);
    expect(zoom.offset.value.y).toBeCloseTo(0);
  });

  it("clamps panning to the image edges", () => {
    const { zoom } = setup();

    zoom.setScale(2);
    // maxX = (600 * 2 - 900) / 2 = 150, maxY = (600 * 2 - 600) / 2 = 300
    zoom.panBy(10_000, 10_000);
    expect(zoom.offset.value).toEqual({ x: 150, y: 300 });

    zoom.panBy(-10_000, -10_000);
    expect(zoom.offset.value).toEqual({ x: -150, y: -300 });
  });

  it("does not allow panning while the image fits the viewport", () => {
    const { zoom } = setup();

    zoom.panBy(100, 100);

    expect(zoom.offset.value).toEqual({ x: 0, y: 0 });
  });

  it("re-clamps the offset when the image shrinks", () => {
    const { zoom, imageSize } = setup({ width: 1200, height: 600 });

    zoom.setScale(2);
    // maxX = (1200 * 2 - 900) / 2 = 750
    zoom.panBy(500, 0);
    expect(zoom.offset.value.x).toBe(500);

    imageSize.value = { width: 600, height: 600 };
    zoom.reflow();
    // maxX = (600 * 2 - 900) / 2 = 150
    expect(zoom.offset.value.x).toBe(150);
  });

  it("keeps the offset at zero without a stage or image size", () => {
    const stage = ref<HTMLElement | null>(null);
    const imageSize = ref<ReaderImageSize | null>({ width: 600, height: 600 });
    const zoom = useReaderZoom(stage, imageSize);

    zoom.setScale(2);
    expect(zoom.scale.value).toBe(1);

    zoom.panBy(10, 10);
    expect(zoom.offset.value).toEqual({ x: 0, y: 0 });

    stage.value = document.createElement("div");
    imageSize.value = null;
    zoom.setScale(2);
    expect(zoom.scale.value).toBe(2);
    expect(zoom.offset.value).toEqual({ x: 0, y: 0 });
  });

  it("rounds the zoom percentage", () => {
    const { zoom } = setup();

    zoom.setScale(1.333);
    expect(zoom.percent.value).toBe(133);

    zoom.setScale(2.5);
    expect(zoom.percent.value).toBe(250);
  });
});
