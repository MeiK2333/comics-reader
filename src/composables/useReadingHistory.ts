import { recordHistory, type HistoryInput } from "../api/history";

/** Everything the reader knows about the current work except the page. */
type HistoryTarget = Omit<HistoryInput, "page">;

const WRITE_DELAY_MS = 600;

/**
 * Persist reading progress without blocking the reader.
 *
 * Page turns are debounced and writes run through a single promise chain, so a
 * slow request cannot land after a newer page and move the saved position
 * backwards. History is auxiliary: failures are ignored instead of surfacing
 * in the reader.
 */
export function useReadingHistory(readTarget: () => HistoryTarget | null) {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let pending: HistoryInput | null = null;
  let queue: Promise<void> = Promise.resolve();

  function enqueue(input: HistoryInput) {
    queue = queue.then(() => recordHistory(input)).catch(() => undefined);
  }

  function build(page: number): HistoryInput | null {
    let target: HistoryTarget | null;
    try {
      target = readTarget();
    } catch {
      // History must never interrupt reading, even if the caller's getter
      // is momentarily inconsistent during a route change.
      return null;
    }
    if (!target || target.totalPages < 1) {
      return null;
    }
    return {
      ...target,
      page: Math.min(Math.max(page, 1), target.totalPages),
    };
  }

  /** Write immediately, e.g. right after a chapter finishes loading. */
  function record(page: number) {
    const input = build(page);
    if (input) {
      enqueue(input);
    }
  }

  /** Write after the reader has stayed on the page briefly. */
  function schedule(page: number) {
    const input = build(page);
    if (!input) {
      return;
    }

    pending = input;
    if (timer !== null) {
      clearTimeout(timer);
    }
    timer = setTimeout(() => {
      timer = null;
      const next = pending;
      pending = null;
      if (next) {
        enqueue(next);
      }
    }, WRITE_DELAY_MS);
  }

  /**
   * Write the pending page now. The snapshot was captured when the page
   * changed, so switching chapters or unmounting cannot record the wrong one.
   */
  function flush() {
    if (timer !== null) {
      clearTimeout(timer);
      timer = null;
    }
    const next = pending;
    pending = null;
    if (next) {
      enqueue(next);
    }
  }

  return { record, schedule, flush };
}
