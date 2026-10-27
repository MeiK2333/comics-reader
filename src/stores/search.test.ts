import { flushPromises } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createApp } from "vue";
import type { ComicSearchResult } from "../api/comics";
import { AppError } from "../errors";

const searchComicsMock = vi.hoisted(() => vi.fn());

vi.mock("../api/comics", () => ({ searchComics: searchComicsMock }));

async function createSearchStore() {
  vi.resetModules();
  const [{ setActivePinia }, { default: pinia }, { useSearchStore }] =
    await Promise.all([
      import("pinia"),
      import("./index"),
      import("./search"),
    ]);
  createApp({ render: () => null }).use(pinia);
  setActivePinia(pinia);
  return useSearchStore();
}

function result(total: number): ComicSearchResult {
  return { total, page_size: 20, result: [] };
}

beforeEach(() => {
  searchComicsMock.mockReset();
  searchComicsMock.mockResolvedValue(result(1));
});

describe("search store", () => {
  it("ignores an empty query", async () => {
    const store = await createSearchStore();

    store.beginSearch("nhentai", "   ");
    await flushPromises();

    expect(searchComicsMock).not.toHaveBeenCalled();
    expect(store.active).toBe(false);
    expect(store.query).toBe("");
  });

  it("trims the query and records the platform and page", async () => {
    const store = await createSearchStore();

    store.beginSearch("jm", "  hello  ");
    await flushPromises();

    expect(searchComicsMock).toHaveBeenCalledWith("jm", "hello", 1);
    expect(store.platform).toBe("jm");
    expect(store.query).toBe("hello");
    expect(store.page).toBe(1);
    expect(store.response).toEqual(result(1));
    expect(store.loading).toBe(false);
    expect(store.active).toBe(true);
  });

  it("clears the previous results as soon as a new search starts", async () => {
    const store = await createSearchStore();
    store.beginSearch("nhentai", "one");
    await flushPromises();
    expect(store.response).not.toBeNull();

    let resolveSecond!: (value: ComicSearchResult) => void;
    searchComicsMock.mockReturnValueOnce(
      new Promise<ComicSearchResult>((resolve) => {
        resolveSecond = resolve;
      }),
    );
    store.beginSearch("nhentai", "two");

    expect(store.response).toBeNull();
    expect(store.loading).toBe(true);

    resolveSecond(result(2));
    await flushPromises();

    expect(store.response).toEqual(result(2));
  });

  it("discards a response for a superseded query", async () => {
    const store = await createSearchStore();
    let resolveFirst!: (value: ComicSearchResult) => void;
    searchComicsMock.mockReturnValueOnce(
      new Promise<ComicSearchResult>((resolve) => {
        resolveFirst = resolve;
      }),
    );
    store.beginSearch("nhentai", "first");

    searchComicsMock.mockResolvedValueOnce(result(2));
    store.beginSearch("nhentai", "second");
    await flushPromises();
    expect(store.response?.total).toBe(2);

    resolveFirst(result(1));
    await flushPromises();

    expect(store.response?.total).toBe(2);
  });

  it("ignores invalid and duplicate page changes", async () => {
    const store = await createSearchStore();
    store.beginSearch("nhentai", "x");
    await flushPromises();
    searchComicsMock.mockClear();

    store.changePage(1);
    store.changePage(0);
    await flushPromises();
    expect(searchComicsMock).not.toHaveBeenCalled();

    store.changePage(2);
    expect(store.page).toBe(2);
    await flushPromises();
    expect(searchComicsMock).toHaveBeenCalledWith("nhentai", "x", 2);
  });

  it("clears loading and keeps the error on failure", async () => {
    searchComicsMock.mockRejectedValue(new AppError("network", "offline"));
    const store = await createSearchStore();

    store.beginSearch("nhentai", "x");
    await flushPromises();

    expect(store.error?.code).toBe("network");
    expect(store.response).toBeNull();
    expect(store.loading).toBe(false);
  });
});
