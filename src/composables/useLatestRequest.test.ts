import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import { defineComponent } from "vue";
import type { LatestRequest } from "./useLatestRequest";
import { useLatestRequest } from "./useLatestRequest";

describe("useLatestRequest", () => {
  it("accepts only the newest token", () => {
    const guard = useLatestRequest();
    const first = guard.begin();
    const second = guard.begin();

    expect(guard.isCurrent(first)).toBe(false);
    expect(guard.isCurrent(second)).toBe(true);
  });

  it("rejects the current token after invalidate", () => {
    const guard = useLatestRequest();
    const token = guard.begin();

    guard.invalidate();

    expect(guard.isCurrent(token)).toBe(false);
  });

  it("snapshots the current token without invalidating it", () => {
    const guard = useLatestRequest();
    const token = guard.begin();
    const snapshot = guard.snapshot();

    expect(snapshot).toBe(token);
    expect(guard.isCurrent(token)).toBe(true);
  });

  it("invalidates in-flight work when the owning scope is disposed", () => {
    let guard: LatestRequest | null = null;
    const wrapper = mount(
      defineComponent({
        setup() {
          guard = useLatestRequest();
          return () => null;
        },
      }),
    );

    const token = guard!.begin();
    wrapper.unmount();

    expect(guard!.isCurrent(token)).toBe(false);
  });
});
