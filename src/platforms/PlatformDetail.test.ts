import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import { i18n } from "../i18n";
import type {
  CopymangaDetail,
  EhentaiDetail,
  ManhuaguiDetail,
  ManwaDetail,
  PixivDetail,
} from "./index";
import PlatformDetail from "./PlatformDetail.vue";
import copymangaWire from "./copymanga/fixtures/wire.json";
import ehentaiWire from "./ehentai/fixtures/wire.json";
import manhuaguiWire from "./manhuagui/fixtures/wire.json";
import manwaWire from "./manwa/fixtures/wire.json";
import pixivWire from "./pixiv/fixtures/wire.json";

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

function mountEhentaiDetail() {
  i18n.global.locale.value = "en";
  return mount(PlatformDetail, {
    props: {
      detail: ehentaiWire.detail as EhentaiDetail,
    },
    global: { stubs: { ComicThumb: true } },
  });
}

function mountPixivDetail() {
  i18n.global.locale.value = "en";
  return mount(PlatformDetail, {
    props: {
      detail: pixivWire.detail as PixivDetail,
    },
    global: { stubs: { ComicThumb: true } },
  });
}

function mountManwaDetail(hideChapters?: boolean) {
  i18n.global.locale.value = "en";
  return mount(PlatformDetail, {
    props: {
      detail: manwaWire.detail as ManwaDetail,
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

  it("renders an E-Hentai detail without a chapter list", () => {
    const wrapper = mountEhentaiDetail();

    expect(wrapper.text()).toContain("E-Hentai Work");
    expect(wrapper.text()).toContain("Doujinshi");
    expect(wrapper.findAll(".chapter-item")).toHaveLength(0);
  });

  it("renders a pixiv detail without a chapter list", () => {
    const wrapper = mountPixivDetail();

    expect(wrapper.text()).toContain("オーズワン");
    expect(wrapper.text()).toContain("Ray");
    expect(wrapper.text()).toContain("Illustration");
    expect(wrapper.text()).toContain("Tags");
    expect(wrapper.findAll(".chapter-item")).toHaveLength(0);
  });

  it("renders manwa chapters and hides them on the offline detail", () => {
    const wrapper = mountManwaDetail();

    expect(wrapper.text()).toContain("水边之夜 (完整版)");
    expect(wrapper.text()).toContain("Subscribers");
    expect(wrapper.text()).toContain("Also known as");
    expect(wrapper.findAll(".chapter-item")).toHaveLength(1);
    expect(mountManwaDetail(true).findAll(".chapter-item")).toHaveLength(0);
  });

  it("collapses a manwa chapter list that is too long to render at once", () => {
    const detail = {
      ...manwaWire.detail,
      chapters: Array.from({ length: 101 }, (_, index) => ({
        id: String(index + 1),
        title: `第${index + 1}话`,
      })),
    } as ManwaDetail;

    i18n.global.locale.value = "en";
    const wrapper = mount(PlatformDetail, {
      props: { detail },
      global: { stubs: { ComicThumb: true } },
    });

    expect(wrapper.text()).toContain("101 chapters");
    expect(wrapper.findAll(".chapter-item")).toHaveLength(0);
  });
});
