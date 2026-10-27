import { effectScope } from "vue";
import { describe, expect, it, vi } from "vitest";
import { usePagedResource } from "./usePagedResource";
import type { PagedResult } from "./usePagedResource";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function createResource(fetch: (offset: number, limit: number) => Promise<PagedResult<number>>) {
  const scope = effectScope();
  const resource = scope.run(() => usePagedResource<number>({ fetch }));
  if (!resource) {
    throw new Error("failed to create paged resource");
  }
  return { resource, scope };
}

describe("usePagedResource", () => {
  it("discards a stale response", async () => {
    const first = deferred<PagedResult<number>>();
    const second = deferred<PagedResult<number>>();
    const fetch = vi
      .fn()
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise);
    const { resource } = createResource(fetch);

    const firstLoad = resource.load();
    const secondLoad = resource.load();

    second.resolve({ total: 2, entries: [2] });
    await secondLoad;

    first.resolve({ total: 1, entries: [1] });
    await firstLoad;

    expect(resource.entries.value).toEqual([2]);
    expect(resource.total.value).toBe(2);
  });

  it("keeps the list and does not surface errors on a silent failure", async () => {
    const fetch = vi
      .fn()
      .mockResolvedValueOnce({ total: 1, entries: [1] })
      .mockRejectedValueOnce(new Error("offline"));
    const { resource } = createResource(fetch);

    await resource.load();
    await resource.load(true);

    expect(resource.entries.value).toEqual([1]);
    expect(resource.error.value).toBeNull();
  });

  it("clamps to the last page and reloads when the total shrinks", async () => {
    const fetch = vi
      .fn()
      .mockResolvedValueOnce({ total: 10, entries: [10] })
      .mockResolvedValueOnce({ total: 10, entries: [10] });
    const { resource } = createResource(fetch);

    resource.page.value = 3;
    await resource.load();

    expect(resource.page.value).toBe(1);
    expect(fetch).toHaveBeenNthCalledWith(1, 100, 50);
    expect(fetch).toHaveBeenNthCalledWith(2, 0, 50);
  });

  it("resets to the first page", async () => {
    const fetch = vi
      .fn()
      .mockResolvedValue({ total: 200, entries: [1] });
    const { resource } = createResource(fetch);

    resource.page.value = 2;
    await resource.reset();

    expect(resource.page.value).toBe(1);
    expect(fetch).toHaveBeenLastCalledWith(0, 50);
  });

  it("invalidates in-flight work when its scope is disposed", async () => {
    const pending = deferred<PagedResult<number>>();
    const fetch = vi.fn().mockReturnValueOnce(pending.promise);
    const { resource, scope } = createResource(fetch);

    const load = resource.load();
    scope.stop();
    pending.resolve({ total: 1, entries: [1] });
    await load;

    expect(resource.entries.value).toEqual([]);
  });
});
