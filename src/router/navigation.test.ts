import { describe, expect, it } from "vitest";

import router from "./index";

describe("detail/reader route hierarchy", () => {
  it("nests the online reader under the online detail", () => {
    const resolved = router.resolve({
      name: "comic-reader",
      params: { platform: "nhentai", id: "42", chapterId: "7" },
    });

    expect(resolved.fullPath).toBe("/comic/nhentai/42/read/7");
    expect(resolved.matched.map((record) => record.name)).toEqual([
      "comic-detail",
      "comic-reader",
    ]);
    expect(resolved.meta.detailRoute).toBe("comic-detail");
    expect(resolved.meta.readerRoute).toBe("comic-reader");
  });

  it("nests the downloaded reader under the downloaded detail", () => {
    const resolved = router.resolve({
      name: "download-reader",
      params: { platform: "nhentai", id: "42", chapterId: "7" },
    });

    expect(resolved.fullPath).toBe("/downloads/nhentai/42/read/7");
    expect(resolved.matched.map((record) => record.name)).toEqual([
      "download-detail",
      "download-reader",
    ]);
    expect(resolved.meta.detailRoute).toBe("download-detail");
    expect(resolved.meta.readerRoute).toBe("download-reader");
  });

  it("keeps the two detail trees on separate paths", () => {
    expect(
      router.resolve({
        name: "comic-detail",
        params: { platform: "jm", id: "9" },
      }).fullPath,
    ).toBe("/comic/jm/9");
    expect(
      router.resolve({
        name: "download-detail",
        params: { platform: "jm", id: "9" },
      }).fullPath,
    ).toBe("/downloads/jm/9");
  });
});
