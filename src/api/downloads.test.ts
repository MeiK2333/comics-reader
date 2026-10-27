import { describe, expect, it } from "vitest";

import jmWire from "../platforms/jm/fixtures/wire.json";
import manhuaguiWire from "../platforms/manhuagui/fixtures/wire.json";
import nhentaiWire from "../platforms/nhentai/fixtures/wire.json";
import type {
  ComicDetail,
  JmDetail,
  ManhuaguiDetail,
  NhentaiDetail,
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
  });
});
