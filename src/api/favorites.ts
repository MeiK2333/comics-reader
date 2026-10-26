import { invoke } from "@tauri-apps/api/core";
import type { ComicPlatform } from "./comics";

/** Built-in group for short-press favorites. Mirrors `favorites::DEFAULT_GROUP_ID`. */
export const DEFAULT_GROUP_ID = "default";

/** One saved work. Mirrors `favorites::FavoriteEntry`. */
export interface FavoriteEntry {
  platform: ComicPlatform;
  id: string;
  title: string;
  thumb: string | null;
  /** Unix timestamp in milliseconds, assigned by Rust. */
  addedAt: number;
  /** Group id, including the built-in `default` group. */
  groupId: string;
}

/** Snapshot sent when the user favorites or moves a work. */
export interface FavoriteInput {
  platform: ComicPlatform;
  id: string;
  title: string;
  thumb: string | null;
  /** Omit to add to the default group while preserving an existing group. */
  groupId?: string | null;
}

/** One page of favorites plus the total number of works in the filter. */
export interface FavoritePage {
  total: number;
  entries: FavoriteEntry[];
}

/** One favorite group plus its current number of works. */
export interface FavoriteGroup {
  id: string;
  /** Empty for the built-in default group; the UI localizes that label. */
  name: string;
  createdAt: number;
  count: number;
}

export interface FavoriteGroupList {
  total: number;
  groups: FavoriteGroup[];
}

/** Read one page of favorites, newest first. `groupId` filters the list. */
export function listFavorites(
  offset = 0,
  limit = 50,
  groupId?: string | null,
): Promise<FavoritePage> {
  return invoke<FavoritePage>("list_favorites", {
    offset,
    limit,
    groupId: groupId ?? null,
  });
}

/** Look up one saved work, including its group. */
export function getFavorite(
  platform: ComicPlatform,
  id: string,
): Promise<FavoriteEntry | null> {
  return invoke<FavoriteEntry | null>("get_favorite", { platform, id });
}

/** Save, refresh or move a work. Returns the persisted row. */
export function addFavorite(entry: FavoriteInput): Promise<FavoriteEntry> {
  return invoke<FavoriteEntry>("add_favorite", { entry });
}

/** Remove a work. Removing an unsaved work is a no-op. */
export function removeFavorite(
  platform: ComicPlatform,
  id: string,
): Promise<void> {
  return invoke("remove_favorite", { platform, id });
}

/** Return every group and the number of works in each. */
export function listFavoriteGroups(): Promise<FavoriteGroupList> {
  return invoke<FavoriteGroupList>("list_favorite_groups");
}

/** Create a custom group. */
export function createFavoriteGroup(name: string): Promise<FavoriteGroup> {
  return invoke<FavoriteGroup>("create_favorite_group", { name });
}

/** Rename a custom group. */
export function renameFavoriteGroup(
  groupId: string,
  name: string,
): Promise<void> {
  return invoke("rename_favorite_group", { groupId, name });
}

/** Delete a custom group; its works move to the default group. */
export function deleteFavoriteGroup(groupId: string): Promise<void> {
  return invoke("delete_favorite_group", { groupId });
}
