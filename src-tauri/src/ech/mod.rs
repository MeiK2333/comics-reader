//! ECH bootstrap, Cloudflare DoH-over-ECH resolution, and handshake probes.
//!
//! Only [`bootstrap`] talks to the caller-provided AliDNS/360 endpoint, and it
//! queries solely `cloudflare-ech.com`. Target A/AAAA resolution and the
//! target TLS connection happen through Cloudflare after that.

mod bootstrap;
mod doh;
mod session;
mod tls;

pub(crate) use bootstrap::EchConfig;
pub(crate) use session::EchSession;
pub(crate) use tls::EchHandshake;

use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::error::{AppError, Result};
use crate::image::HttpClient;

/// Bootstrap endpoints tried in order when the caller does not choose one.
const DEFAULT_BOOTSTRAP_DOHS: [&str; 2] = [
    "https://dns.alidns.com/dns-query",
    "https://doh.360.cn/dns-query",
];

/// Runtime state reported to the settings page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EchStatus {
    pub(crate) enabled: bool,
    pub(crate) ready: bool,
    pub(crate) config_domain: Option<String>,
    pub(crate) bootstrap_doh: Option<String>,
    /// Unix timestamp in seconds; `None` when ECH is disabled.
    pub(crate) expires_at: Option<u64>,
}

/// Fetch the shared `cloudflare-ech.com` ECH config. This is the only step
/// that uses the caller-supplied bootstrap DoH endpoint.
#[tauri::command]
pub(crate) async fn fetch_ech_config(doh_endpoint: String) -> Result<EchConfig> {
    bootstrap::query_ech_config(&doh_endpoint).await
}

/// Bootstrap ECH, resolve `domain` through Cloudflare DoH over ECH, and try an
/// ECH handshake followed by a plain-TLS fallback for non-Cloudflare hosts.
#[tauri::command]
pub(crate) async fn probe_ech_handshake(
    domain: String,
    doh_endpoint: String,
) -> Result<EchHandshake> {
    let session = EchSession::bootstrap(&doh_endpoint).await?;
    session.handshake(&domain).await
}

/// Enable ECH for the shared HTTP client, or drop the runtime when disabled.
///
/// Enabling clears any configured proxy, matching the settings-page rule that
/// ECH and the custom proxy cannot be active together. A bootstrap failure
/// leaves the previous runtime untouched.
#[tauri::command]
pub(crate) async fn set_ech_enabled(
    http: State<'_, HttpClient>,
    enabled: bool,
    bootstrap_doh: Option<String>,
) -> Result<()> {
    if !enabled {
        http.disable_ech()?;
        return Ok(());
    }

    let session = bootstrap_session(bootstrap_doh).await?;
    http.enable_ech(Arc::new(session))
}

/// Report whether ECH is enabled and which config is in use.
#[tauri::command]
pub(crate) fn get_ech_status(http: State<'_, HttpClient>) -> Result<EchStatus> {
    http.ech_status()
}

async fn bootstrap_session(bootstrap_doh: Option<String>) -> Result<EchSession> {
    if let Some(endpoint) = bootstrap_doh {
        return EchSession::bootstrap(&endpoint).await;
    }

    let mut last_error = None;
    for endpoint in DEFAULT_BOOTSTRAP_DOHS {
        match EchSession::bootstrap(endpoint).await {
            Ok(session) => return Ok(session),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error
        .unwrap_or_else(|| AppError::Unexpected("no ECH bootstrap endpoint configured".into())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_ech_status_for_the_frontend() {
        let status = EchStatus {
            enabled: true,
            ready: true,
            config_domain: Some("cloudflare-ech.com".into()),
            bootstrap_doh: Some("https://dns.alidns.com/dns-query".into()),
            expires_at: Some(1_234_567_890),
        };

        let value = serde_json::to_value(status).unwrap();

        assert_eq!(value["enabled"], true);
        assert_eq!(value["ready"], true);
        assert_eq!(value["configDomain"], "cloudflare-ech.com");
        assert_eq!(value["bootstrapDoh"], "https://dns.alidns.com/dns-query");
        assert_eq!(value["expiresAt"], 1_234_567_890_u64);
    }
}
