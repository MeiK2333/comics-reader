import { DEFAULT_GROUP_ID, type FavoriteGroup } from "../../api/favorites";
import type { MessageKey } from "../../i18n";

/** Longest accepted group name; mirrors the inputs' `maxlength`. */
export const GROUP_NAME_MAX_LENGTH = 40;

export type GroupNameIssue = "required" | "tooLong" | "taken";

/** Copy for each validation failure; callers resolve it through `t()`. */
export const GROUP_NAME_ISSUE_KEYS: Record<GroupNameIssue, MessageKey> = {
  required: "favorites.group.nameRequired",
  tooLong: "favorites.group.nameTooLong",
  taken: "favorites.group.nameTaken",
};

/**
 * Validate an already trimmed group name against the loaded groups.
 *
 * `currentGroupId` is the group being renamed, so its own name is not treated
 * as a duplicate. The built-in default group cannot be shadowed by a custom
 * name either, which is why the localized label is passed in rather than
 * compared against the empty name the backend stores.
 */
export function groupNameIssue(
  name: string,
  groups: readonly Pick<FavoriteGroup, "id" | "name">[],
  currentGroupId: string | null,
  defaultGroupLabel: string,
): GroupNameIssue | null {
  if (!name) {
    return "required";
  }
  if (name.length > GROUP_NAME_MAX_LENGTH) {
    return "tooLong";
  }

  const lowered = name.toLocaleLowerCase();
  if (lowered === defaultGroupLabel.toLocaleLowerCase()) {
    return "taken";
  }

  const taken = groups.some(
    (group) =>
      group.id !== DEFAULT_GROUP_ID &&
      group.id !== currentGroupId &&
      group.name.toLocaleLowerCase() === lowered,
  );
  return taken ? "taken" : null;
}
