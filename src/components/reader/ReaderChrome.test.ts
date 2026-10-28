import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

const mobile = vi.hoisted(() => ({ value: false }));

vi.mock("../../device", () => ({
  isMobileWebView: () => mobile.value,
}));

import { i18n } from "../../i18n";
import ReaderChrome from "./ReaderChrome.vue";

function mountChrome() {
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
      isFullscreen: false,
      percent: 100,
    },
  });
}

function countFullscreenToggles(wrapper: ReturnType<typeof mountChrome>) {
  return wrapper.findAll(
    'button[aria-label="Fullscreen"], button[aria-label="Exit fullscreen"]',
  ).length;
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
