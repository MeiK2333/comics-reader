import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.hoisted(() => vi.fn());
const waitForEchStartupMock = vi.hoisted(() => vi.fn());

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("./echStartup", () => ({
  waitForEchStartup: waitForEchStartupMock,
}));

import { invokeCommand, invokeLocal } from "./client";

beforeEach(() => {
  invokeMock.mockReset();
  waitForEchStartupMock.mockReset();
  invokeMock.mockResolvedValue({ ok: true });
  waitForEchStartupMock.mockResolvedValue(undefined);
});

describe("invokeCommand", () => {
  it("waits for the ECH startup before invoking by default", async () => {
    const result = await invokeCommand("search_comics", { query: "x" });

    expect(waitForEchStartupMock).toHaveBeenCalledOnce();
    expect(invokeMock).toHaveBeenCalledWith("search_comics", { query: "x" });
    expect(result).toEqual({ ok: true });
    expect(
      waitForEchStartupMock.mock.invocationCallOrder[0],
    ).toBeLessThan(invokeMock.mock.invocationCallOrder[0]!);
  });

  it("skips the gate when waitForEch is false", async () => {
    await invokeCommand("get_download", { id: "1" }, { waitForEch: false });

    expect(waitForEchStartupMock).not.toHaveBeenCalled();
    expect(invokeMock).toHaveBeenCalledWith("get_download", { id: "1" });
  });

  it("passes arguments through unchanged", async () => {
    const args = { platform: "nhentai", chapters: [{ id: "c1" }] };
    await invokeCommand("start_download", args);

    expect(invokeMock).toHaveBeenCalledWith("start_download", args);
  });

  it("does not swallow command errors", async () => {
    invokeMock.mockRejectedValue(new Error("ipc failed"));

    await expect(invokeCommand("broken")).rejects.toThrow("ipc failed");
  });
});

describe("invokeLocal", () => {
  it("invokes local index commands without waiting for ECH", async () => {
    await invokeLocal("list_favorites", { offset: 0 });

    expect(waitForEchStartupMock).not.toHaveBeenCalled();
    expect(invokeMock).toHaveBeenCalledWith("list_favorites", { offset: 0 });
  });
});
