import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import { i18n } from "../i18n";
import type { ComicPlatform } from "../platforms";
import ComicSearchBar from "./ComicSearchBar.vue";

function mountBar(platform: ComicPlatform, query: string) {
  i18n.global.locale.value = "en";
  return mount(ComicSearchBar, {
    props: { platform, query, loading: false },
    global: { stubs: { PlatformPicker: true } },
  });
}

describe("ComicSearchBar direct open", () => {
  it("offers a direct-open action for numeric JM and nhentai ids", async () => {
    for (const platform of ["jm", "nhentai"] as const) {
      const wrapper = mountBar(platform, " 12345 ");
      const action = wrapper.find(".direct-action");

      expect(action.exists()).toBe(true);
      expect(action.attributes("aria-label")).toBe(
        "Open comic 12345 directly",
      );

      await action.trigger("click");
      expect(wrapper.emitted("direct-open")).toEqual([["12345"]]);
      expect(wrapper.emitted("search")).toBeUndefined();
    }
  });

  it("hides the action when the input is not a plain numeric id", () => {
    expect(mountBar("jm", "JM123").find(".direct-action").exists()).toBe(false);
    expect(mountBar("jm", "12a").find(".direct-action").exists()).toBe(false);
    expect(mountBar("nhentai", "").find(".direct-action").exists()).toBe(false);
  });

  it("does not offer the action on other platforms", () => {
    expect(
      mountBar("manhuagui", "123").find(".direct-action").exists(),
    ).toBe(false);
    expect(
      mountBar("copymanga", "123").find(".direct-action").exists(),
    ).toBe(false);
  });
});
