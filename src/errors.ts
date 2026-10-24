/**
 * Error contract shared with `src-tauri/src/error.rs`.
 *
 * Commands reject with this payload instead of a plain string, so the UI can
 * choose localized copy from `code` and still read technical context from
 * `message` / `details`.
 */
export interface ErrorPayload {
  /** Machine-readable kind, e.g. `invalidInput`. */
  code: string;
  /** Technical English description; not meant to be shown verbatim. */
  message: string;
  /** Primitive context such as the offending field name or file size. */
  details?: unknown;
}

/** Error codes emitted by the Rust `ErrorCode` enum, plus the local fallback. */
export type ErrorCode =
  | "invalidInput"
  | "network"
  | "httpStatus"
  | "emptyResponse"
  | "notImage"
  | "tooLarge"
  | "notFound"
  | "io"
  | "json"
  | "tauri"
  | "unexpected"
  | "unknown";

/** Values safe to pass to vue-i18n as message parameters. */
export type ErrorDetails = Record<string, string | number>;

const ERROR_CODES: readonly ErrorCode[] = [
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
];

function isErrorCode(value: string): value is ErrorCode {
  return (ERROR_CODES as readonly string[]).includes(value);
}

function normalizeDetails(value: unknown): ErrorDetails {
  if (typeof value !== "object" || value === null) {
    return {};
  }

  const details: ErrorDetails = {};
  for (const [key, item] of Object.entries(value)) {
    if (typeof item === "string" || typeof item === "number") {
      details[key] = item;
    }
  }
  return details;
}

function isErrorPayload(value: unknown): value is ErrorPayload {
  if (typeof value !== "object" || value === null) {
    return false;
  }

  const payload = value as Partial<ErrorPayload>;
  return typeof payload.code === "string" && typeof payload.message === "string";
}

/**
 * A command failure with a stable code. Extends `Error` so it still behaves
 * like anything else thrown in the app.
 */
export class AppError extends Error {
  constructor(
    readonly code: ErrorCode,
    message: string,
    readonly details: ErrorDetails = {},
  ) {
    super(message);
    this.name = "AppError";
  }
}

/**
 * Normalize a rejection from `invoke` into an `AppError`. Unknown codes map to
 * `unknown` rather than leaking a value the i18n map cannot resolve.
 */
export function toAppError(error: unknown): AppError {
  if (error instanceof AppError) {
    return error;
  }

  if (isErrorPayload(error)) {
    return new AppError(
      isErrorCode(error.code) ? error.code : "unknown",
      error.message,
      normalizeDetails(error.details),
    );
  }

  return new AppError(
    "unknown",
    error instanceof Error ? error.message : String(error),
  );
}
