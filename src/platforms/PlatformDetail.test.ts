import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import { i18n } from "../i18n";
import type { CopymangaDetail, ManhuaguiDetail } from "./index";
import PlatformDetail from "./PlatformDetail.vue";
import copymangaWire from "./copymanga/fixtures/wire.json";
import manhuaguiWire from "./manhuagui/fixtures/wire.json";

function mountDetail(hideChapters?: boolean) {
  i18n.global.locale.value = "en";
  return mount(PlatformDetail, {
    props: {
      detail: manhuaguiWire.detail as ManhuaguiDetail,
      hideChapters,
    },
    global: { stubs: { ComicThumb: true } },
  });
}

function mountCopymangaDetail(hideChapters?: boolean) {
  i18n.global.locale.value = "en";
  return mount(PlatformDetail, {
    props: {
      detail: copymangaWire.detail as CopymangaDetail,
      hideChapters,
    },
    global: { stubs: { ComicThumb: true } },
  });
}

describe("PlatformDetail", () => {
  it("keeps the online chapter list unless the caller hides it", () => {
    expect(mountDetail().findAll(".chapter-item")).toHaveLength(1);
    expect(mountDetail(true).findAll(".chapter-item")).toHaveLength(0);
  });

  it("renders copymanga chapters and hides them on the offline detail", () => {
    expect(mountCopymangaDetail().findAll(".chapter-item")).toHaveLength(2);
    expect(mountCopymangaDetail(true).findAll(".chapter-item")).toHaveLength(0);
  });
});
