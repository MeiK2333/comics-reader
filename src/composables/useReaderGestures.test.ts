import { afterEach, describe, expect, it, vi } from "vitest";
import { computed, effectScope, ref } from "vue";
import type { EffectScope } from "vue";
import type { ReaderDirection } from "../stores/settings";
import { useReaderGestures } from "./useReaderGestures";

const scopes: EffectScope[] = [];

afterEach(() => {
  for (const scope of scopes) {
    scope.stop();
  }
  scopes.length = 0;
});

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
  element.setPointerCapture = vi.fn();
  element.releasePointerCapture = vi.fn();
  element.hasPointerCapture = vi.fn(() => true);
  return element;
}

interface HarnessOptions {
  direction?: ReaderDirection;
  displayPage?: number;
  total?: number;
  scale?: number;
  animating?: boolean;
}

function createHarness(options: HarnessOptions = {}) {
  const stageElement = makeStage();
  const stage = ref<HTMLElement | null>(stageElement);
  const displayPage = ref(options.displayPage ?? 1);
  const scale = ref(options.scale ?? 1);
  const trackOffset = ref(0);
  const direction = options.direction ?? "ltr";
  const animating = options.animating ?? false;

  const calls = {
    turnTo: vi.fn(),
    nextPage: vi.fn(),
    previousPage: vi.fn(),
    toggleChrome: vi.fn(),
    panBy: vi.fn(),
    setScale: vi.fn(),
    snapBack: vi.fn(),
    cancelTurn: vi.fn(),
    resetTrack: vi.fn(),
  };

  const scope = effectScope();
  const api = scope.run(() =>
    useReaderGestures({
      stage,
      isAnimating: () => animating,
      direction: () => direction,
      horizontal: computed(() => direction !== "ttb"),
      total: computed(() => options.total ?? 3),
      displayPage,
      trackOffset,
      stageSpan: () => 900,
      scale,
      panBy: calls.panBy,
      setScale: calls.setScale,
      cancelTurn: calls.cancelTurn,
      resetTrack: calls.resetTrack,
      snapBack: calls.snapBack,
      turnTo: calls.turnTo,
      nextPage: calls.nextPage,
      previousPage: calls.previousPage,
      toggleChrome: calls.toggleChrome,
    }),
  )!;

  scopes.push(scope);

  return { api, calls, stageElement, displayPage, trackOffset, scale };
}

type Harness = ReturnType<typeof createHarness>;

interface PointerOptions {
  id?: number;
  target?: Element;
}

function pointer(
  stage: Element,
  x: number,
  y: number,
  options: PointerOptions = {},
): PointerEvent {
  return {
    pointerId: options.id ?? 1,
    pointerType: "touch",
    button: 0,
    clientX: x,
    clientY: y,
    target: options.target ?? stage,
    preventDefault: vi.fn(),
  } as unknown as PointerEvent;
}

function down(harness: Harness, x: number, y: number, options?: PointerOptions) {
  harness.api.onStagePointerDown(
    pointer(harness.stageElement, x, y, options),
  );
}

function move(harness: Harness, x: number, y: number, options?: PointerOptions) {
  harness.api.onStagePointerMove(
    pointer(harness.stageElement, x, y, options),
  );
}

function up(harness: Harness, x: number, y: number, options?: PointerOptions) {
  harness.api.onStagePointerUp(pointer(harness.stageElement, x, y, options));
}

function cancel(harness: Harness, x: number, y: number, options?: PointerOptions) {
  harness.api.onStagePointerCancel(
    pointer(harness.stageElement, x, y, options),
  );
}

function tap(harness: Harness, x: number, y: number) {
  down(harness, x, y);
  up(harness, x, y);
}

