import { getCurrentScope, onScopeDispose } from "vue";

/**
 * Guard async work against a newer request.
 *
 * Call `begin()` before an async operation and check `isCurrent(token)` after
 * every await that writes component state. Disposing the owning scope (for
 * example, unmounting the view) invalidates the current token automatically, so
 * a late response cannot write into a page the user already left.
 */
export interface LatestRequest {
  /** Start a new request and return its token. */
  begin: () => number;
  /** Whether `token` still belongs to the newest request. */
  isCurrent: (token: number) => boolean;
  /** Current token without starting or invalidating a request (for polling). */
  snapshot: () => number;
  /** Reject every in-flight token, e.g. after a route change. */
  invalidate: () => void;
}

export function useLatestRequest(): LatestRequest {
  let current = 0;

  if (getCurrentScope()) {
    onScopeDispose(() => {
      current += 1;
    });
  }

  return {
    begin: () => {
      current += 1;
      return current;
    },
    isCurrent: (token: number) => token === current,
    snapshot: () => current,
    invalidate: () => {
      current += 1;
    },
  };
}
