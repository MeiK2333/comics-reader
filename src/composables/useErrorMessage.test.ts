import { beforeEach, describe, expect, it } from "vitest";
import { i18n } from "../i18n";
import { useErrorMessage } from "./useErrorMessage";

// Kept in the test on purpose: a new code must be added to src/errors.ts and
// this list, which is the same "five places" reminder the app documents.
const ERROR_CODES = [
  "invalidInput",
  "network",
  "httpStatus",
  "emptyResponse",
  "notImage",
  "tooLarge",
  "notFound",
  "io",
  "json",
  "tauri",
  "unexpected",
  "unknown",
] as const;

beforeEach(() => {
  i18n.global.locale.value = "en";
});

describe("useErrorMessage", () => {
  it("resolves every error code to localized copy", () => {
    const format = useErrorMessage();

    for (const code of ERROR_CODES) {
      const text = format({ code, message: "technical detail" });

      expect(text.length).toBeGreaterThan(0);
      // A missing catalog entry would render the raw key.
      expect(text).not.toContain(`errors.${code}`);
    }
  });

  it("interpolates command details", () => {
    const format = useErrorMessage();

    expect(
      format({
        code: "httpStatus",
        message: "unexpected status",
        details: { status: 503 },
      }),
    ).toBe("The server responded with HTTP 503.");

    expect(
      format({ code: "invalidInput", message: "bad page" }),
    ).toBe("Invalid input: bad page");
  });

  it("falls back to unknown for unrecognized codes", () => {
    expect(useErrorMessage()({ code: "nope", message: "mystery" })).toBe(
      "Unexpected error: mystery",
    );
  });

  it("handles plain errors thrown outside Tauri", () => {
    expect(useErrorMessage()(new Error("exploded"))).toBe(
      "Unexpected error: exploded",
    );
  });
});
