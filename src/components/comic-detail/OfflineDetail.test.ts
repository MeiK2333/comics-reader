import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import type { DownloadDetail } from "../../api/downloads";
import { createDownloadDetailSnapshot } from "../../api/downloads";
import { i18n } from "../../i18n";
import type { ManhuaguiDetail } from "../../platforms";
import manhuaguiWire from "../../platforms/manhuagui/fixtures/wire.json";
import OfflineDetail from "./OfflineDetail.vue";

function download(): DownloadDetail {
  return {
    platform: "manhuagui",
    id: "42",
    title: "Work",
    thumb: "https://example.com/cover.jpg",
    coverPath: null,
    status: "completed",
    totalChapters: 2,
    completedChapters: 2,
    failedChapters: 0,
    downloadedPages: 20,
    totalBytes: 200,
    readChapterId: "1",
    addedAt: 1,
    updatedAt: 2,
    error: null,
    progress: null,
    chapters: [
      { id: "1", title: "Chapter 1", pageCount: 10, completed: true },
      { id: "2", title: "Chapter 2", pageCount: 10, completed: true },
    ],
    detail: createDownloadDetailSnapshot(
      manhuaguiWire.detail as ManhuaguiDetail,
    ),
  };
}

describe("OfflineDetail", () => {
  it("renders the stored platform metadata next to the offline chapters", () => {
    i18n.global.locale.value = "en";

    const wrapper = mount(OfflineDetail, {
      props: { detail: download() },
      global: { stubs: { ComicThumb: true } },
    });

    expect(wrapper.text()).toContain("Work");
    // Categories, authors and the description come from the stored snapshot,
    // through the same platform component as the online detail.
    expect(wrapper.text()).toContain("Categories");
    expect(wrapper.text()).toContain("Authors");
    expect(wrapper.findAll(".chapter-item")).toHaveLength(2);
    expect(wrapper.text()).toContain("Chapter 1");
  });

  it("emits the stored chapter id from the offline chapter grid", async () => {
    i18n.global.locale.value = "en";

    const wrapper = mount(OfflineDetail, {
      props: { detail: download() },
      global: { stubs: { ComicThumb: true } },
    });

    await wrapper.findAll(".chapter-item")[1]?.trigger("click");

    expect(wrapper.emitted("openChapter")).toEqual([["2"]]);
  });
});
