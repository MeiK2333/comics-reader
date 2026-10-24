import type { MessageKey } from "../i18n";
import { useAppI18n } from "../i18n";
import type { ErrorCode } from "../errors";
import { toAppError } from "../errors";

/**
 * Catalog entry for every error code. The map is exhaustive so a new Rust
 * `ErrorCode` fails the build here until its copy is added.
 */
const ERROR_MESSAGE_KEYS: Record<ErrorCode, MessageKey> = {
  invalidInput: "errors.invalidInput",
  network: "errors.network",
  notFound: "errors.notFound",
  io: "errors.io",
  json: "errors.json",
  tauri: "errors.tauri",
  unexpected: "errors.unexpected",
  unknown: "errors.unknown",
};

/**
 * Turn anything thrown by a command into a localized message. `details` feed
 * the catalog placeholders; `message` is the technical fallback.
 */
export function useErrorMessage() {
  const { t } = useAppI18n();

  return (error: unknown): string => {
    const appError = toAppError(error);
    return t(ERROR_MESSAGE_KEYS[appError.code], {
      ...appError.details,
      message: appError.message,
    });
  };
}
