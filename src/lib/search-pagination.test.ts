import { describe, expect, it } from "vitest";
import type { ComicSearchResult } from "../platforms";
import { searchPageCount } from "./search-pagination";

function result(overrides: Partial<ComicSearchResult> = {}): ComicSearchResult {
  return {
    total: 0,
    page_size: 25,
    result: [],
    ...overrides,
  };
}

describe("searchPageCount", () => {
  it("uses the exact total for platforms that report one", () => {
    expect(searchPageCount(result({ total: 51 }), 1)).toBe(3);
    expect(searchPageCount(result({ total: 0 }), 1)).toBe(1);
    expect(searchPageCount(null, 1)).toBe(1);
  });

  it("expands cursor pagination by exactly one page at a time", () => {
    expect(searchPageCount(result({ has_next: true }), 1)).toBe(2);
    expect(searchPageCount(result({ has_next: true }), 2)).toBe(3);
  });

  it("stops at the current page when the cursor is exhausted", () => {
    expect(searchPageCount(result({ has_next: false }), 1)).toBe(1);
    expect(searchPageCount(result({ has_next: false }), 4)).toBe(4);
  });
});
