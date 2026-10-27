import { invokeCommand } from "./client";

export { beginEchStartup, waitForEchStartup } from "./echStartup";

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
  return invokeCommand(
    "set_ech_enabled",
    {
      enabled,
      bootstrapDoh: bootstrapDoh ?? null,
    },
    { waitForEch: false },
  );
}

export function getEchStatus(): Promise<EchStatus> {
  return invokeCommand("get_ech_status", undefined, { waitForEch: false });
}
