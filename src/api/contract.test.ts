import { describe, expect, it } from "vitest";

import detailFixture from "../../contracts/download-detail.json";
import favoriteGroupsFixture from "../../contracts/favorite-groups.json";
import favoritesFixture from "../../contracts/favorites-page.json";
import historyFixture from "../../contracts/history-page.json";
import logsFixture from "../../contracts/log-snapshot.json";
import fixture from "../../contracts/reader-chapter.json";
import searchFixture from "../../contracts/search-page.json";
import comicDetailsFixture from "../../contracts/comic-details.json";
import type {
  ComicDetail,
  ComicSearchItem,
  ComicSearchResult,
  JmDetailExtra,
  JmExtra,
  ManhuaguiExtra,
  NhentaiDetailExtra,
  NhentaiExtra,
} from "./comics";
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
const logs: LogSnapshot = {
  ...logsFixture,
  entries: logsFixture.entries.map((entry) => ({
    ...entry,
    level: entry.level === "error" ? "error" : "info",
  })),
};
const search: ComicSearchResult = {
  ...searchFixture,
  result: searchFixture.result.map((item): ComicSearchItem => {
    if (item.platform === "manhuagui") {
      return {
        ...item,
        platform: "manhuagui",
        extra: item.extra as ManhuaguiExtra,
      };
    }
    if (item.platform === "jm") {
      return { ...item, platform: "jm", extra: item.extra as JmExtra };
    }
    return {
      ...item,
      platform: "nhentai",
      extra: item.extra as NhentaiExtra,
    };
  }),
};
const comicDetails: ComicDetail[] = comicDetailsFixture.map(
  (detail): ComicDetail => {
    if (detail.platform === "manhuagui") {
      return {
        ...detail,
        platform: "manhuagui",
        extra: detail.extra as ManhuaguiExtra,
      };
    }
    if (detail.platform === "jm") {
      return { ...detail, platform: "jm", extra: detail.extra as JmDetailExtra };
    }
    return {
      ...detail,
      platform: "nhentai",
      extra: detail.extra as NhentaiDetailExtra,
    };
  },
);

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
      "jm",
      "nhentai",
    ]);
    expect(Object.keys(search.result[0]).sort()).toEqual([
      "extra",
      "id",
      "platform",
      "thumb",
      "title",
    ]);
    expect(search.result[0].extra).toHaveProperty("chapterGroups");
    expect(search.result[1].extra).toHaveProperty("author");
    expect(search.result[2].extra).toHaveProperty("tagIds");
  });
});

describe("ComicDetail wire contract", () => {
  it("keeps one typed detail per platform", () => {
    expect(comicDetails.map((detail) => detail.platform)).toEqual([
      "manhuagui",
      "jm",
      "nhentai",
    ]);
    expect(Object.keys(comicDetails[0])).toEqual([
      "title",
      "thumb",
      "platform",
      "extra",
    ]);
    expect(comicDetails[0].extra).toHaveProperty("chapterGroups");
    expect(comicDetails[1].extra).toHaveProperty("chapters");
    expect(comicDetails[2].extra).toHaveProperty("tags");
  });
});
