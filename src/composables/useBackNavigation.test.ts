import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  back: vi.fn(),
  go: vi.fn(),
  replace: vi.fn(),
  resolve: vi.fn((target: unknown) => ({
    fullPath:
      typeof target === "string"
        ? target
        : ((target as { path?: string }).path ?? ""),
  })),
}));

vi.mock("vue-router", () => ({
  useRouter: () => mocks,
}));

import { resetNavigationStack } from "../router/navigationStack";
import { previousPath, useBackNavigation } from "./useBackNavigation";

beforeEach(() => {
  mocks.back.mockReset();
  mocks.go.mockReset();
  mocks.replace.mockReset();
  resetNavigationStack();
  mocks.resolve.mockClear();
  window.history.replaceState(null, "");
});

describe("previousPath", () => {
  it("returns null without a usable history entry", () => {
    expect(previousPath()).toBeNull();

    window.history.replaceState({ back: 42 }, "");
    expect(previousPath()).toBeNull();

    window.history.replaceState({ back: "" }, "");
    expect(previousPath()).toBeNull();
  });

  it("unwraps hash routes", () => {
    window.history.replaceState({ back: "#/favorites" }, "");
    expect(previousPath()).toBe("/favorites");

    window.history.replaceState({ back: "#" }, "");
    expect(previousPath()).toBe("/");
  });

  it("parses absolute URLs with and without a hash", () => {
    window.history.replaceState(
      { back: "http://localhost:1420/#/history?page=2" },
      "",
    );
    expect(previousPath()).toBe("/history?page=2");

    window.history.replaceState(
      { back: "http://localhost:1420/downloads?tab=all" },
      "",
    );
    expect(previousPath()).toBe("/downloads?tab=all");
  });

  it("normalizes relative paths", () => {
    window.history.replaceState({ back: "search" }, "");
    expect(previousPath()).toBe("/search");

    window.history.replaceState({ back: "/comic/jm/1" }, "");
    expect(previousPath()).toBe("/comic/jm/1");
  });
});

describe("useBackNavigation", () => {
  it("pops when the previous page is already the target", () => {
    window.history.replaceState({ back: "/search" }, "");
    mocks.resolve.mockReturnValue({ fullPath: "/search" });

    useBackNavigation().goBack({ name: "search" });

    expect(mocks.back).toHaveBeenCalledTimes(1);
    expect(mocks.replace).not.toHaveBeenCalled();
  });

  it("replaces for deep links and unexpected origins", () => {
    window.history.replaceState({ back: "/settings" }, "");
    mocks.resolve.mockReturnValue({ fullPath: "/search" });

    useBackNavigation().goBack({ name: "search" });

    expect(mocks.replace).toHaveBeenCalledWith({ name: "search" });
    expect(mocks.back).not.toHaveBeenCalled();
  });

  it("replaces when there is no previous entry", () => {
    mocks.resolve.mockReturnValue({ fullPath: "/" });

    useBackNavigation().goBack("/");

    expect(mocks.replace).toHaveBeenCalledWith("/");
    expect(mocks.back).not.toHaveBeenCalled();
  });
});
