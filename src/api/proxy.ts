import { invokeLocal } from "./client";
import type { ProxySettings } from "../stores/settings";

/** Field-level problems the settings form can show without calling Rust. */
export type ProxyValidationError =
  | "hostRequired"
  | "hostInvalid"
  | "portInvalid";

/**
 * Validate the two proxy form fields the same way the Rust side does, so a
 * typo is reported inline instead of failing the next request.
 */
function proxyUrl(host: string, port: number): string | null {
  const trimmed = host.trim();
  if (!trimmed) {
    return null;
  }

  // Bare IPv6 literals need brackets in a URL; accept the bracketed form too.
  const authority =
    trimmed.includes(":") && !trimmed.startsWith("[")
      ? `[${trimmed}]`
      : trimmed;

  try {
    const url = new URL(`http://${authority}:${port}`);
    // Reject values URL parsing would turn into credentials, a path, or a
    // query instead of a plain proxy address.
    if (
      url.username ||
      url.password ||
      url.pathname !== "/" ||
      url.search ||
      url.hash
    ) {
      return null;
    }
    return url.toString();
  } catch {
    return null;
  }
}

export function validateProxy(
  host: string,
  port: number | null,
): ProxyValidationError | null {
  if (!host.trim()) {
    return "hostRequired";
  }
  if (port === null || !Number.isInteger(port) || port < 1 || port > 65535) {
    return "portInvalid";
  }
  if (!proxyUrl(host, port)) {
    return "hostInvalid";
  }
  return null;
}

/**
 * Push the saved settings to the Rust HTTP client. `null` clears the custom
 * proxy and returns to reqwest's default behaviour.
 */
export function applyProxy(proxy: ProxySettings): Promise<void> {
  return invokeLocal("set_http_proxy", {
    proxy: proxy.enabled
      ? { host: proxy.host.trim(), port: proxy.port }
      : null,
  });
}
