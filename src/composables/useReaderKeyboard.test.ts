import { describe, expect, it, vi } from "vitest";
import { useReaderKeyboard } from "./useReaderKeyboard";

function keyEvent(
  key: string,
  init: Partial<KeyboardEvent> = {},
): KeyboardEvent {
  return {
    key,
    defaultPrevented: false,
    ctrlKey: false,
    metaKey: false,
    altKey: false,
    target: null,
    preventDefault: vi.fn(),
    ...init,
  } as unknown as KeyboardEvent;
}

function createKeyboard(direction: "ltr" | "rtl" | "ttb" = "ltr") {
  const nextPage = vi.fn();
  const previousPage = vi.fn();
  const goToPage = vi.fn();
  const zoomIn = vi.fn();
  const zoomOut = vi.fn();
  const resetZoom = vi.fn();
  const toggleFullscreen = vi.fn();
  const close = vi.fn();
  let animating = false;

  const { onKeydown } = useReaderKeyboard({
    direction: () => direction,
    total: () => 10,
    isAnimating: () => animating,
    nextPage,
    previousPage,
    goToPage,
    zoomIn,
    zoomOut,
    resetZoom,
    toggleFullscreen,
    close,
  });

  return {
    onKeydown,
    nextPage,
    previousPage,
    goToPage,
    zoomIn,
    zoomOut,
    resetZoom,
    toggleFullscreen,
    close,
    setAnimating: (value: boolean) => {
      animating = value;
    },
  };
}

describe("useReaderKeyboard", () => {
  it("maps horizontal directions to the expected arrow keys", () => {
    const ltr = createKeyboard("ltr");
    ltr.onKeydown(keyEvent("ArrowRight"));
    ltr.onKeydown(keyEvent("ArrowLeft"));
    expect(ltr.nextPage).toHaveBeenCalledTimes(1);
    expect(ltr.previousPage).toHaveBeenCalledTimes(1);

    const rtl = createKeyboard("rtl");
    rtl.onKeydown(keyEvent("ArrowLeft"));
    rtl.onKeydown(keyEvent("ArrowRight"));
    expect(rtl.nextPage).toHaveBeenCalledTimes(1);
    expect(rtl.previousPage).toHaveBeenCalledTimes(1);
  });

  it("jumps with Home and End", () => {
    const { onKeydown, goToPage } = createKeyboard();

    onKeydown(keyEvent("Home"));
    onKeydown(keyEvent("End"));

    expect(goToPage).toHaveBeenNthCalledWith(1, 1);
    expect(goToPage).toHaveBeenNthCalledWith(2, 10);
  });

  it("handles zoom, fullscreen and close shortcuts", () => {
    const keyboard = createKeyboard();

    keyboard.onKeydown(keyEvent("+"));
    keyboard.onKeydown(keyEvent("-"));
    keyboard.onKeydown(keyEvent("0"));
    keyboard.onKeydown(keyEvent("f"));
    keyboard.onKeydown(keyEvent("Escape"));

    expect(keyboard.zoomIn).toHaveBeenCalledTimes(1);
    expect(keyboard.zoomOut).toHaveBeenCalledTimes(1);
    expect(keyboard.resetZoom).toHaveBeenCalledTimes(1);
    expect(keyboard.toggleFullscreen).toHaveBeenCalledTimes(1);
    expect(keyboard.close).toHaveBeenCalledTimes(1);
  });

  it("ignores modified keys and editable targets", () => {
    const keyboard = createKeyboard();
    const input = document.createElement("input");

    keyboard.onKeydown(keyEvent("ArrowRight", { ctrlKey: true }));
    keyboard.onKeydown(keyEvent("ArrowRight", { target: input }));

    expect(keyboard.nextPage).not.toHaveBeenCalled();
  });

  it("ignores page keys while a turn is running but still allows Escape", () => {
    const keyboard = createKeyboard();
    keyboard.setAnimating(true);

    keyboard.onKeydown(keyEvent("ArrowRight"));
    keyboard.onKeydown(keyEvent("Escape"));

    expect(keyboard.nextPage).not.toHaveBeenCalled();
    expect(keyboard.close).toHaveBeenCalledTimes(1);
  });
});
