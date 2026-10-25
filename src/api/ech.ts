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
