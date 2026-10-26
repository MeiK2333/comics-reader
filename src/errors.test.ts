import { describe, expect, it } from "vitest";
import { AppError, toAppError } from "./errors";

describe("toAppError", () => {
  it("returns an existing AppError unchanged", () => {
    const original = new AppError("network", "offline", { status: 1 });

    expect(toAppError(original)).toBe(original);
  });

  it("normalizes the serialized command payload", () => {
    const error = toAppError({
      code: "httpStatus",
      message: "unexpected status",
      details: { status: 404, url: "https://example.com/a" },
    });

    expect(error).toBeInstanceOf(AppError);
    expect(error.code).toBe("httpStatus");
    expect(error.message).toBe("unexpected status");
    expect(error.details).toEqual({
      status: 404,
      url: "https://example.com/a",
    });
  });

  it("maps unknown codes to the local fallback", () => {
    const error = toAppError({ code: "brandNewCode", message: "boom" });

    expect(error.code).toBe("unknown");
    expect(error.message).toBe("boom");
  });

  it("keeps only flat string and number details", () => {
    const error = toAppError({
      code: "notImage",
      message: "bad payload",
      details: {
        status: 200,
        contentType: "text/html",
        nested: { a: 1 },
        list: [1, 2],
        flag: true,
        nothing: null,
      },
    });

    expect(error.details).toEqual({ status: 200, contentType: "text/html" });
  });

  it("rejects payloads without a code or message", () => {
    expect(toAppError({ code: 1, message: "x" }).code).toBe("unknown");
    expect(toAppError({ code: "io" }).code).toBe("unknown");
    expect(toAppError(null).code).toBe("unknown");
  });

  it("wraps a thrown Error", () => {
    const error = toAppError(new Error("kaput"));

    expect(error.code).toBe("unknown");
    expect(error.message).toBe("kaput");
    expect(error.details).toEqual({});
  });

  it("stringifies values that are not errors", () => {
    expect(toAppError("nope").message).toBe("nope");
    expect(toAppError(42).message).toBe("42");
  });
});
