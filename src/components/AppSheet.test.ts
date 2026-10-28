import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { VueWrapper } from "@vue/test-utils";

const device = vi.hoisted(() => ({ mobile: false }));

vi.mock("../device", () => ({
  isMobileWebView: () => device.mobile,
}));

import AppSheet from "./AppSheet.vue";

// Both branches teleport to <body> (the sheet via NDrawer, the dialog via
// NModal), so the assertions look at the document rather than the wrapper.
let wrapper: VueWrapper | null = null;

function mountSheet() {
  wrapper = mount(AppSheet, {
    props: { show: true, title: "Groups" },
    slots: {
      default: '<p class="sheet-content">content</p>',
      footer: '<button class="sheet-action">ok</button>',
    },
    attachTo: document.body,
  });
  return wrapper;
}

beforeEach(() => {
  device.mobile = false;
});

afterEach(() => {
  wrapper?.unmount();
  wrapper = null;
});

describe("AppSheet", () => {
  it("uses a bottom sheet on mobile and keeps the actions pinned", () => {
    device.mobile = true;
    mountSheet();

    expect(document.body.querySelector(".n-drawer")).not.toBeNull();
    expect(document.body.querySelector(".n-modal")).toBeNull();
    expect(document.body.querySelector(".app-sheet__panel")).not.toBeNull();
    expect(document.body.querySelector(".sheet-content")).not.toBeNull();
    expect(document.body.querySelector(".app-sheet__footer--sheet")).not.toBeNull();
    expect(document.body.querySelector(".sheet-action")).not.toBeNull();
  });

  it("keeps a centered card dialog on desktop", () => {
    mountSheet();

    expect(document.body.querySelector(".n-modal")).not.toBeNull();
    expect(document.body.querySelector(".n-drawer")).toBeNull();
    expect(document.body.querySelector(".app-sheet__footer--dialog")).not.toBeNull();
  });

  it("renders the title and the close control", () => {
    device.mobile = true;
    mountSheet();

    expect(document.body.textContent).toContain("Groups");
    expect(
      document.body.querySelector(".app-sheet__close")?.getAttribute("aria-label"),
    ).toBe("Close");
  });
});
