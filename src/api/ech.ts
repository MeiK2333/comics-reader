import { invoke } from "@tauri-apps/api/core";

/** Runtime state reported by `get_ech_status`. */
export interface EchStatus {
  enabled: boolean;
  ready: boolean;
  configDomain: string | null;
  bootstrapDoh: string | null;
  /** Unix timestamp in seconds, or `null` when ECH is disabled. */
  expiresAt: number | null;
}

/**
 * Turn ECH on or off for the Rust HTTP client. Enabling clears the custom
 * proxy; a bootstrap failure leaves the previous runtime untouched.
 */
export function setEchEnabled(
  enabled: boolean,
  bootstrapDoh?: string,
): Promise<void> {
  return invoke("set_ech_enabled", {
    enabled,
    bootstrapDoh: bootstrapDoh ?? null,
  });
}

export function getEchStatus(): Promise<EchStatus> {
  return invoke("get_ech_status");
}

let echStartup: Promise<void> | null = null;

/**
 * Register an in-flight ECH bootstrap. Business commands await this gate so a
 * deep link cannot issue its first request through the ordinary client.
 */
export function beginEchStartup(task: Promise<void>): void {
  const tracked = task
    .catch(() => undefined)
    .finally(() => {
      if (echStartup === tracked) {
        echStartup = null;
      }
    });
  echStartup = tracked;
}

export async function waitForEchStartup(): Promise<void> {
  // Loop in case a newer toggle replaced the promise while we were waiting.
  while (echStartup) {
    await echStartup;
  }
}
