import { describe, expect, it } from "vitest";

import cacheUsageFixture from "../../contracts/cache-usage.json";
import detailFixture from "../../contracts/download-detail.json";
import favoriteGroupsFixture from "../../contracts/favorite-groups.json";
import favoritesFixture from "../../contracts/favorites-page.json";
import historyFixture from "../../contracts/history-page.json";
import logsFixture from "../../contracts/log-snapshot.json";
import fixture from "../../contracts/reader-chapter.json";
import copymangaWire from "../platforms/copymanga/fixtures/wire.json";
import ehentaiWire from "../platforms/ehentai/fixtures/wire.json";
import jmWire from "../platforms/jm/fixtures/wire.json";
import manhuaguiWire from "../platforms/manhuagui/fixtures/wire.json";
import manwaWire from "../platforms/manwa/fixtures/wire.json";
import nhentaiWire from "../platforms/nhentai/fixtures/wire.json";
import pixivWire from "../platforms/pixiv/fixtures/wire.json";
import type {
  ComicDetail,
  ComicSearchResult,
  CopymangaDetail,
  CopymangaSearchItem,
  EhentaiDetail,
  EhentaiSearchItem,
  JmDetail,
  JmSearchItem,
  ManhuaguiDetail,
  ManhuaguiSearchItem,
  ManwaDetail,
  ManwaSearchItem,
  NhentaiDetail,
  NhentaiSearchItem,
  PixivDetail,
  PixivSearchItem,
} from "../platforms";
import type { CacheUsage } from "./cache";
import type { DownloadDetail } from "./downloads";
import type { FavoriteGroupList, FavoritePage } from "./favorites";
import type { HistoryPage } from "./history";
import type { LogSnapshot } from "./logs";
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
const favorites: FavoritePage = {
  ...favoritesFixture,
  entries: favoritesFixture.entries.map((entry) => ({
    ...entry,
    platform: entry.platform === "manhuagui" ? "manhuagui" : "nhentai",
  })),
};
const favoriteGroups: FavoriteGroupList = favoriteGroupsFixture;
const cacheUsage: CacheUsage = cacheUsageFixture;
const logs: LogSnapshot = {
  ...logsFixture,
  entries: logsFixture.entries.map((entry) => ({
    ...entry,
    level: entry.level === "error" ? "error" : "info",
  })),
};

// The per-platform fixtures live next to the platform code. Rust serializes
// the same files, so a shape change must be made on both sides at once.
const search: ComicSearchResult = {
  total: 7,
  page_size: 25,
  result: [
    manhuaguiWire.search as ManhuaguiSearchItem,
    manwaWire.search as ManwaSearchItem,
    jmWire.search as JmSearchItem,
    nhentaiWire.search as NhentaiSearchItem,
    copymangaWire.search as CopymangaSearchItem,
    ehentaiWire.search as EhentaiSearchItem,
    pixivWire.search as PixivSearchItem,
  ],
};
const comicDetails: ComicDetail[] = [
  manhuaguiWire.detail as ManhuaguiDetail,
  manwaWire.detail as ManwaDetail,
  jmWire.detail as JmDetail,
  nhentaiWire.detail as NhentaiDetail,
  copymangaWire.detail as CopymangaDetail,
  ehentaiWire.detail as EhentaiDetail,
  pixivWire.detail as PixivDetail,
];

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

describe("FavoritePage wire contract", () => {
  it("keeps the paged favorite fields", () => {
    expect(Object.keys(favorites)).toEqual(["total", "entries"]);
    expect(Object.keys(favorites.entries[0]).sort()).toEqual([
      "addedAt",
      "groupId",
      "id",
      "platform",
      "thumb",
      "title",
    ]);
    expect(favorites.entries[1].thumb).toBeNull();
    expect(favorites.entries[1].groupId).toBe("group-1");
  });
});

