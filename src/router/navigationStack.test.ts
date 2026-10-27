import { describe, expect, it, beforeEach } from "vitest";

import {
  backDeltaTo,
  recordNavigation,
  resetNavigationStack,
} from "./navigationStack";

beforeEach(() => {
  resetNavigationStack();
});

describe("navigation stack", () => {
  it("finds a component parent even when another page is in between", () => {
    recordNavigation("/", 0);
    recordNavigation("/history", 1);
    recordNavigation("/comic/jm/1", 2);
    recordNavigation("/downloads", 3);

    expect(backDeltaTo("/history")).toBe(-2);
    expect(backDeltaTo("/")).toBe(-3);
    expect(backDeltaTo("/comic/jm/1")).toBe(-1);
  });

  it("returns null for an unknown target or before the first record", () => {
    expect(backDeltaTo("/")).toBeNull();

    recordNavigation("/", 0);
    expect(backDeltaTo("/favorites")).toBeNull();
    expect(backDeltaTo("/")).toBeNull();
  });

  it("drops the forward branch when a new push happens after going back", () => {
    recordNavigation("/", 0);
    recordNavigation("/history", 1);
    recordNavigation("/comic/jm/1", 2);

    // Back to /history, then push /comic/jm/2 at position 2.
    recordNavigation("/comic/jm/2", 2);

    expect(backDeltaTo("/comic/jm/1")).toBeNull();
    expect(backDeltaTo("/history")).toBe(-1);
  });

  it("updates the current entry when replace keeps the same position", () => {
    recordNavigation("/", 0);
    recordNavigation("/comic/jm/1", 1);
    recordNavigation("/comic/jm/2", 1);

    expect(backDeltaTo("/comic/jm/1")).toBeNull();
    expect(backDeltaTo("/")).toBe(-1);
  });

  it("ignores entries without a usable history position", () => {
    recordNavigation("/", undefined);
    recordNavigation("/history", null);
    recordNavigation("/downloads", Number.NaN);

    expect(backDeltaTo("/")).toBeNull();
  });
});
