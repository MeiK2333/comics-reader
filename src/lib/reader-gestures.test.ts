import { describe, expect, it } from "vitest";
import {
  atPageBoundary,
  dragLimit,
  isForwardGesture,
  swipeTarget,
  tapTurnsForward,
  tapZone,
} from "./reader-gestures";

describe("tapZone", () => {
  it("splits the stage into thirds", () => {
    expect(tapZone(0)).toBe("left");
    expect(tapZone(0.2)).toBe("left");
    expect(tapZone(1 / 3)).toBe("middle");
    expect(tapZone(0.5)).toBe("middle");
    expect(tapZone(2 / 3)).toBe("middle");
    expect(tapZone(0.8)).toBe("right");
    expect(tapZone(1)).toBe("right");
  });

  it("maps positions outside the stage to the outer zones", () => {
    expect(tapZone(-0.5)).toBe("left");
    expect(tapZone(1.5)).toBe("right");
  });
});

describe("isForwardGesture", () => {
  it("maps ltr, rtl and vertical deltas", () => {
    expect(isForwardGesture("ltr", true, -10)).toBe(true);
    expect(isForwardGesture("ltr", true, 10)).toBe(false);
    expect(isForwardGesture("rtl", true, 10)).toBe(true);
    expect(isForwardGesture("rtl", true, -10)).toBe(false);
    expect(isForwardGesture("ttb", false, -10)).toBe(true);
    expect(isForwardGesture("ttb", false, 10)).toBe(false);
  });

  it("treats a zero delta as backward", () => {
    expect(isForwardGesture("ltr", true, 0)).toBe(false);
    expect(isForwardGesture("rtl", true, 0)).toBe(false);
  });
});

describe("tapTurnsForward", () => {
  it("makes the right side forward except in rtl", () => {
    expect(tapTurnsForward("ltr", "right")).toBe(true);
    expect(tapTurnsForward("ltr", "left")).toBe(false);
    expect(tapTurnsForward("rtl", "left")).toBe(true);
    expect(tapTurnsForward("rtl", "right")).toBe(false);
    expect(tapTurnsForward("ttb", "right")).toBe(true);
    expect(tapTurnsForward("ttb", "left")).toBe(false);
  });
});

describe("atPageBoundary", () => {
  it("detects the edge the gesture would leave", () => {
    expect(atPageBoundary(3, 3, true)).toBe(true);
    expect(atPageBoundary(2, 3, true)).toBe(false);
    expect(atPageBoundary(1, 3, false)).toBe(true);
    expect(atPageBoundary(2, 3, false)).toBe(false);
  });
});

describe("swipeTarget", () => {
  it("returns the adjacent page inside the chapter", () => {
    expect(swipeTarget(2, 5, true)).toBe(3);
    expect(swipeTarget(2, 5, false)).toBe(1);
  });

  it("returns null when the swipe leaves the chapter", () => {
    expect(swipeTarget(5, 5, true)).toBeNull();
    expect(swipeTarget(1, 5, false)).toBeNull();
    expect(swipeTarget(1, 1, true)).toBeNull();
    expect(swipeTarget(1, 1, false)).toBeNull();
  });
});

describe("dragLimit", () => {
  it("follows the pointer normally and rubber-bands at the boundary", () => {
    expect(dragLimit(900, false)).toBe(900);
    expect(dragLimit(900, true)).toBeCloseTo(162);
  });
});
