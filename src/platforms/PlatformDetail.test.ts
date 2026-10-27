import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import { i18n } from "../i18n";
import type { ManhuaguiDetail } from "./index";
import PlatformDetail from "./PlatformDetail.vue";
import manhuaguiWire from "./manhuagui/fixtures/wire.json";

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

describe("PlatformDetail", () => {
  it("keeps the online chapter list unless the caller hides it", () => {
    expect(mountDetail().findAll(".chapter-item")).toHaveLength(1);
    expect(mountDetail(true).findAll(".chapter-item")).toHaveLength(0);
  });
});
