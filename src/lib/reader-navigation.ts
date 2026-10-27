/** Reading-order navigation shared by the reader view and the viewer. */

export interface ResolvedReaderPage {
  page: number;
  /** The URL asked for the last page of the chapter (`?page=last`). */
  startAtEnd: boolean;
}

/**
 * Resolve the `page` query value against a chapter's page count.
 *
 * `last` is an internal chapter-switch value, never a page number: the caller
 * uses `startAtEnd` and then replaces the query with the real page. Any other
 * invalid or out-of-range value clamps to 1..total.
 */
export function resolveReaderPage(
  raw: unknown,
  total: number,
): ResolvedReaderPage {
  const safeTotal = Math.max(1, Math.trunc(total));
  if (raw === "last") {
    return { page: safeTotal, startAtEnd: true };
  }

  const parsed =
    typeof raw === "number"
      ? raw
      : typeof raw === "string"
        ? Number.parseInt(raw, 10)
        : 1;
  const page = Number.isFinite(parsed)
    ? Math.min(Math.max(Math.trunc(parsed), 1), safeTotal)
    : 1;

  return { page, startAtEnd: false };
}

export type ReaderDirection = "previous" | "next";

export interface ReaderBoundaryInput {
  direction: ReaderDirection;
  page: number;
  total: number;
  previousChapterId: string | null;
  nextChapterId: string | null;
}

/**
 * Decide what a page-turn attempt at the current position should do. The
 * viewer only asks at a boundary, but the function also covers a normal turn so
 * every entry point can share the same policy.
 */
export type ReaderBoundaryAction =
  | { kind: "turn"; page: number }
  | {
      kind: "chapter";
      chapterId: string;
      position: "start" | "end";
    }
  | { kind: "detail" };

export function readerBoundaryAction(
  input: ReaderBoundaryInput,
): ReaderBoundaryAction {
  if (input.direction === "previous") {
    if (input.page > 1) {
      return { kind: "turn", page: input.page - 1 };
    }
    if (input.previousChapterId) {
      return {
        kind: "chapter",
        chapterId: input.previousChapterId,
        position: "end",
      };
    }
    return { kind: "detail" };
  }

  if (input.page < input.total) {
    return { kind: "turn", page: input.page + 1 };
  }
  if (input.nextChapterId) {
    return {
      kind: "chapter",
      chapterId: input.nextChapterId,
      position: "start",
    };
  }
  return { kind: "detail" };
}
