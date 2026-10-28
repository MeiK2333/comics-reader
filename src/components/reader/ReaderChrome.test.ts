import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mobile = vi.hoisted(() => ({ value: false }));

vi.mock("../../device", () => ({
  isMobileWebView: () => mobile.value,
}));

import { i18n } from "../../i18n";
import ReaderChrome from "./ReaderChrome.vue";

function mountChrome(overrides: Record<string, unknown> = {}) {
  i18n.global.locale.value = "en";
  return mount(ReaderChrome, {
    props: {
      chromeVisible: true,
      title: "Comic",
      chapterTitle: "Chapter 1",
      counterPage: 1,
      total: 3,
      canPrevious: true,
      canNext: true,
      hasPreviousChapter: false,
      hasNextChapter: false,
      trackAnimating: false,
      boundaryLeaving: false,
      isFullscreen: false,
      percent: 100,
      ...overrides,
    },
  });
}

function countFullscreenToggles(wrapper: ReturnType<typeof mountChrome>) {
  return wrapper.findAll(
    'button[aria-label="Fullscreen"], button[aria-label="Exit fullscreen"]',
  ).length;
}

async function openPageJump(wrapper: ReturnType<typeof mountChrome>) {
  await wrapper.find(".reader-page-jump__button").trigger("click");
  return wrapper.find("input");
}

describe("ReaderChrome fullscreen toggle", () => {
  beforeEach(() => {
    mobile.value = false;
  });

  it("shows the toggle on desktop", () => {
    expect(countFullscreenToggles(mountChrome())).toBe(1);
  });

  it("hides the toggle on mobile, where the window is already fullscreen", () => {
    mobile.value = true;

    expect(countFullscreenToggles(mountChrome())).toBe(0);
  });
});

describe("ReaderChrome page jump", () => {
  it("opens an input prefilled with the current page", async () => {
    const wrapper = mountChrome({ counterPage: 10, total: 100 });

    const input = await openPageJump(wrapper);

    expect(input.exists()).toBe(true);
    expect((input.element as HTMLInputElement).value).toBe("10");
  });

  it("jumps to the page submitted with Enter", async () => {
    const wrapper = mountChrome({ counterPage: 10, total: 100 });
    const input = await openPageJump(wrapper);

    await input.setValue("42");
    await input.trigger("keydown", { key: "Enter" });

    expect(wrapper.emitted("goToPage")).toEqual([[42]]);
  });

  it("clamps out-of-range input to the chapter bounds", async () => {
    const wrapper = mountChrome({ counterPage: 10, total: 100 });
    const input = await openPageJump(wrapper);

    await input.setValue("999");
    await input.trigger("keydown", { key: "Enter" });

    expect(wrapper.emitted("goToPage")).toEqual([[100]]);
  });

  it("cancels with Escape without jumping", async () => {
    const wrapper = mountChrome({ counterPage: 10, total: 100 });
    const input = await openPageJump(wrapper);

    await input.setValue("42");
    await input.trigger("keydown", { key: "Escape" });

    expect(wrapper.emitted("goToPage")).toBeUndefined();
    expect(wrapper.find(".reader-page-jump__button").exists()).toBe(true);
  });

  it("jumps to the entered page when the input loses focus", async () => {
    const wrapper = mountChrome({ counterPage: 10, total: 100 });
    const input = await openPageJump(wrapper);

    await input.setValue("42");
    await input.trigger("blur");

    expect(wrapper.emitted("goToPage")).toEqual([[42]]);
  });

  it("does not jump when the entered page is already shown", async () => {
    const wrapper = mountChrome({ counterPage: 10, total: 100 });
    const input = await openPageJump(wrapper);

    await input.setValue("10");
    await input.trigger("keydown", { key: "Enter" });

    expect(wrapper.emitted("goToPage")).toBeUndefined();
  });

  it("opens the jump input from the HUD page pill", async () => {
    const wrapper = mountChrome({
      chromeVisible: false,
      counterPage: 10,
      total: 100,
    });

    await wrapper
      .find(".viewer__hud-page .reader-page-jump__button")
      .trigger("click");

    const input = wrapper.find(".viewer__hud-jump input");
    expect(input.exists()).toBe(true);
    expect((input.element as HTMLInputElement).value).toBe("10");
  });
});
