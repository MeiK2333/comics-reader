import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import type { FavoriteGroup } from "../../api/favorites";
import { i18n } from "../../i18n";
import FavoriteGroupBar from "./FavoriteGroupBar.vue";

const groups: FavoriteGroup[] = [
  { id: "default", name: "", createdAt: 1, count: 2 },
  { id: "g1", name: "Travel", createdAt: 2, count: 3 },
];

function mountBar(
  selectedGroupId: string | null = null,
  loading = false,
) {
  i18n.global.locale.value = "en";
  return mount(FavoriteGroupBar, {
    props: { groups, total: 5, selectedGroupId, loading },
  });
}

describe("FavoriteGroupBar", () => {
  it("renders the all chip and every group with its count", () => {
    const chips = mountBar().findAll(".group-bar__chip");

    expect(chips).toHaveLength(3);
    expect(chips[0].text()).toContain("All");
    expect(chips[0].text()).toContain("5");
    expect(chips[1].text()).toContain("Default");
    expect(chips[1].text()).toContain("2");
    expect(chips[2].text()).toContain("Travel");
    expect(chips[2].text()).toContain("3");
  });

  it("exposes the active filter through aria-pressed", () => {
    const chips = mountBar("g1").findAll(".group-bar__chip");

    expect(chips[0].attributes("aria-pressed")).toBe("false");
    expect(chips[1].attributes("aria-pressed")).toBe("false");
    expect(chips[2].attributes("aria-pressed")).toBe("true");
  });

  it("emits the selected group id", async () => {
    const wrapper = mountBar();

    await wrapper.findAll(".group-bar__chip")[2].trigger("click");

    expect(wrapper.emitted("select")).toEqual([["g1"]]);
  });

  it("keeps group actions reachable as labelled icon buttons", async () => {
    const wrapper = mountBar();
    const actions = wrapper.findAll(".group-bar__action");

    expect(actions).toHaveLength(2);
    expect(actions[0].attributes("aria-label")).toBe("New group");
    expect(actions[1].attributes("aria-label")).toBe("Manage groups");

    await actions[0].trigger("click");
    await actions[1].trigger("click");

    expect(wrapper.emitted("create")).toHaveLength(1);
    expect(wrapper.emitted("manage")).toHaveLength(1);
  });

  it("disables the strip while the groups are loading", () => {
    const chips = mountBar(null, true).findAll(".group-bar__chip");

    expect(chips[0].attributes("disabled")).toBeDefined();
  });
});
