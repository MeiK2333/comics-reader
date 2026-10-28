import { describe, expect, it } from "vitest";

import copymangaWire from "../platforms/copymanga/fixtures/wire.json";
import jmWire from "../platforms/jm/fixtures/wire.json";
import manhuaguiWire from "../platforms/manhuagui/fixtures/wire.json";
import nhentaiWire from "../platforms/nhentai/fixtures/wire.json";
import pixivWire from "../platforms/pixiv/fixtures/wire.json";
import type {
  ComicDetail,
  CopymangaDetail,
  JmDetail,
  ManhuaguiDetail,
  NhentaiDetail,
  PixivDetail,
} from "../platforms";
import { createDownloadDetailSnapshot } from "./downloads";

function expectNoChapters(snapshot: ComicDetail) {
  expect(snapshot.chapters).toEqual([]);
}

describe("createDownloadDetailSnapshot", () => {
  it("keeps detail metadata but drops the chapter lists", () => {
    const manhuagui = createDownloadDetailSnapshot(
      manhuaguiWire.detail as ManhuaguiDetail,
    );
    expectNoChapters(manhuagui);
    expect(manhuagui.platform).toBe("manhuagui");
    if (manhuagui.platform === "manhuagui") {
      expect(manhuagui.extra.chapterGroups).toEqual([]);
      expect(manhuagui.extra.categories.length).toBeGreaterThan(0);
    }

    const jm = createDownloadDetailSnapshot(jmWire.detail as JmDetail);
    expectNoChapters(jm);
    expect(jm.platform).toBe("jm");
    if (jm.platform === "jm") {
      expect(jm.extra.chapters).toEqual([]);
      expect(jm.extra.tags.length).toBeGreaterThan(0);
    }

    const nhentai = createDownloadDetailSnapshot(
      nhentaiWire.detail as NhentaiDetail,
    );
    expectNoChapters(nhentai);
    expect(nhentai.platform).toBe("nhentai");
    if (nhentai.platform === "nhentai") {
      expect(nhentai.extra.tags.length).toBeGreaterThan(0);
    }

    const copymanga = createDownloadDetailSnapshot(
      copymangaWire.detail as CopymangaDetail,
    );
    expectNoChapters(copymanga);
    expect(copymanga.platform).toBe("copymanga");
    if (copymanga.platform === "copymanga") {
      expect(copymanga.extra.chapterGroups).toEqual([]);
      expect(copymanga.extra.authors.length).toBeGreaterThan(0);
    }

    const pixiv = createDownloadDetailSnapshot(pixivWire.detail as PixivDetail);
    expectNoChapters(pixiv);
    expect(pixiv.platform).toBe("pixiv");
    if (pixiv.platform === "pixiv") {
      expect(pixiv.extra.tags.length).toBeGreaterThan(0);
    }
  });
});