describe("FavoriteGroupList wire contract", () => {
  it("keeps group summaries and the empty default name", () => {
    expect(Object.keys(favoriteGroups)).toEqual(["total", "groups"]);
    expect(Object.keys(favoriteGroups.groups[0]).sort()).toEqual([
      "count",
      "createdAt",
      "id",
      "name",
    ]);
    expect(favoriteGroups.groups[0].name).toBe("");
    expect(favoriteGroups.groups[1].name).toBe("Reading");
  });
});

describe("CacheUsage wire contract", () => {
  it("keeps the cache measurement fields", () => {
    expect(Object.keys(cacheUsage).sort()).toEqual(["bytes", "files"]);
    expect(cacheUsage.files).toBe(3);
    expect(cacheUsage.bytes).toBe(1024);
  });
});

describe("LogSnapshot wire contract", () => {
  it("keeps the log page fields and levels", () => {
    expect(Object.keys(logs)).toEqual(["enabled", "entries", "truncated"]);
    expect(Object.keys(logs.entries[0]).sort()).toEqual([
      "level",
      "message",
      "target",
      "timestamp",
    ]);
    expect(logs.entries[1].level).toBe("error");
    expect(logs.truncated).toBe(true);
  });
});

describe("SearchPage wire contract", () => {
  it("keeps the paged search fields and one extra per platform", () => {
    expect(Object.keys(search)).toEqual(["total", "page_size", "result"]);
    expect(search.result.map((item) => item.platform)).toEqual([
      "manhuagui",
      "manwa",
      "jm",
      "nhentai",
      "copymanga",
      "ehentai",
      "pixiv",
    ]);
    expect(Object.keys(search.result[0]).sort()).toEqual([
      "extra",
      "id",
      "platform",
      "thumb",
      "title",
    ]);
    expect(search.result[0].extra).toHaveProperty("chapterGroups");
    expect(search.result[1].extra).toHaveProperty("authors");
    expect(search.result[2].extra).toHaveProperty("author");
    expect(search.result[3].extra).toHaveProperty("tagIds");
    expect(search.result[4].extra).toHaveProperty("chapterGroups");
    expect(search.result[5].extra).toHaveProperty("languages");
    expect(search.result[6].extra).toHaveProperty("pageCount");
  });
});

describe("ComicDetail wire contract", () => {
  it("keeps one typed detail per platform with core chapters", () => {
    expect(comicDetails.map((detail) => detail.platform)).toEqual([
      "manhuagui",
      "manwa",
      "jm",
      "nhentai",
      "copymanga",
      "ehentai",
      "pixiv",
    ]);
    expect(Object.keys(comicDetails[0])).toEqual([
      "title",
      "thumb",
      "platform",
      "chapters",
      "extra",
    ]);
    expect(comicDetails[0].chapters).toEqual([{ id: "ch-1", title: "第1话" }]);
    expect(comicDetails[1].chapters).toEqual([
      { id: "497238", title: "第01话" },
    ]);
    expect(comicDetails[2].chapters).toEqual([{ id: "c1", title: "Chapter 1" }]);
    expect(comicDetails[3].chapters).toEqual([{ id: null, title: null }]);
    expect(comicDetails[4].chapters).toEqual([
      { id: "b9741745-e439-11ed-9b25-024352452ce0", title: "第01話" },
      { id: "a036001e-ef46-11ed-8221-024352452ce0", title: "第02話" },
    ]);
    expect(comicDetails[5].chapters).toEqual([{ id: null, title: null }]);
    expect(comicDetails[6].chapters).toEqual([{ id: null, title: null }]);
    expect(comicDetails[0].extra).toHaveProperty("chapterGroups");
    expect(comicDetails[1].extra).toHaveProperty("authors");
    expect(comicDetails[2].extra).toHaveProperty("chapters");
    expect(comicDetails[3].extra).toHaveProperty("tags");
    expect(comicDetails[4].extra).toHaveProperty("chapterGroups");
    expect(comicDetails[5].extra).toHaveProperty("token");
    expect(comicDetails[6].extra).toHaveProperty("pageCount");
  });
});