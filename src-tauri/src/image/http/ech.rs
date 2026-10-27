//! ECH runtime, refresh and fallback state.

use std::sync::Arc;
use std::time::{Instant, SystemTime};

use crate::ech::{EchSession, EchStatus};
use crate::error::Result;
use crate::logging;

use super::client::{build_client, build_ech_client};
use super::{ECH_CONFIG_TTL, ECH_FALLBACK_TTL, ECH_REFRESH_RETRY, HttpClient};
/// The two clients used while ECH is enabled. Both resolve targets through
/// Cloudflare DoH and ignore any proxy; only the TLS layer differs.
pub(super) struct EchRuntime {
    pub(super) session: Arc<EchSession>,
    pub(super) ech_client: reqwest::Client,
    pub(super) plain_client: reqwest::Client,
    pub(super) expires_at: SystemTime,
    pub(super) refresh_after: SystemTime,
}

impl HttpClient {
    /// Start a user-visible ECH state change. The returned generation must be
    /// passed to [`Self::enable_ech_if_current`] after the (slow) bootstrap so
    /// a newer disable/enable can win the race.
    pub(crate) fn begin_ech_control(&self) -> Result<u64> {
        let mut state = self.lock()?;
        state.ech_control_generation = state.ech_control_generation.wrapping_add(1);
        Ok(state.ech_control_generation)
    }

    pub(super) fn ech_control_is_current(&self, control: u64) -> Result<bool> {
        Ok(self.lock()?.ech_control_generation == control)
    }

    /// Install the ECH runtime if no newer control command has been issued.
    ///
    /// A previously configured proxy is cleared because both ECH clients are
    /// built with `no_proxy`.
    pub(crate) fn enable_ech_if_current(
        &self,
        control: u64,
        session: Arc<EchSession>,
    ) -> Result<()> {
        if !self.ech_control_is_current(control)? {
            return Ok(());
        }

        let resolver = session.resolver();
        let ech_client = build_ech_client(&session, resolver.clone(), true)?;
        let plain_client = build_ech_client(&session, resolver, false)?;
        let normal_client = build_client(None)?;
        let now = SystemTime::now();

        let mut state = self.lock()?;
        if state.ech_control_generation != control {
            return Ok(());
        }
        state.proxy = None;
        state.normal_client = Some(normal_client);
        state.ech_generation = state.ech_generation.wrapping_add(1);
        state.ech_fallback_until.clear();
        state.ech = Some(EchRuntime {
            session,
            ech_client,
            plain_client,
            expires_at: now + ECH_CONFIG_TTL,
            refresh_after: now + ECH_CONFIG_TTL,
        });
        Ok(())
    }

    /// Drop the ECH runtime; the ordinary client is reused on the next send.
    ///
    /// Bumping the control generation also makes any in-flight bootstrap
    /// discard its result instead of resurrecting ECH.
    pub(crate) fn disable_ech(&self) -> Result<()> {
        let mut state = self.lock()?;
        state.ech = None;
        state.ech_fallback_until.clear();
        state.ech_generation = state.ech_generation.wrapping_add(1);
        state.ech_control_generation = state.ech_control_generation.wrapping_add(1);
        Ok(())
    }

    /// Re-bootstrap the shared ECH config once its TTL has elapsed.
    ///
    /// The refresh is lazy and runs on the first HTTPS request after the
    /// deadline. A failed bootstrap keeps the old runtime (it may still work)
    /// and postpones the next attempt.
    pub(super) async fn refresh_ech_if_due(&self) {
        if !self.refresh_is_due() {
            return;
        }

        let _guard = self.refresh.lock().await;

        let Some((generation, bootstrap_doh)) = self.refresh_target() else {
            return;
        };

        let session = match EchSession::bootstrap(&bootstrap_doh).await {
            Ok(session) => Arc::new(session),
            Err(error) => {
                logging::warn("ech", format!("failed to refresh the ECH config: {error}"));
                self.postpone_refresh(generation);
                return;
            }
        };

        if let Err(error) = self.install_refreshed_ech(generation, session) {
            logging::warn(
                "ech",
                format!("failed to install the refreshed ECH runtime: {error}"),
            );
            self.postpone_refresh(generation);
        }
    }

    fn refresh_is_due(&self) -> bool {
        let Ok(state) = self.lock() else {
            return false;
        };
        state
            .ech
            .as_ref()
            .is_some_and(|runtime| runtime.refresh_after <= SystemTime::now())
    }

    fn refresh_target(&self) -> Option<(u64, String)> {
        let state = self.lock().ok()?;
        let runtime = state.ech.as_ref()?;
        if runtime.refresh_after > SystemTime::now() {
            return None;
        }
        Some((
            state.ech_generation,
            runtime.session.bootstrap_doh().to_owned(),
        ))
    }

    fn install_refreshed_ech(&self, generation: u64, session: Arc<EchSession>) -> Result<()> {
        let resolver = session.resolver();
        let ech_client = build_ech_client(&session, resolver.clone(), true)?;
        let plain_client = build_ech_client(&session, resolver, false)?;
        let now = SystemTime::now();

        let mut state = self.lock()?;
        if state.ech_generation != generation || state.ech.is_none() {
            return Ok(());
        }

        state.ech_generation = state.ech_generation.wrapping_add(1);
        state.ech = Some(EchRuntime {
            session,
            ech_client,
            plain_client,
            expires_at: now + ECH_CONFIG_TTL,
            refresh_after: now + ECH_CONFIG_TTL,
        });
        Ok(())
    }

    fn postpone_refresh(&self, generation: u64) {
        let Ok(mut state) = self.lock() else {
            return;
        };
        if state.ech_generation != generation {
            return;
        }
        if let Some(runtime) = state.ech.as_mut() {
            runtime.refresh_after = SystemTime::now() + ECH_REFRESH_RETRY;
        }
    }

    pub(crate) fn ech_status(&self) -> Result<EchStatus> {
        let state = self.lock()?;
        let Some(runtime) = state.ech.as_ref() else {
            return Ok(EchStatus {
                enabled: false,
                ready: false,
                config_domain: None,
                bootstrap_doh: None,
                expires_at: None,
            });
        };

        let expires_at = runtime
            .expires_at
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_secs());

        Ok(EchStatus {
            enabled: true,
            ready: true,
            config_domain: Some(runtime.session.config_domain().to_owned()),
            bootstrap_doh: Some(runtime.session.bootstrap_doh().to_owned()),
            expires_at,
        })
    }

    pub(super) fn ech_fallback_is_active(&self, key: &str) -> Result<bool> {
        let mut state = self.lock()?;
        match state.ech_fallback_until.get(key).copied() {
            Some(until) if until > Instant::now() => Ok(true),
            Some(_) => {
                state.ech_fallback_until.remove(key);
                Ok(false)
            }
            None => Ok(false),
        }
    }

    pub(super) fn remember_ech_fallback(&self, key: &str) -> Result<()> {
        let now = Instant::now();
        let mut state = self.lock()?;
        // Expired keys are only useful for diagnosing that the host was tried;
        // prune them here so a long-running app cannot accumulate stale hosts.
        state.ech_fallback_until.retain(|_, until| *until > now);
        state
            .ech_fallback_until
            .insert(key.to_owned(), now + ECH_FALLBACK_TTL);
        Ok(())
    }

    pub(super) fn forget_ech_fallback(&self, key: &str) {
        if let Ok(mut state) = self.lock() {
            state.ech_fallback_until.remove(key);
        }
    }
}
