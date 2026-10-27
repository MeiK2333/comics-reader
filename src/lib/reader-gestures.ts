import type { ReaderDirection } from "../stores/settings";

/** Fraction of the stage width covered by each side tap zone. */
export const TAP_ZONE_RATIO = 1 / 3;

/** How far the track may follow a drag that has already hit a chapter edge. */
export const BOUNDARY_DRAG_RATIO = 0.18;

export type TapZone = "left" | "middle" | "right";
export type TapSide = Exclude<TapZone, "middle">;

/**
 * Map a horizontal position inside the stage (0..1) onto a tap zone. Values
 * exactly on a boundary belong to the middle zone, matching the original
 * `ratio < 1/3` / `ratio > 2/3` checks.
 */
export function tapZone(ratio: number): TapZone {
  if (ratio < TAP_ZONE_RATIO) {
    return "left";
  }
  if (ratio > 1 - TAP_ZONE_RATIO) {
    return "right";
  }
  return "middle";
}

/**
 * Whether a horizontal/vertical drag advances the reading direction. ltr and
 * ttb advance on a negative delta; rtl advances on a positive horizontal delta.
 */
export function isForwardGesture(
  direction: ReaderDirection,
  horizontal: boolean,
  primaryDelta: number,
): boolean {
  if (!horizontal) {
    return primaryDelta < 0;
  }
  return direction === "rtl" ? primaryDelta > 0 : primaryDelta < 0;
}

/** Whether a side tap advances the reading direction. */
export function tapTurnsForward(
  direction: ReaderDirection,
  side: TapSide,
): boolean {
  return direction === "rtl" ? side === "left" : side === "right";
}

/** Whether the reader is already on the edge the gesture would leave. */
export function atPageBoundary(
  displayPage: number,
  total: number,
  forward: boolean,
): boolean {
  return forward ? displayPage >= total : displayPage <= 1;
}

/**
 * Page reached by a completed swipe, or `null` when it would leave the
 * chapter. The caller turns the boundary case into a chapter/detail action.
 */
export function swipeTarget(
  displayPage: number,
  total: number,
  forward: boolean,
): number | null {
  const target = displayPage + (forward ? 1 : -1);
  return target >= 1 && target <= total ? target : null;
}

/**
 * Limit for the track offset while dragging. A normal drag follows the
 * pointer; one that is already leaving the chapter shows a short rubber band.
 */
export function dragLimit(stageSpan: number, atBoundary: boolean): number {
  return stageSpan * (atBoundary ? BOUNDARY_DRAG_RATIO : 1);
}
