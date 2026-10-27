interface NavigationEntry {
  position: number;
  fullPath: string;
}

// Vue Router records a monotonically increasing `position` in history.state.
// Keeping the visited full paths by position lets a back button target a
// component parent even when another page sits between it and the current
// page (for example detail -> downloads -> home).
const entries = new Map<number, NavigationEntry>();
let currentPosition: number | null = null;

export function recordNavigation(fullPath: string, position: unknown) {
  if (
    typeof position !== "number" ||
    !Number.isFinite(position) ||
    position < 0
  ) {
    return;
  }

  // A push after going back discards the old forward branch.
  for (const key of entries.keys()) {
    if (key > position) {
      entries.delete(key);
    }
  }
  entries.set(position, { position, fullPath });
  currentPosition = position;
}

/**
 * How many history steps are needed to reach `fullPath` before the current
 * entry. `null` means the target is not in the recorded app stack, so the
 * caller should replace instead of guessing with `router.back()`.
 */
export function backDeltaTo(fullPath: string): number | null {
  if (currentPosition === null) {
    return null;
  }

  let best: number | null = null;
  for (const [position, entry] of entries) {
    if (
      position < currentPosition &&
      entry.fullPath === fullPath &&
      (best === null || position > best)
    ) {
      best = position;
    }
  }

  return best === null ? null : best - currentPosition;
}

/** Test hook: the stack only matters for the lifetime of the app process. */
export function resetNavigationStack() {
  entries.clear();
  currentPosition = null;
}
