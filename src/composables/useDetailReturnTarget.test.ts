import { describe, expect, it } from "vitest";

import {
  detailOriginFromPath,
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

  it("prefers explicit state over a legacy previous path", () => {
    const source =
      readDetailOrigin({ from: "downloads" }) ??
      detailOriginFromPath("/history");
    expect(source).toBe("downloads");
  });

  it("maps legacy list paths and rejects unrelated pages", () => {
    expect(detailOriginFromPath("/favorites")).toBe("favorites");
    expect(detailOriginFromPath("/history")).toBe("history");
    expect(detailOriginFromPath("/downloads")).toBe("downloads");
    expect(detailOriginFromPath("/comic/jm/1")).toBeNull();
    expect(detailOriginFromPath(null)).toBeNull();
  });
});
