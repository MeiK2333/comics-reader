import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("./echStartup", () => ({ waitForEchStartup: vi.fn() }));

import { clearImageCache, getImageCacheUsage } from "./cache";

beforeEach(() => {
  invokeMock.mockReset();
});

describe("image cache commands", () => {
  it("reads the current usage without waiting for ECH", async () => {
    const usage = { files: 4, bytes: 2048 };
    invokeMock.mockResolvedValue(usage);

    await expect(getImageCacheUsage()).resolves.toBe(usage);
    expect(invokeMock).toHaveBeenCalledWith(
      "get_image_cache_usage",
      undefined,
    );
  });

  it("returns what the clear command freed", async () => {
    const freed = { files: 4, bytes: 2048 };
    invokeMock.mockResolvedValue(freed);

    await expect(clearImageCache()).resolves.toBe(freed);
    expect(invokeMock).toHaveBeenCalledWith("clear_image_cache", undefined);
  });
});
