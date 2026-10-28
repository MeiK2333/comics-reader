import { describe, expect, it } from "vitest";
import { GROUP_NAME_MAX_LENGTH, groupNameIssue } from "./favorite-group-name";

const groups = [
  { id: "default", name: "" },
  { id: "g1", name: "Travel" },
  { id: "g2", name: "Cooking" },
];

describe("groupNameIssue", () => {
  it("requires a name", () => {
    expect(groupNameIssue("", groups, null, "Default")).toBe("required");
  });

  it("caps the length", () => {
    expect(
      groupNameIssue("a".repeat(GROUP_NAME_MAX_LENGTH), groups, null, "Default"),
    ).toBeNull();
    expect(
      groupNameIssue(
        "a".repeat(GROUP_NAME_MAX_LENGTH + 1),
        groups,
        null,
        "Default",
      ),
    ).toBe("tooLong");
  });

  it("rejects the localized default label, case-insensitively", () => {
    expect(groupNameIssue("default", groups, null, "Default")).toBe("taken");
    expect(groupNameIssue("DEFAULT", groups, null, "Default")).toBe("taken");
  });

  it("rejects a duplicate custom name, case-insensitively", () => {
    expect(groupNameIssue("travel", groups, null, "Default")).toBe("taken");
  });

  it("lets a group keep its own name while renaming", () => {
    expect(groupNameIssue("Travel", groups, "g1", "Default")).toBeNull();
  });

  it("rejects renaming onto another group's name", () => {
    expect(groupNameIssue("cooking", groups, "g1", "Default")).toBe("taken");
  });
});
