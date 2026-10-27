import { describe, expect, it } from "vitest";
import type { ComicDetail } from "../api/comics";
import type { DownloadDetail } from "../api/downloads";
import { AppError } from "../errors";
import { resolveDetailState } from "./detailState";

const detail = { platform: "nhentai", title: "Example", thumb: "" } as ComicDetail;
const download = {
  platform: "nhentai",
  id: "1",
  title: "Example",
  completedChapters: 1,
  totalChapters: 1,
} as DownloadDetail;

describe("resolveDetailState", () => {
  it("prefers an online detail even while loading is still true", () => {
    const state = resolveDetailState({
      detail,
      download,
      error: null,
      offline: true,
      loading: true,
    });

    expect(state).toEqual({ kind: "online", detail });
  });

  it("shows the offline download when the fallback wins", () => {
    const state = resolveDetailState({
      detail: null,
      download,
      error: null,
      offline: true,
      loading: false,
    });

    expect(state).toEqual({ kind: "offline", download });
  });

  it("does not treat a missing download as an offline surface", () => {
    const error = new AppError("network", "offline");
    const state = resolveDetailState({
      detail: null,
      download: null,
      error,
      offline: true,
      loading: false,
    });

    expect(state).toEqual({ kind: "error", error });
  });

  it("reports loading before an empty state", () => {
    expect(
      resolveDetailState({
        detail: null,
        download: null,
        error: null,
        offline: false,
        loading: true,
      }),
    ).toEqual({ kind: "loading" });
  });

  it("falls back to empty when the request settled without a result", () => {
    expect(
      resolveDetailState({
        detail: null,
        download: null,
        error: null,
        offline: false,
        loading: false,
      }),
    ).toEqual({ kind: "empty" });
  });
});
