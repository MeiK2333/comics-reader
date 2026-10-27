import { describe, expect, it } from "vitest";
import type { ComicDetail } from "../platforms";
import { AppError } from "../errors";
import { resolveDetailState } from "./detailState";

const detail = { platform: "nhentai", title: "Example", thumb: "" } as ComicDetail;

describe("resolveDetailState", () => {
  it("prefers an online detail even while loading is still true", () => {
    const state = resolveDetailState({
      detail,
      error: null,
      loading: true,
    });

    expect(state).toEqual({ kind: "online", detail });
  });

  it("reports the online error when there is no detail", () => {
    const error = new AppError("network", "offline");
    const state = resolveDetailState({
      detail: null,
      error,
      loading: false,
    });

    expect(state).toEqual({ kind: "error", error });
  });

  it("reports loading before an empty state", () => {
    expect(
      resolveDetailState({
        detail: null,
        error: null,
        loading: true,
      }),
    ).toEqual({ kind: "loading" });
  });

  it("falls back to empty when the request settled without a result", () => {
    expect(
      resolveDetailState({
        detail: null,
        error: null,
        loading: false,
      }),
    ).toEqual({ kind: "empty" });
  });
});
