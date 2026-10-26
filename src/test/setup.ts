import { afterEach } from "vitest";
import { clearMocks } from "@tauri-apps/api/mocks";

// jsdom does not ship WebCrypto. Tauri's mock IPC builds callback ids with
// `crypto.getRandomValues`, so a minimal implementation is enough here.
// Deliberately not typed against @types/node: installing it would turn the
// `@ts-expect-error` in vite.config.ts into an unused directive and fail
// `vue-tsc`.
if (typeof globalThis.crypto?.getRandomValues !== "function") {
  Object.defineProperty(globalThis, "crypto", {
    configurable: true,
    value: {
      getRandomValues<T extends ArrayBufferView>(array: T): T {
        const bytes = new Uint8Array(
          array.buffer,
          array.byteOffset,
          array.byteLength,
        );
        for (let index = 0; index < bytes.length; index += 1) {
          bytes[index] = Math.floor(Math.random() * 256);
        }
        return array;
      },
    },
  });
}

// Tauri mocks are process-wide and leak between tests; clearing them is
// required, not optional (see the "Mock Tauri APIs" guide).
afterEach(() => {
  clearMocks();
});
