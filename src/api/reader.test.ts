import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReaderChapter } from "./reader";

const invokeMock = vi.hoisted(() => vi.fn());
const waitMock = vi.hoisted(() => vi.fn());
const localMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("./echStartup", () => ({ waitForEchStartup: waitMock }));
vi.mock("./downloads", () => ({ getDownloadedChapter: localMock }));

import {
  clearReaderChapterCache,
  getCachedReaderChapter,
  getPendingReaderChapter,
  getReaderChapter,
  prefetchReaderChapter,
} from "./reader";

function chapter(id: string, pages = 2): ReaderChapter {
  return {
    title: `Work ${id}`,
    thumb: `https://example.com/${id}/thumb.jpg`,
    chapterId: id,
    chapterTitle: `Chapter ${id}`,
    pages: Array.from({ length: pages }, (_, index) => ({
      url: `https://example.com/${id}/${index}.jpg`,
      width: 800,
      height: 1200,
      localPath: null,
    })),
    previous: null,
    next: null,
  };
}

beforeEach(() => {
  invokeMock.mockReset();
  waitMock.mockReset();
  localMock.mockReset();
  waitMock.mockResolvedValue(undefined);
  localMock.mockResolvedValue(null);
});

describe("getReaderChapter", () => {
  it("prefers a downloaded chapter and never waits for ECH", async () => {
    const local = chapter("local");
    local.pages[0].localPath = "/downloads/nhentai/1/0.jpg";
    localMock.mockResolvedValue(local);

    await expect(getReaderChapter("nhentai", "1", "local")).resolves.toBe(local);

    expect(waitMock).not.toHaveBeenCalled();
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it("falls back to the network when the download index is broken", async () => {
    localMock.mockRejectedValue(new Error("broken index"));
    invokeMock.mockResolvedValue(chapter("online"));

    await expect(getReaderChapter("nhentai", "2", "c1")).resolves.toMatchObject({
      chapterId: "online",
    });

    expect(waitMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("get_reader_chapter", {
      platform: "nhentai",
      id: "2",
      chapterId: "c1",
    });
  });

  it("sends null when the platform has no chapter id", async () => {
    invokeMock.mockResolvedValue(chapter("implicit"));

    await getReaderChapter("jm", "3");

    expect(invokeMock).toHaveBeenCalledWith("get_reader_chapter", {
      platform: "jm",
      id: "3",
      chapterId: null,
    });
  });
});

describe("prefetchReaderChapter", () => {
  it("shares an in-flight prefetch and caches the result", async () => {
    let release!: (value: ReaderChapter) => void;
    invokeMock.mockReturnValue(
      new Promise<ReaderChapter>((resolve) => {
        release = resolve;
      }),
    );

    const first = prefetchReaderChapter("jm", "4", "a");
    const second = prefetchReaderChapter("jm", "4", "a");

    expect(second).toBe(first);
    expect(getPendingReaderChapter("jm", "4", "a")).toBe(first);
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    release(chapter("a"));
    await expect(first).resolves.toMatchObject({ chapterId: "a" });

    expect(getPendingReaderChapter("jm", "4", "a")).toBeNull();
    expect(getCachedReaderChapter("jm", "4", "a")).toMatchObject({
      chapterId: "a",
    });
  });

  it("keeps only the four most recent chapters", async () => {
    invokeMock.mockImplementation(
      async (_command: string, args: { chapterId: string }) =>
        chapter(args.chapterId),
    );

    for (const id of ["c1", "c2", "c3", "c4", "c5"]) {
      await prefetchReaderChapter("jm", "5", id);
    }

    expect(getCachedReaderChapter("jm", "5", "c1")).toBeNull();
    for (const id of ["c2", "c3", "c4", "c5"]) {
      expect(getCachedReaderChapter("jm", "5", id)).not.toBeNull();
    }
  });

  it("clears the pending request after a failure so it can retry", async () => {
    invokeMock
      .mockRejectedValueOnce(new Error("upstream down"))
      .mockResolvedValueOnce(chapter("ok"));

    await expect(prefetchReaderChapter("jm", "6", "x")).rejects.toThrow(
      "upstream down",
    );
    expect(getPendingReaderChapter("jm", "6", "x")).toBeNull();

    await expect(prefetchReaderChapter("jm", "6", "x")).resolves.toMatchObject({
      chapterId: "ok",
    });
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("does not cache chapters that were never prefetched", async () => {
    expect(getCachedReaderChapter("jm", "7", "missing")).toBeNull();
    expect(getCachedReaderChapter("jm", "7")).toBeNull();
    expect(getPendingReaderChapter("jm", "7")).toBeNull();
  });
});

describe("clearReaderChapterCache", () => {
  it("drops resolved chapters after a download is deleted", async () => {
    invokeMock.mockResolvedValue(chapter("c"));

    await prefetchReaderChapter("nhentai", "8", "c");
    expect(getCachedReaderChapter("nhentai", "8", "c")).not.toBeNull();

    clearReaderChapterCache();

    expect(getCachedReaderChapter("nhentai", "8", "c")).toBeNull();
  });
});
