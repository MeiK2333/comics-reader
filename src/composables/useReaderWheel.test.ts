import { ref } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useReaderWheel } from "./useReaderWheel";

function wheelEvent(init: Partial<WheelEvent> = {}): WheelEvent {
  return {
    deltaY: 100,
    deltaMode: 0,
    ctrlKey: false,
    clientX: 10,
    clientY: 20,
    preventDefault: vi.fn(),
    ...init,
  } as unknown as WheelEvent;
}

function createWheel(animating = false) {
  const nextPage = vi.fn();
  const previousPage = vi.fn();
  const zoomBy = vi.fn();
  const { onWheel } = useReaderWheel({
    stage: ref<HTMLElement | null>(null),
    isAnimating: () => animating,
    nextPage,
    previousPage,
    zoomBy,
  });
  return { onWheel, nextPage, previousPage, zoomBy };
}

beforeEach(() => {
  vi.spyOn(performance, "now").mockReturnValue(1_000);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("useReaderWheel", () => {
  it("turns the page once the wheel accumulator passes the threshold", () => {
    const { onWheel, nextPage } = createWheel();

    onWheel(wheelEvent({ deltaY: 100 }));

    expect(nextPage).toHaveBeenCalledTimes(1);
  });

  it("throttles repeated wheel turns", () => {
    const { onWheel, nextPage } = createWheel();
    const now = performance.now as ReturnType<typeof vi.fn>;

    onWheel(wheelEvent({ deltaY: 100 }));
    now.mockReturnValue(1_100);
    onWheel(wheelEvent({ deltaY: 100 }));
    expect(nextPage).toHaveBeenCalledTimes(1);

    now.mockReturnValue(1_400);
    onWheel(wheelEvent({ deltaY: 100 }));
    expect(nextPage).toHaveBeenCalledTimes(2);
  });

  it("zooms with Ctrl+wheel instead of turning", () => {
    const { onWheel, nextPage, zoomBy } = createWheel();

    onWheel(wheelEvent({ deltaY: -50, ctrlKey: true, clientX: 4, clientY: 8 }));

    expect(zoomBy).toHaveBeenCalledTimes(1);
    expect(nextPage).not.toHaveBeenCalled();
  });

  it("ignores wheel input during a turn animation", () => {
    const { onWheel, nextPage } = createWheel(true);

    onWheel(wheelEvent());

    expect(nextPage).not.toHaveBeenCalled();
  });

  it("turns backward for a negative delta", () => {
    const { onWheel, previousPage } = createWheel();

    onWheel(wheelEvent({ deltaY: -100 }));

    expect(previousPage).toHaveBeenCalledTimes(1);
  });
});
