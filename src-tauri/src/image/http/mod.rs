//! Shared HTTP client and the platform-agnostic download pipeline.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::error::Result;
use ech::EchRuntime;

use client::{build_client, invalid_proxy};

/// Search and detail pages are small; this is only a guard against a runaway
/// response body.
const MAX_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// The ECH config is refreshed lazily after this long; the deadline is also
/// reported by `get_ech_status`.
const ECH_CONFIG_TTL: Duration = Duration::from_secs(10 * 60);
/// After a failed refresh, wait this long before trying the bootstrap again.
const ECH_REFRESH_RETRY: Duration = Duration::from_secs(60);
/// A host that rejected ECH but accepted plain TLS is retried with ECH after
/// this long, so a server or config change cannot disable ECH indefinitely.
const ECH_FALLBACK_TTL: Duration = Duration::from_secs(10 * 60);

/// Shared HTTP client and the proxy currently applied to it. Building the
/// client lazily keeps startup infallible while still reusing pooled
/// connections across the many images of a chapter.
#[derive(Default)]
pub(crate) struct HttpClient {
    state: Mutex<ClientState>,
    /// Serializes lazy refreshes so concurrent requests do not bootstrap the
    /// same ECH config more than once.
    refresh: tokio::sync::Mutex<()>,
}

#[derive(Default)]
struct ClientState {
    /// Normalized `http://host:port/` URL, or `None` for reqwest's default.
    proxy: Option<String>,
    normal_client: Option<reqwest::Client>,
    ech: Option<EchRuntime>,
    /// Incremented on every enable, disable and successful refresh. A refresh
    /// that started before a state change must not resurrect the old runtime.
    ech_generation: u64,
    /// Incremented when a command starts changing the ECH state. A bootstrap
    /// that finishes after a newer command must be discarded.
    ech_control_generation: u64,
    /// `https://host:port` keys that rejected ECH but accepted plain TLS. The
    /// cache survives an ECH config refresh but is cleared when ECH is toggled.
    ech_fallback_until: HashMap<String, Instant>,
}

impl HttpClient {
    /// Ordinary client, used for HTTP requests and whenever ECH is off.
    pub(super) fn get(&self) -> Result<reqwest::Client> {
        let mut state = self.lock()?;

        if let Some(client) = state.normal_client.as_ref() {
            return Ok(client.clone());
        }

        let client = build_client(state.proxy.as_deref())?;
        state.normal_client = Some(client.clone());
        Ok(client)
    }

    /// Apply the user's proxy. Building eagerly means an address reqwest
    /// cannot use is rejected before it can replace a working proxy.
    pub(crate) fn set_proxy(&self, proxy: Option<HttpProxyConfig>) -> Result<()> {
        let proxy = proxy.map(|proxy| proxy.normalized_url()).transpose()?;
        let mut state = self.lock()?;

        if state.ech.is_some() {
            if proxy.is_some() {
                return Err(invalid_proxy(
                    "cannot enable the HTTP proxy while ECH is enabled",
                ));
            }
            state.proxy = None;
            return Ok(());
        }

        let client = build_client(proxy.as_deref())?;
        state.proxy = proxy;
        state.normal_client = Some(client);
        Ok(())
    }
}

mod client;
mod ech;
mod fetch;

#[cfg(test)]
mod tests;

pub(super) use fetch::{FetchedDocument, download_image, fetch_document, is_mirror_error};

pub(crate) use client::HttpProxyConfig;
