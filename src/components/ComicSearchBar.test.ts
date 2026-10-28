import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import { i18n } from "../i18n";
import { platformList } from "../platforms";
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
  it("declares numeric-id direct open for JM, nhentai and pixiv", () => {
    const numericPlatforms = platformList
      .filter((platform) => platform.numericIdDirectOpen)
      .map((platform) => platform.id);

    expect(numericPlatforms).toEqual(["jm", "nhentai", "pixiv"]);
  });

  it("offers a direct-open action for a numeric work id", async () => {
    for (const platform of ["jm", "nhentai", "pixiv"] as const) {
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
    expect(mountBar("pixiv", "").find(".direct-action").exists()).toBe(false);
  });

  it("does not offer the action on platforms without numeric work ids", () => {
    for (const platform of [
      "manhuagui",
      "copymanga",
      "ehentai",
    ] as const) {
      expect(
        mountBar(platform, "123").find(".direct-action").exists(),
      ).toBe(false);
    }
  });
});
