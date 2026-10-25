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

use crate::error::Result;

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
