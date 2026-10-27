import { beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  back: vi.fn(),
  go: vi.fn(),
  replace: vi.fn(),
  resolve: vi.fn(() => ({ fullPath: "/history" })),
}));

vi.mock("vue-router", () => ({
  useRouter: () => mocks,
}));

import {
  recordNavigation,
  resetNavigationStack,
} from "../router/navigationStack";
import { useBackNavigation } from "./useBackNavigation";

beforeEach(() => {
  mocks.back.mockReset();
  mocks.go.mockReset();
  mocks.replace.mockReset();
  mocks.resolve.mockClear();
  resetNavigationStack();
  window.history.replaceState(null, "");
});

describe("useBackNavigation with a recorded stack", () => {
  it("walks back to an earlier component parent", () => {
    recordNavigation("/", 0);
    recordNavigation("/history", 1);
    recordNavigation("/comic/jm/1", 2);
    recordNavigation("/downloads", 3);

    useBackNavigation().goBack({ name: "history" });

    expect(mocks.go).toHaveBeenCalledWith(-2);
    expect(mocks.back).not.toHaveBeenCalled();
    expect(mocks.replace).not.toHaveBeenCalled();
  });

  it("still pops when the target is immediately previous", () => {
    window.history.replaceState({ back: "/history" }, "");
    mocks.resolve.mockReturnValue({ fullPath: "/history" });

    useBackNavigation().goBack({ name: "history" });

    expect(mocks.back).toHaveBeenCalledTimes(1);
    expect(mocks.go).not.toHaveBeenCalled();
  });

  it("replaces when the target is not in the app stack", () => {
    recordNavigation("/", 0);
    recordNavigation("/history", 1);

    useBackNavigation().goBack({ name: "favorites" });

    expect(mocks.replace).toHaveBeenCalledWith({ name: "favorites" });
    expect(mocks.go).not.toHaveBeenCalled();
  });
});
