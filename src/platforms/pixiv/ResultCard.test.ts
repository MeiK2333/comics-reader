import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import { i18n } from "../../i18n";
import PixivResultCard from "./ResultCard.vue";
import pixivWire from "./fixtures/wire.json";
import type { PixivSearchItem } from "./types";

describe("PixivResultCard", () => {
  it("renders the author, type and page count", () => {
    i18n.global.locale.value = "en";
    const wrapper = mount(PixivResultCard, {
      props: { item: pixivWire.search as PixivSearchItem },
      global: { stubs: { ComicThumb: true } },
    });

    expect(wrapper.text()).toContain("by Ray");
    expect(wrapper.text()).toContain("Illustration");
    expect(wrapper.text()).toContain("Pages: 15");
  });
});
