import { describe, expect, it } from "vitest";

import {
  detailBackTarget,
  readDetailOrigin,
} from "./useDetailReturnTarget";

describe("detail return origin", () => {
  it("reads an explicit origin from navigation state", () => {
    expect(readDetailOrigin({ from: "favorites" })).toBe("favorites");
    expect(readDetailOrigin({ from: "history" })).toBe("history");
    expect(readDetailOrigin({ from: "downloads" })).toBe("downloads");
    expect(readDetailOrigin({ from: "search" })).toBe("search");
    expect(readDetailOrigin({ from: "home" })).toBe("home");
  });

  it("ignores missing or unknown origins", () => {
    expect(readDetailOrigin(null)).toBeNull();
    expect(readDetailOrigin(undefined)).toBeNull();
    expect(readDetailOrigin("favorites")).toBeNull();
    expect(readDetailOrigin({ from: "settings" })).toBeNull();
    expect(readDetailOrigin({ from: 1 })).toBeNull();
  });

  it("targets the component that opened the detail", () => {
    expect(detailBackTarget("history")).toEqual({ name: "history" });
    expect(detailBackTarget("search")).toEqual({ name: "search" });
  });

  it("returns home when a deep link has no explicit origin", () => {
    expect(detailBackTarget(null)).toEqual({ name: "home" });
  });
});
