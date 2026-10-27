import { describe, expect, it } from "vitest";

import fixture from "../../contracts/reader-chapter.json";
import type { ReaderChapter } from "./reader";

// The JSON fixture is shared with the Rust contract test. If either side
// changes a wire field, one of the two tests fails until the fixture and the
// types are brought back in sync.
const chapter = fixture satisfies ReaderChapter;

describe("ReaderChapter wire contract", () => {
  it("keeps the expected top-level fields", () => {
    expect(Object.keys(chapter).sort()).toEqual([
      "chapterId",
      "chapterTitle",
      "next",
      "pages",
      "previous",
      "thumb",
      "title",
    ]);
  });

  it("keeps page fields, including localPath", () => {
    expect(Object.keys(chapter.pages[0]).sort()).toEqual([
      "height",
      "localPath",
      "url",
      "width",
    ]);
    expect(chapter.pages[1].localPath).toBe(
      "/tmp/downloads/generic/1/12/2.jpg",
    );
  });
});
