import { invokeCommand } from "./client";
import type {
  ComicDetail,
  ComicPlatform,
  ComicSearchResult,
} from "../platforms";

/**
 * Search a platform by keyword. `page` is 1-based.
 */
export async function searchComics(
  platform: ComicPlatform,
  query: string,
  page = 1,
): Promise<ComicSearchResult> {
  return invokeCommand<ComicSearchResult>("search_comics", {
    platform,
    query,
    page,
  });
}

/**
 * Fetch exact comic metadata by the id returned from search.
 */
export async function getComic(
  platform: ComicPlatform,
  id: string,
): Promise<ComicDetail> {
  return invokeCommand<ComicDetail>("get_comic", { platform, id });
}