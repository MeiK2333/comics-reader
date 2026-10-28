import type { ComicSearchResult } from "../platforms";

/**
 * Number of pages the numeric pagination may show.
 *
 * Cursor-paginated platforms have no total; `has_next` lets the UI expose
 * exactly one additional page while keeping the existing numeric control.
 */
export function searchPageCount(
  response: ComicSearchResult | null,
  page: number,
): number {
  if (!response) {
    return 1;
  }
  if (response.has_next !== undefined) {
    return response.has_next ? page + 1 : page;
  }
  return Math.max(
    1,
    Math.ceil(response.total / Math.max(1, response.page_size)),
  );
}