describe("useReaderGestures", () => {
  it("turns forward on a left swipe in ltr", () => {
    const harness = createHarness();

    down(harness, 700, 300);
    move(harness, 500, 300);
    up(harness, 500, 300);

    expect(harness.calls.turnTo).toHaveBeenCalledWith(2);
    expect(harness.calls.snapBack).not.toHaveBeenCalled();
  });

  it("turns forward on a right swipe in rtl", () => {
    const harness = createHarness({ direction: "rtl" });

    down(harness, 500, 300);
    move(harness, 700, 300);
    up(harness, 700, 300);

    expect(harness.calls.turnTo).toHaveBeenCalledWith(2);
  });

  it("turns forward on an upward swipe in ttb", () => {
    const harness = createHarness({ direction: "ttb" });

    down(harness, 450, 500);
    move(harness, 450, 300);
    up(harness, 450, 300);

    expect(harness.calls.turnTo).toHaveBeenCalledWith(2);
  });

  it("snaps back when the swipe is below the threshold", () => {
    const harness = createHarness();

    down(harness, 500, 300);
    move(harness, 520, 300);
    up(harness, 520, 300);

    expect(harness.calls.turnTo).not.toHaveBeenCalled();
    expect(harness.calls.snapBack).toHaveBeenCalledOnce();
  });

  it("leaves the chapter instead of targeting an out-of-range page", () => {
    const harness = createHarness({ displayPage: 3, total: 3 });

    down(harness, 700, 300);
    move(harness, 500, 300);
    up(harness, 500, 300);

    expect(harness.calls.nextPage).toHaveBeenCalledOnce();
    expect(harness.calls.turnTo).not.toHaveBeenCalled();
  });

  it("limits the track offset once the drag hits the chapter edge", () => {
    const atEdge = createHarness({ displayPage: 3, total: 3 });
    down(atEdge, 700, 300);
    move(atEdge, 0, 300);
    expect(atEdge.trackOffset.value).toBe(-162);

    const interior = createHarness({ displayPage: 1, total: 3 });
    down(interior, 300, 300);
    move(interior, 100, 300);
    expect(interior.trackOffset.value).toBe(-200);
  });

  it("maps side taps to page turns and the middle to chrome", () => {
    const ltr = createHarness({ displayPage: 2, total: 5 });
    tap(ltr, 850, 300);
    expect(ltr.calls.nextPage).toHaveBeenCalledOnce();

    const rtl = createHarness({
      direction: "rtl",
      displayPage: 2,
      total: 5,
    });
    tap(rtl, 850, 300);
    expect(rtl.calls.previousPage).toHaveBeenCalledOnce();

    const middle = createHarness({ displayPage: 2, total: 5 });
    tap(middle, 450, 300);
    expect(middle.calls.toggleChrome).toHaveBeenCalledOnce();
  });

  it("pans instead of turning while zoomed", () => {
    const harness = createHarness({
      scale: 2,
      displayPage: 2,
      total: 3,
    });

    down(harness, 500, 300);
    move(harness, 600, 300);
    expect(harness.calls.panBy).toHaveBeenCalledWith(100, 0);

    up(harness, 600, 300);
    expect(harness.calls.snapBack).toHaveBeenCalledOnce();
    expect(harness.calls.turnTo).not.toHaveBeenCalled();
    expect(harness.calls.nextPage).not.toHaveBeenCalled();
    expect(harness.calls.previousPage).not.toHaveBeenCalled();
    expect(harness.calls.toggleChrome).not.toHaveBeenCalled();
  });

  it("pinches with two touches and never turns or taps afterwards", () => {
    const harness = createHarness();

    down(harness, 400, 300, { id: 1 });
    down(harness, 600, 300, { id: 2 });
    move(harness, 380, 300, { id: 1 });

    expect(harness.calls.panBy).toHaveBeenCalled();
    expect(harness.calls.setScale).toHaveBeenCalled();
    expect(harness.calls.setScale.mock.calls[0]?.[0]).toBeCloseTo(1.1);

    up(harness, 380, 300, { id: 1 });
    move(harness, 610, 300, { id: 2 });
    up(harness, 610, 300, { id: 2 });

    expect(harness.calls.snapBack).toHaveBeenCalled();
    expect(harness.calls.turnTo).not.toHaveBeenCalled();
    expect(harness.calls.nextPage).not.toHaveBeenCalled();
    expect(harness.calls.previousPage).not.toHaveBeenCalled();
    expect(harness.calls.toggleChrome).not.toHaveBeenCalled();
  });

  it("ignores pointer input while the turn animation runs", () => {
    const harness = createHarness({ animating: true });

    down(harness, 700, 300);
    move(harness, 500, 300);
    up(harness, 500, 300);

    expect(harness.calls.turnTo).not.toHaveBeenCalled();
    expect(harness.calls.snapBack).not.toHaveBeenCalled();
    expect(harness.trackOffset.value).toBe(0);
  });

  it("ignores pointers that start on an interactive element", () => {
    const harness = createHarness();
    const button = document.createElement("button");
    harness.stageElement.appendChild(button);

    down(harness, 850, 300, { target: button });
    up(harness, 850, 300, { target: button });

    expect(harness.calls.nextPage).not.toHaveBeenCalled();
    expect(harness.calls.toggleChrome).not.toHaveBeenCalled();
  });

  it("snaps back when the pointer is cancelled", () => {
    const harness = createHarness();

    down(harness, 700, 300);
    cancel(harness, 500, 300);

    expect(harness.calls.snapBack).toHaveBeenCalledOnce();
    expect(harness.calls.turnTo).not.toHaveBeenCalled();
  });
});
