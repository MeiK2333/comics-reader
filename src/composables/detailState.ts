import type { ComicDetail } from "../api/comics";
import type { DownloadDetail } from "../api/downloads";
import type { AppError } from "../errors";

/**
 * What the detail page should render. Keeping the priority in one pure
 * function means the online response, the 1.5s offline fallback and the error
 * path cannot disagree about which surface wins.
 */
export type DetailState =
  | { kind: "online"; detail: ComicDetail }
  | { kind: "offline"; download: DownloadDetail }
  | { kind: "error"; error: AppError }
  | { kind: "loading" }
  | { kind: "empty" };

export interface DetailStateInput {
  detail: ComicDetail | null;
  download: DownloadDetail | null;
  error: AppError | null;
  offline: boolean;
  loading: boolean;
}

export function resolveDetailState(input: DetailStateInput): DetailState {
  // A late online response replaces the offline view; prioritize it even if
  // `loading` has not been cleared yet.
  if (input.detail) {
    return { kind: "online", detail: input.detail };
  }
  if (input.offline && input.download) {
    return { kind: "offline", download: input.download };
  }
  if (input.error) {
    return { kind: "error", error: input.error };
  }
  if (input.loading) {
    return { kind: "loading" };
  }
  return { kind: "empty" };
}
