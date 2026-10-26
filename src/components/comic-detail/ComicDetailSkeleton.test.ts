import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import { i18n } from "../../i18n";
import ComicDetailSkeleton from "./ComicDetailSkeleton.vue";

describe("ComicDetailSkeleton", () => {
  it("announces loading and keeps the hero/body layout", () => {
    i18n.global.locale.value = "en";

    const wrapper = mount(ComicDetailSkeleton);

    expect(wrapper.attributes("role")).toBe("status");
    expect(wrapper.attributes("aria-busy")).toBe("true");
    expect(wrapper.attributes("aria-label")).toBe("Loading comic details…");
    expect(wrapper.find(".detail-skeleton__cover").exists()).toBe(true);
    expect(wrapper.findAll(".detail-skeleton__section")).toHaveLength(3);
    // The chapter grid is what makes the skeleton match the loaded detail page
    // instead of collapsing to a generic spinner.
    expect(wrapper.findAll(".detail-skeleton__chapter")).toHaveLength(8);
  });
});
