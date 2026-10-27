import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CachedImage } from "./images";

const invokeMock = vi.hoisted(() => vi.fn());
const waitMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("./echStartup", () => ({ waitForEchStartup: waitMock }));

import {
  fetchCachedImage,
  forgetCachedImage,
  getCachedImage,
  prefetchCachedImages,
} from "./images";

let sequence = 0;

function uniqueUrl(label: string): string {
  sequence += 1;
  return `https://example.com/${label}-${sequence}.jpg`;
}

function imageFor(platform: string): CachedImage {
  sequence += 1;
  return {
    path: `/cache/${platform}/${sequence}.jpg`,
    contentType: "image/jpeg",
    byteLength: 3,
    fromCache: false,
    platform,
  };
}

beforeEach(() => {
  invokeMock.mockReset();
  waitMock.mockReset();
  waitMock.mockResolvedValue(undefined);
});

describe("fetchCachedImage", () => {
  it("goes through the ECH gate and passes platform and url", async () => {
    const url = uniqueUrl("pass");
    const image = imageFor("nhentai");
    invokeMock.mockResolvedValue(image);

    await expect(fetchCachedImage("nhentai", url)).resolves.toBe(image);

    expect(waitMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith("fetch_image", {
      url,
      platform: "nhentai",
    });
  });

  it("shares one request between concurrent callers", async () => {
    const url = uniqueUrl("dedupe");
    let release!: (image: CachedImage) => void;
    invokeMock.mockReturnValue(
      new Promise<CachedImage>((resolve) => {
        release = resolve;
      }),
    );

    const first = fetchCachedImage("jm", url);
    const second = fetchCachedImage("jm", url);

    expect(first).toBe(second);
    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(1));

    release(imageFor("jm"));
    await expect(first).resolves.toMatchObject({ platform: "jm" });
  });

  it("serves a resolved path without another command", async () => {
    const url = uniqueUrl("resolved");
    const image = imageFor("nhentai");
    invokeMock.mockResolvedValue(image);

    const first = await fetchCachedImage("nhentai", url);
    const second = await fetchCachedImage("nhentai", url);

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(second).toBe(first);
    expect(getCachedImage("nhentai", url)).toBe(first);
  });

  it("retries a failed request instead of caching the rejection", async () => {
    const url = uniqueUrl("retry");
    invokeMock
      .mockRejectedValueOnce(new Error("network down"))
      .mockResolvedValueOnce(imageFor("nhentai"));

    await expect(fetchCachedImage("nhentai", url)).rejects.toThrow(
      "network down",
    );
    await expect(fetchCachedImage("nhentai", url)).resolves.toMatchObject({
      platform: "nhentai",
    });

    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("drops a resolved path after a decode failure", async () => {
    const url = uniqueUrl("forget");
    invokeMock.mockResolvedValue(imageFor("nhentai"));

    await fetchCachedImage("nhentai", url);
    forgetCachedImage("nhentai", url);

    expect(getCachedImage("nhentai", url)).toBeNull();

    await fetchCachedImage("nhentai", url);
    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it("waits for an in-flight ECH bootstrap before invoking", async () => {
    let releaseEch!: () => void;
    waitMock.mockReturnValue(
      new Promise<void>((resolve) => {
        releaseEch = resolve;
      }),
    );
    invokeMock.mockResolvedValue(imageFor("nhentai"));

    const pending = fetchCachedImage("nhentai", uniqueUrl("gate"));
    await Promise.resolve();

    expect(invokeMock).not.toHaveBeenCalled();

    releaseEch();
    await pending;
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });
});

describe("prefetchCachedImages", () => {
  it("caps concurrency while warming every url", async () => {
    const urls = Array.from({ length: 5 }, () => uniqueUrl("prefetch"));
    const gates: Array<() => void> = [];
    let active = 0;
    let peak = 0;

    invokeMock.mockImplementation(
      () =>
        new Promise<CachedImage>((resolve) => {
          active += 1;
          peak = Math.max(peak, active);
          gates.push(() => {
            active -= 1;
            resolve(imageFor("nhentai"));
          });
        }),
    );

    const preload = prefetchCachedImages("nhentai", urls, 2);

    await vi.waitFor(() => expect(invokeMock).toHaveBeenCalledTimes(2));
    while (gates.length > 0) {
      gates.shift()?.();
      await new Promise((resolve) => setTimeout(resolve, 0));
    }

    await preload;

    expect(invokeMock).toHaveBeenCalledTimes(urls.length);
    expect(peak).toBe(2);
  });

  it("keeps warming after one url fails", async () => {
    const urls = [uniqueUrl("fail"), uniqueUrl("ok")];
    invokeMock
      .mockRejectedValueOnce(new Error("broken"))
      .mockResolvedValueOnce(imageFor("nhentai"));

    await expect(prefetchCachedImages("nhentai", urls, 1)).resolves.toBeUndefined();

    expect(invokeMock).toHaveBeenCalledTimes(2);
  });
});
