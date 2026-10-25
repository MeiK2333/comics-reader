//! Long-lived ECH state: one shared `cloudflare-ech.com` config, one
//! Cloudflare DoH-over-ECH resolver, and per-target handshakes.

use std::net::IpAddr;
use std::sync::Arc;

use crate::error::{AppError, Result};

use super::bootstrap::{self, EchConfig as DohEchConfig};
use super::doh::CloudflareDoh;
use super::tls::{self, EchHandshake};

/// The bootstrap DoH endpoint is only consulted once, while constructing this
/// session. All target resolution afterwards goes through Cloudflare DoH.
pub(crate) struct EchSession {
    config: DohEchConfig,
    ech_tls: Arc<rustls::ClientConfig>,
    doh: CloudflareDoh,
}

impl EchSession {
    pub(crate) async fn bootstrap(bootstrap_doh: &str) -> Result<Self> {
        let config = bootstrap::query_ech_config(bootstrap_doh).await?;
        let ech_tls = Arc::new(tls::build_ech_tls_config(&config.ech_config_list)?);
        let doh = CloudflareDoh::new(ech_tls.clone())?;

        Ok(Self {
            config,
            ech_tls,
            doh,
        })
    }

    /// Resolve a target's A/AAAA through Cloudflare DoH over ECH.
    pub(crate) async fn resolve(&self, domain: &str) -> Result<Vec<IpAddr>> {
        let domain = validate_domain(domain)?;
        self.doh.resolve(domain).await
    }

    /// Resolve the target through Cloudflare, then try ECH and finally plain
    /// TLS for non-Cloudflare hosts.
    pub(crate) async fn handshake(&self, domain: &str) -> Result<EchHandshake> {
        let domain = validate_domain(domain)?;
        let ips = self.resolve(domain).await?;
        tls::handshake(domain, &self.config, &self.ech_tls, ips).await
    }
}

fn validate_domain(domain: &str) -> Result<&str> {
    let domain = domain.trim().trim_end_matches('.');
    if domain.is_empty() || domain.parse::<IpAddr>().is_ok() {
        return Err(AppError::InvalidInput {
            field: "domain",
            reason: "must be a DNS name, not an IP address".into(),
        });
    }
    Ok(domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live verification of the whole ECH path. Run manually:
    /// `cargo test --lib -- --ignored performs_ech_handshake`.
    ///
    /// Each bootstrap provider supplies one `cloudflare-ech.com` config, which
    /// is then applied to every target.
    #[tokio::test]
    #[ignore = "performs a public ECH TLS handshake"]
    async fn performs_ech_handshake() {
        for bootstrap_doh in [
            "https://dns.alidns.com/dns-query",
            "https://doh.360.cn/dns-query",
        ] {
            let session = EchSession::bootstrap(bootstrap_doh).await.unwrap();

            for domain in [
                "cloudflare-ech.com",
                "cloudflare-dns.com",
                "crypto.cloudflare.com",
                "www.cloudflare.com",
                "example.com",
            ] {
                let result = session.handshake(domain).await.unwrap();
                println!("{domain} via {bootstrap_doh}: {result:?}");

                assert_eq!(result.config_domain, bootstrap::ECH_DOMAIN);
                assert_eq!(result.ech_status, "accepted");
                assert_eq!(result.tls_version, "TLSv1.3");
                assert!(!result.fallback);
            }
        }
    }

    /// Cloudflare DoH over ECH must resolve ordinary domains without falling
    /// back to AliDNS/360 or system DNS.
    #[tokio::test]
    #[ignore = "queries Cloudflare DoH over ECH"]
    async fn queries_cloudflare_dns_over_ech() {
        let session = EchSession::bootstrap("https://dns.alidns.com/dns-query")
            .await
            .unwrap();

        let ips = session.resolve("example.com").await.unwrap();
        println!("example.com via Cloudflare DoH: {ips:?}");

        assert!(!ips.is_empty());
        assert!(ips.iter().all(|ip| !ip.is_unspecified()));
    }

    /// nhentai.net's local/AliDNS answers are polluted; this must work through
    /// Cloudflare DoH resolution and a direct ECH handshake, with no proxy.
    #[tokio::test]
    #[ignore = "performs a direct ECH handshake to nhentai.net"]
    async fn queries_nhentai_via_cloudflare_doh_over_ech() {
        let session = EchSession::bootstrap("https://dns.alidns.com/dns-query")
            .await
            .unwrap();

        let result = session.handshake("nhentai.net").await.unwrap();
        println!("nhentai.net: {result:?}");

        assert_eq!(result.ech_status, "accepted");
        assert_eq!(result.tls_version, "TLSv1.3");
        assert!(!result.fallback);
        assert_eq!(result.http_status, Some(200));
    }

    /// A non-Cloudflare target rejects the shared ECH config and must still be
    /// reachable through the plain-TLS fallback.
    #[tokio::test]
    #[ignore = "performs a plain TLS fallback handshake"]
    async fn falls_back_to_plain_tls_for_non_cloudflare_site() {
        let session = EchSession::bootstrap("https://dns.alidns.com/dns-query")
            .await
            .unwrap();

        let result = session.handshake("www.qq.com").await.unwrap();
        println!("www.qq.com: {result:?}");

        assert!(result.fallback);
        assert_eq!(result.ech_status, "notOffered");
        assert_eq!(result.tls_version, "TLSv1.3");
    }
}
