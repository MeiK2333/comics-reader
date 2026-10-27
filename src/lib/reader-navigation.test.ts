import { describe, expect, it } from "vitest";
import {
  readerBoundaryAction,
  resolveReaderPage,
} from "./reader-navigation";

describe("resolveReaderPage", () => {
  it("resolves the internal last-page marker", () => {
    expect(resolveReaderPage("last", 12)).toEqual({
      page: 12,
      startAtEnd: true,
    });
  });

  it("clamps numeric values to the chapter", () => {
    expect(resolveReaderPage("3", 12)).toEqual({
      page: 3,
      startAtEnd: false,
    });
    expect(resolveReaderPage("0", 12).page).toBe(1);
    expect(resolveReaderPage("99", 12).page).toBe(12);
  });

  it("falls back to the first page for missing or invalid values", () => {
    for (const raw of [undefined, null, "", "abc", Number.NaN]) {
      expect(resolveReaderPage(raw, 12)).toEqual({
        page: 1,
        startAtEnd: false,
      });
    }
  });

  it("never returns a page below one when the chapter is empty", () => {
    expect(resolveReaderPage("5", 0).page).toBe(1);
    expect(resolveReaderPage("last", 0)).toEqual({
      page: 1,
      startAtEnd: true,
    });
  });
});

describe("readerBoundaryAction", () => {
  const base = {
    page: 1,
    total: 10,
    previousChapterId: null as string | null,
    nextChapterId: null as string | null,
  };

  it("turns inside the chapter before considering chapters", () => {
    expect(
      readerBoundaryAction({
        ...base,
        direction: "next",
        page: 3,
        nextChapterId: "7",
      }),
    ).toEqual({ kind: "turn", page: 4 });
    expect(
      readerBoundaryAction({
        ...base,
        direction: "previous",
        page: 3,
        previousChapterId: "1",
      }),
    ).toEqual({ kind: "turn", page: 2 });
  });

  it("switches to the previous chapter's last page at the first page", () => {
    expect(
      readerBoundaryAction({
        ...base,
        direction: "previous",
        previousChapterId: "4",
      }),
    ).toEqual({ kind: "chapter", chapterId: "4", position: "end" });
  });

  it("switches to the next chapter's first page at the last page", () => {
    expect(
      readerBoundaryAction({
        ...base,
        direction: "next",
        page: 10,
        nextChapterId: "8",
      }),
    ).toEqual({ kind: "chapter", chapterId: "8", position: "start" });
  });

  it("returns to the detail page when there is no adjacent chapter", () => {
    expect(
      readerBoundaryAction({
        ...base,
        direction: "previous",
      }),
    ).toEqual({ kind: "detail" });
    expect(
      readerBoundaryAction({
        ...base,
        direction: "next",
        page: 10,
      }),
    ).toEqual({ kind: "detail" });
  });
});
