import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CachedImage } from "../../api/images";
import type { ReaderPage } from "../../api/reader";

const fetchMock = vi.hoisted(() => vi.fn());
const getCachedMock = vi.hoisted(() => vi.fn());
const forgetMock = vi.hoisted(() => vi.fn());
const convertMock = vi.hoisted(() =>
  vi.fn((path: string) => `asset://localhost/${path}`),
);

vi.mock("../../api/images", () => ({
  fetchCachedImage: fetchMock,
  getCachedImage: getCachedMock,
  forgetCachedImage: forgetMock,
}));
vi.mock("@tauri-apps/api/core", () => ({ convertFileSrc: convertMock }));

import ReaderPageImage from "./ReaderPageImage.vue";

function image(path: string): CachedImage {
  return {
    path,
    contentType: "image/jpeg",
    byteLength: 10,
    fromCache: false,
    platform: "nhentai",
  };
}

function page(overrides: Partial<ReaderPage> = {}): ReaderPage {
  return {
    url: "https://example.com/1.jpg",
    width: 800,
    height: 1200,
    localPath: null,
    ...overrides,
  };
}

function mountPage(value: ReaderPage) {
  return mount(ReaderPageImage, {
    props: {
      platform: "nhentai",
      page: value,
      pageNumber: 2,
      total: 9,
      imageStyle: {},
    },
  });
}

beforeEach(() => {
  fetchMock.mockReset();
  getCachedMock.mockReset();
  forgetMock.mockReset();
  convertMock.mockClear();

  getCachedMock.mockReturnValue(null);
  fetchMock.mockResolvedValue(image("/cache/default.jpg"));
});

describe("ReaderPageImage", () => {
  it("uses a downloaded page without touching the network", async () => {
    const wrapper = mountPage(
      page({ localPath: "/downloads/nhentai/1/00001.jpg" }),
    );
    await flushPromises();

    expect(fetchMock).not.toHaveBeenCalled();
    expect(convertMock).toHaveBeenCalledWith(
      "/downloads/nhentai/1/00001.jpg",
    );
    expect(wrapper.get("img").attributes("src")).toBe(
      "asset://localhost//downloads/nhentai/1/00001.jpg",
    );
  });

  it("renders an already resolved cache path without a request", async () => {
    getCachedMock.mockReturnValue(image("/cache/nhentai/a.jpg"));

    const wrapper = mountPage(page());
    await flushPromises();

    expect(fetchMock).not.toHaveBeenCalled();
    expect(wrapper.get("img").attributes("src")).toBe(
      "asset://localhost//cache/nhentai/a.jpg",
    );
  });

  it("shows the spinner while a remote page is fetching", async () => {
    let release!: (value: CachedImage) => void;
    fetchMock.mockReturnValue(
      new Promise<CachedImage>((resolve) => {
        release = resolve;
      }),
    );

    const wrapper = mountPage(page());
    await flushPromises();

    expect(wrapper.find('[role="status"]').exists()).toBe(true);
    expect(wrapper.find("img").exists()).toBe(false);

    release(image("/cache/nhentai/slow.jpg"));
    await flushPromises();

    expect(wrapper.find('[role="status"]').exists()).toBe(false);
    expect(wrapper.get("img").attributes("src")).toBe(
      "asset://localhost//cache/nhentai/slow.jpg",
    );
  });

  it("shows an error and retries without a stale request", async () => {
    fetchMock.mockRejectedValueOnce(new Error("404"));

    const wrapper = mountPage(page());
    await flushPromises();

    expect(wrapper.get('[role="alert"]').text()).toContain(
      "Could not load this page.",
    );

    fetchMock.mockResolvedValueOnce(image("/cache/nhentai/retry.jpg"));
    await wrapper.get('[role="alert"] button').trigger("click");
    await flushPromises();

    expect(wrapper.find('[role="alert"]').exists()).toBe(false);
    expect(wrapper.get("img").attributes("src")).toBe(
      "asset://localhost//cache/nhentai/retry.jpg",
    );
  });

  it("drops a bad resolved path after the image fails to decode", async () => {
    getCachedMock.mockReturnValue(image("/cache/nhentai/bad.jpg"));

    const wrapper = mountPage(page());
    await flushPromises();

    await wrapper.get("img").trigger("error");

    expect(forgetMock).toHaveBeenCalledWith(
      "nhentai",
      "https://example.com/1.jpg",
    );
    expect(wrapper.find("img").exists()).toBe(false);
    expect(wrapper.find('[role="alert"]').exists()).toBe(true);
  });
});
