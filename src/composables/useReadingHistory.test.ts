import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { HistoryInput } from "../api/history";

const recordMock = vi.hoisted(() => vi.fn());

vi.mock("../api/history", () => ({ recordHistory: recordMock }));

import { useReadingHistory } from "./useReadingHistory";

const TARGET: Omit<HistoryInput, "page"> = {
  platform: "nhentai",
  id: "42",
  title: "Gallery",
  thumb: "https://example.com/thumb.jpg",
  chapterId: null,
  chapterTitle: null,
  totalPages: 10,
};

function target(overrides: Partial<Omit<HistoryInput, "page">> = {}) {
  return { ...TARGET, ...overrides };
}

async function flush() {
  await vi.advanceTimersByTimeAsync(0);
}

beforeEach(() => {
  recordMock.mockReset();
  recordMock.mockResolvedValue(undefined);
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("useReadingHistory", () => {
  it("writes immediately with record()", async () => {
    const history = useReadingHistory(() => target());

    history.record(4);
    await flush();

    expect(recordMock).toHaveBeenCalledTimes(1);
    expect(recordMock).toHaveBeenCalledWith(expect.objectContaining({ page: 4 }));
  });

  it("debounces page turns", async () => {
    const history = useReadingHistory(() => target());

    history.schedule(2);
    await vi.advanceTimersByTimeAsync(599);
    expect(recordMock).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(1);
    expect(recordMock).toHaveBeenCalledTimes(1);
  });

  it("only writes the last page of a burst", async () => {
    const history = useReadingHistory(() => target());

    history.schedule(2);
    await vi.advanceTimersByTimeAsync(300);
    history.schedule(3);
    await vi.advanceTimersByTimeAsync(300);
    history.schedule(7);
    await vi.advanceTimersByTimeAsync(600);

    expect(recordMock).toHaveBeenCalledTimes(1);
    expect(recordMock.mock.calls[0]?.[0]).toMatchObject({ page: 7 });
  });

  it("clamps the page into the chapter range", async () => {
    const history = useReadingHistory(() => target({ totalPages: 10 }));

    history.record(99);
    await flush();
    history.record(-3);
    await flush();

    expect(recordMock.mock.calls[0]?.[0]).toMatchObject({ page: 10 });
    expect(recordMock.mock.calls[1]?.[0]).toMatchObject({ page: 1 });
  });

  it("does nothing without a usable target", async () => {
    useReadingHistory(() => null).record(1);
    useReadingHistory(() => target({ totalPages: 0 })).schedule(1);
    await vi.advanceTimersByTimeAsync(1_000);

    expect(recordMock).not.toHaveBeenCalled();
  });

  it("survives a throwing target getter", async () => {
    const getter = () => {
      throw new Error("route is changing");
    };
    const history = useReadingHistory(getter);

    expect(() => history.record(1)).not.toThrow();
    await flush();
    expect(recordMock).not.toHaveBeenCalled();
  });

  it("flushes a pending write without waiting for the timer", async () => {
    const history = useReadingHistory(() => target());

    history.schedule(5);
    history.flush();
    await flush();

    expect(recordMock).toHaveBeenCalledTimes(1);
    expect(recordMock.mock.calls[0]?.[0]).toMatchObject({ page: 5 });

    await vi.advanceTimersByTimeAsync(1_000);
    expect(recordMock).toHaveBeenCalledTimes(1);
  });

  it("serializes writes so an older page cannot land last", async () => {
    let releaseFirst!: () => void;
    recordMock.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        releaseFirst = resolve;
      }),
    );
    const history = useReadingHistory(() => target());

    history.record(1);
    await flush();
    history.record(2);
    await flush();

    expect(recordMock).toHaveBeenCalledTimes(1);

    releaseFirst();
    await flush();

    expect(recordMock).toHaveBeenCalledTimes(2);
    expect(recordMock.mock.calls[1]?.[0]).toMatchObject({ page: 2 });
  });

  it("keeps writing after one failed write", async () => {
    recordMock
      .mockRejectedValueOnce(new Error("disk full"))
      .mockResolvedValueOnce(undefined);
    const history = useReadingHistory(() => target());

    history.record(1);
    await flush();
    history.record(2);
    await flush();

    expect(recordMock).toHaveBeenCalledTimes(2);
  });
});
