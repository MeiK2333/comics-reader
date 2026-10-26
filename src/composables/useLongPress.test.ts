import { mount } from "@vue/test-utils";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { defineComponent } from "vue";
import { useLongPress } from "./useLongPress";

interface Options {
  delay?: number;
  moveThreshold?: number;
  disabled?: () => boolean;
}

function mountButton(options: Options = {}) {
  const onLongPress = vi.fn();
  const onClick = vi.fn();
  const wrapper = mount(
    defineComponent({
      setup() {
        const { handlers, consumeClick } = useLongPress({
          onLongPress,
          ...options,
        });

        function handleClick() {
          if (!consumeClick()) {
            onClick();
          }
        }

        return { handlers, handleClick };
      },
      template: `<button v-on="handlers" @click="handleClick">press</button>`,
    }),
  );

  return {
    wrapper,
    onLongPress,
    onClick,
    button: wrapper.get("button").element as HTMLElement,
  };
}

function pointer(
  type: string,
  init: Partial<PointerEvent> = {},
): Event {
  const event = new Event(type, { bubbles: true, cancelable: true });
  Object.assign(event, {
    isPrimary: true,
    pointerId: 1,
    pointerType: "touch",
    button: 0,
    clientX: 0,
    clientY: 0,
    ...init,
  });
  return event;
}

function click(): MouseEvent {
  return new MouseEvent("click", { bubbles: true, cancelable: true });
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useLongPress", () => {
  it("fires on pointerup after the delay and swallows the next click", async () => {
    const { button, onLongPress, onClick } = mountButton();

    button.dispatchEvent(pointer("pointerdown"));
    await vi.advanceTimersByTimeAsync(499);
    button.dispatchEvent(pointer("pointerup"));
    expect(onLongPress).not.toHaveBeenCalled();

    button.dispatchEvent(pointer("pointerdown"));
    await vi.advanceTimersByTimeAsync(500);
    button.dispatchEvent(pointer("pointerup"));
    expect(onLongPress).toHaveBeenCalledTimes(1);

    button.dispatchEvent(click());
    expect(onClick).not.toHaveBeenCalled();

    button.dispatchEvent(click());
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it("cancels when the pointer moves past the threshold", async () => {
    const { button, onLongPress } = mountButton();

    button.dispatchEvent(pointer("pointerdown"));
    button.dispatchEvent(pointer("pointermove", { clientX: 11 }));
    await vi.advanceTimersByTimeAsync(500);
    button.dispatchEvent(pointer("pointerup"));

    expect(onLongPress).not.toHaveBeenCalled();
  });

  it("tolerates movement within the threshold", async () => {
    const { button, onLongPress } = mountButton();

    button.dispatchEvent(pointer("pointerdown"));
    button.dispatchEvent(pointer("pointermove", { clientX: 10, clientY: 0 }));
    await vi.advanceTimersByTimeAsync(500);
    button.dispatchEvent(pointer("pointerup"));

    expect(onLongPress).toHaveBeenCalledTimes(1);
  });

  it("cancels on pointercancel and pointerleave", async () => {
    const { button, onLongPress } = mountButton();

    button.dispatchEvent(pointer("pointerdown"));
    button.dispatchEvent(pointer("pointercancel"));
    await vi.advanceTimersByTimeAsync(500);
    button.dispatchEvent(pointer("pointerup"));
    expect(onLongPress).not.toHaveBeenCalled();

    button.dispatchEvent(pointer("pointerdown"));
    button.dispatchEvent(pointer("pointerleave"));
    await vi.advanceTimersByTimeAsync(500);
    button.dispatchEvent(pointer("pointerup"));
    expect(onLongPress).not.toHaveBeenCalled();
  });

  it("opens the picker from the context menu", () => {
    const { button, onLongPress } = mountButton();
    const event = new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
    });

    button.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    expect(onLongPress).toHaveBeenCalledTimes(1);
  });

  it("supports Shift+Enter and suppresses the following click", () => {
    const { button, onLongPress, onClick } = mountButton();
    const event = new KeyboardEvent("keydown", {
      key: "Enter",
      shiftKey: true,
      bubbles: true,
      cancelable: true,
    });

    button.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    expect(onLongPress).toHaveBeenCalledTimes(1);

    button.dispatchEvent(click());
    expect(onClick).not.toHaveBeenCalled();
  });

  it("ignores non-primary pointers and non-left mouse buttons", async () => {
    const { button, onLongPress } = mountButton();

    button.dispatchEvent(pointer("pointerdown", { isPrimary: false }));
    await vi.advanceTimersByTimeAsync(500);
    button.dispatchEvent(pointer("pointerup"));

    button.dispatchEvent(
      pointer("pointerdown", { pointerType: "mouse", button: 2 }),
    );
    await vi.advanceTimersByTimeAsync(500);
    button.dispatchEvent(pointer("pointerup"));

    expect(onLongPress).not.toHaveBeenCalled();
  });

  it("does nothing while disabled", async () => {
    const { button, onLongPress } = mountButton({ disabled: () => true });

    button.dispatchEvent(pointer("pointerdown"));
    await vi.advanceTimersByTimeAsync(500);
    button.dispatchEvent(pointer("pointerup"));
    button.dispatchEvent(
      new MouseEvent("contextmenu", { bubbles: true, cancelable: true }),
    );
    button.dispatchEvent(
      new KeyboardEvent("keydown", {
        key: "Enter",
        shiftKey: true,
        bubbles: true,
        cancelable: true,
      }),
    );

    expect(onLongPress).not.toHaveBeenCalled();
  });

  it("clears the timer on unmount", async () => {
    const { wrapper, button, onLongPress } = mountButton();

    button.dispatchEvent(pointer("pointerdown"));
    wrapper.unmount();
    await vi.advanceTimersByTimeAsync(1_000);

    expect(onLongPress).not.toHaveBeenCalled();
  });
});
