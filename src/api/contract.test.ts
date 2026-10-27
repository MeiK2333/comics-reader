import { describe, expect, it } from "vitest";

import detailFixture from "../../contracts/download-detail.json";
import historyFixture from "../../contracts/history-page.json";
import fixture from "../../contracts/reader-chapter.json";
import type { DownloadDetail } from "./downloads";
import type { HistoryPage } from "./history";
import type { ReaderChapter } from "./reader";

// The JSON fixture is shared with the Rust contract test. If either side
// changes a wire field, one of the two tests fails until the fixture and the
// types are brought back in sync.
const chapter = fixture satisfies ReaderChapter;
const detail: DownloadDetail = {
  ...detailFixture,
  platform: "manhuagui",
  status: "partial",
};
const history: HistoryPage = {
  ...historyFixture,
  entries: historyFixture.entries.map((entry) => ({
    ...entry,
    platform: entry.platform === "manhuagui" ? "manhuagui" : "nhentai",
  })),
};

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

  it("keeps flattened download summary and chapter fields", () => {
    expect(Object.keys(detail)).toContain("coverPath");
    expect(Object.keys(detail)).toContain("progress");
    expect(Object.keys(detail.chapters[0]).sort()).toEqual([
      "completed",
      "id",
      "pageCount",
      "title",
    ]);
    expect(detail.chapters[1].completed).toBe(false);
  });

  it("keeps history entries and their null chapter fields", () => {
    expect(Object.keys(history)).toEqual(["total", "entries"]);
    expect(history.entries[0].chapterId).toBeNull();
    expect(history.entries[1].chapterTitle).toBe("Chapter 7");
    expect(history.entries[1].updatedAt).toBe(1700000001000);
  });
});
