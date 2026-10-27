import { computed, effectScope, nextTick, ref } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { usePageTurn } from "./usePageTurn";

function createTurn(initialPage = 1, initialTotal = 3) {
  const page = ref(initialPage);
  const totalSource = ref(initialTotal);
  const stage = ref<HTMLElement | null>(null);
  const onPageChange = vi.fn();
  const onBoundary = vi.fn();
  const onSettled = vi.fn();

  const scope = effectScope();
  const turn = scope.run(() =>
    usePageTurn({
      page: () => page.value,
      total: computed(() => totalSource.value),
      direction: () => "ltr",
      horizontal: computed(() => true),
      stage,
      onPageChange,
      onBoundary,
      onSettled,
      isZoomed: () => false,
    }),
  );

  if (!turn) {
    throw new Error("page turn was not created");
  }

  return {
    page,
    total: totalSource,
    onPageChange,
    onBoundary,
    onSettled,
    turn,
    scope,
  };
}

function stubReducedMotion(matches: boolean) {
  vi.stubGlobal("matchMedia", () => ({
    matches,
    media: "(prefers-reduced-motion: reduce)",
    onchange: null,
    addListener: () => undefined,
    removeListener: () => undefined,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    dispatchEvent: () => false,
  }));
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    callback(0);
    return 1;
  });
  stubReducedMotion(true);
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("usePageTurn", () => {
  it("finishes a one-page turn and reports the new page", () => {
    const { turn, onPageChange } = createTurn();

    turn.turnTo(2);

    expect(turn.displayPage.value).toBe(2);
    expect(onPageChange).toHaveBeenCalledWith(2);
    expect(turn.trackOffset.value).toBe(0);
    expect(turn.trackAnimating.value).toBe(false);
  });

  it("jumps more than one page without faking a turn", () => {
    const { turn, onPageChange } = createTurn();

    turn.goToPage(3);

    expect(turn.displayPage.value).toBe(3);
    expect(onPageChange).toHaveBeenCalledWith(3);
    expect(turn.turnTarget.value).toBeNull();
  });

  it("reports a boundary when the chapter has no previous page", () => {
    const { turn, onBoundary } = createTurn();

    turn.previousPage();

    expect(onBoundary).toHaveBeenCalledWith("previous");
    expect(turn.boundaryLeaving.value).toBe(true);
  });

  it("mirrors a changed page prop and resets the boundary flag", async () => {
    const { page, turn } = createTurn();
    turn.previousPage();

    page.value = 3;
    await nextTick();

    expect(turn.displayPage.value).toBe(3);
    expect(turn.boundaryLeaving.value).toBe(false);
  });

  it("cancels an in-flight animation", () => {
    stubReducedMotion(false);
    const { turn } = createTurn();

    turn.turnTo(2);
    expect(turn.trackAnimating.value).toBe(true);
    expect(turn.trackOffset.value).toBe(-1);

    turn.cancelTurn();

    expect(turn.trackAnimating.value).toBe(false);
    expect(turn.trackOffset.value).toBe(0);
    expect(turn.turnTarget.value).toBeNull();
  });

  it("snaps back to the current page without reporting a change", () => {
    stubReducedMotion(true);
    const { turn, onPageChange } = createTurn();
    turn.trackOffset.value = 40;

    turn.snapBack();

    expect(turn.trackOffset.value).toBe(0);
    expect(turn.displayPage.value).toBe(1);
    expect(onPageChange).not.toHaveBeenCalled();
  });

  it("settles the initial and changed page", async () => {
    const { turn, onSettled, total } = createTurn();

    expect(onSettled).toHaveBeenLastCalledWith(1);

    total.value = 5;
    turn.goToPage(4);
    await nextTick();

    expect(onSettled).toHaveBeenLastCalledWith(4);
  });
});
