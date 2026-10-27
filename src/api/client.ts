import { invoke } from "@tauri-apps/api/core";

import { waitForEchStartup } from "./echStartup";

export interface InvokeOptions {
  /**
   * Wait for an in-flight ECH bootstrap before calling Rust. Defaults to true
   * for network commands; local index/settings commands should pass false so
   * they stay usable while startup networking is still being configured.
   */
  waitForEch?: boolean;
}

/**
 * Single IPC entry point. This only waits and invokes; it does not normalize
 * errors or add fallback behavior, which stay with the caller.
 */
export async function invokeCommand<T>(
  command: string,
  args?: Record<string, unknown>,
  options: InvokeOptions = {},
): Promise<T> {
  if (options.waitForEch ?? true) {
    await waitForEchStartup();
  }
  return invoke<T>(command, args);
}

/** Local index/settings commands that must work before ECH finishes starting. */
export function invokeLocal<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  return invokeCommand<T>(command, args, { waitForEch: false });
}
