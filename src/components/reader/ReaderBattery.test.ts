import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import ReaderBattery from "./ReaderBattery.vue";

function mountBattery(level: number) {
  return mount(ReaderBattery, {
    props: { level, label: `Battery ${level}%` },
  });
}

describe("ReaderBattery", () => {
  it("renders the localized label and percentage", () => {
    const wrapper = mountBattery(85);

    expect(wrapper.attributes("aria-label")).toBe("Battery 85%");
    expect(wrapper.attributes("title")).toBe("Battery 85%");
    expect(wrapper.get(".reader-battery__value").text()).toBe("85%");
  });

  it("maps the level onto a 1..14 unit fill", () => {
    const cases: Array<[level: number, width: string]> = [
      [-20, "1"],
      [0, "1"],
      [2, "1"],
      [50, "7"],
      [85, "12"],
      [100, "14"],
      [140, "14"],
    ];

    for (const [level, width] of cases) {
      const wrapper = mountBattery(level);
      expect(wrapper.findAll("rect")[1]?.attributes("width")).toBe(width);
      wrapper.unmount();
    }
  });
});
