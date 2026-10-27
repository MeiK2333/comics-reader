import type { ComicDetail } from "../platforms";
import type { AppError } from "../errors";

/**
 * What the online detail page should render. Downloaded works have their own
 * detail route and never flow through this state: the two data sources are
 * intentionally not merged into one loading/fallback machine.
 */
export type DetailState =
  | { kind: "online"; detail: ComicDetail }
  | { kind: "error"; error: AppError }
  | { kind: "loading" }
  | { kind: "empty" };

export interface DetailStateInput {
  detail: ComicDetail | null;
  error: AppError | null;
  loading: boolean;
}

export function resolveDetailState(input: DetailStateInput): DetailState {
  // A late online response wins even if the loading flag has not cleared yet.
  if (input.detail) {
    return { kind: "online", detail: input.detail };
  }
  if (input.error) {
    return { kind: "error", error: input.error };
  }
  if (input.loading) {
    return { kind: "loading" };
  }
  return { kind: "empty" };
}
