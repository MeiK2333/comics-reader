//! Long-lived ECH state: one shared `cloudflare-ech.com` config, one
//! Cloudflare DoH-over-ECH resolver, and per-target handshakes.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use crate::error::{AppError, Result};

use super::bootstrap::{self, EchConfig as DohEchConfig};
use super::doh::CloudflareDoh;
use super::tls::{self, EchHandshake};

/// ALPN used by the reqwest clients. reqwest does not populate ALPN for a
/// preconfigured TLS config, so both h2 and HTTP/1.1 have to be requested here.
const REQWEST_ALPN: &[&[u8]] = &[b"h2", b"http/1.1"];

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

    /// ECH TLS config for reqwest, including the h2 ALPN reqwest cannot add.
    pub(crate) fn reqwest_ech_tls(&self) -> Result<rustls::ClientConfig> {
        tls::build_ech_tls_config_with_alpn(&self.config.ech_config_list, REQWEST_ALPN)
    }

    /// Plain-TLS fallback config for reqwest; DNS still goes through Cloudflare.
    pub(crate) fn reqwest_plain_tls(&self) -> Result<rustls::ClientConfig> {
        tls::build_plain_tls_config_with_alpn(REQWEST_ALPN)
    }

    /// Resolver for the ECH and fallback clients.
    pub(crate) fn resolver(self: &Arc<Self>) -> Arc<CloudflareResolver> {
        Arc::new(CloudflareResolver {
            session: self.clone(),
        })
    }

    pub(crate) fn config_domain(&self) -> &str {
        &self.config.domain
    }

    pub(crate) fn bootstrap_doh(&self) -> &str {
        &self.config.doh_endpoint
    }
}

/// `reqwest::dns::Resolve` adapter over [`EchSession::resolve`].
///
/// Addresses are returned with port 0 so reqwest can substitute the port from
/// the request URL (443 for HTTPS).
pub(crate) struct CloudflareResolver {
    session: Arc<EchSession>,
}

impl CloudflareResolver {
    fn address_list(ips: Vec<IpAddr>) -> Addrs {
        Box::new(ips.into_iter().map(|ip| SocketAddr::new(ip, 0)))
    }
}

impl Resolve for CloudflareResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let session = self.session.clone();
        let domain = name.as_str().to_owned();

        Box::pin(async move {
            let ips = session
                .resolve(&domain)
                .await
                .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> { Box::new(error) })?;
            Ok(CloudflareResolver::address_list(ips))
        })
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

    use std::net::SocketAddr;
    use std::time::Duration;

    #[test]
    fn maps_resolved_addresses_to_port_zero() {
        let addresses: Vec<SocketAddr> =
            CloudflareResolver::address_list(vec![IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)])
                .collect();

        assert_eq!(
            addresses,
            vec![SocketAddr::new(
                IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                0
            )]
        );
    }

    #[test]
    fn session_and_resolver_are_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<EchSession>();
        assert_send_sync::<CloudflareResolver>();
    }

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

    /// Phase 0 spike: prove reqwest can carry the rustls ECH config, resolve
    /// every host through Cloudflare DoH, and fall back to ordinary TLS.
    ///
    /// Run manually:
    /// `cargo test --lib -- --ignored reqwest_client_uses_ech_and_cloudflare_dns --nocapture`.
    #[tokio::test]
    #[ignore = "performs live ECH requests through reqwest"]
    async fn reqwest_client_uses_ech_and_cloudflare_dns() {
        const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/155.0.0.0 Safari/537.36";

        let session = Arc::new(
            EchSession::bootstrap("https://dns.alidns.com/dns-query")
                .await
                .unwrap(),
        );

        let ech_tls = session.reqwest_ech_tls().unwrap();
        let resolver = session.resolver();

        let ech_client = reqwest::Client::builder()
            .use_preconfigured_tls(ech_tls)
            .dns_resolver(resolver.clone())
            .no_proxy()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();

        let response = ech_client
            .get("https://nhentai.net/")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await
            .unwrap();
        println!(
            "nhentai.net: status={} version={:?}",
            response.status(),
            response.version()
        );
        assert_eq!(response.status(), reqwest::StatusCode::OK);

        // An empty ALPN list is not "let reqwest decide": it leaves the
        // connection on HTTP/1.1, which is why the reqwest config must name h2.
        let empty_alpn_tls =
            tls::build_ech_tls_config_with_alpn(&session.config.ech_config_list, &[]).unwrap();
        let http1_client = reqwest::Client::builder()
            .use_preconfigured_tls(empty_alpn_tls)
            .dns_resolver(resolver.clone())
            .no_proxy()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        let response = http1_client
            .get("https://nhentai.net/")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await
            .unwrap();
        println!(
            "nhentai.net without ALPN: status={} version={:?}",
            response.status(),
            response.version()
        );
        // The status is not asserted: nhentai's bot protection answers this
        // HTTP/1.1 request with 403, but the negotiated version proves the
        // empty list never reaches h2.
        assert_eq!(response.version(), reqwest::Version::HTTP_11);

        let error = ech_client
            .get("https://www.qq.com/")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await
            .expect_err("a non-Cloudflare host must reject the shared ECH config");
        println!(
            "www.qq.com ECH failure: is_connect={} is_timeout={} is_request={} error={error}",
            error.is_connect(),
            error.is_timeout(),
            error.is_request()
        );
        let mut source = std::error::Error::source(&error);
        while let Some(cause) = source {
            println!("  caused by: {cause}");
            source = cause.source();
        }
        // reqwest reports this as `kind: Request` but `is_connect()` still
        // walks the source chain to the hyper connect error. Keep the fallback
        // predicate narrow: `is_request()` alone would retry ordinary HTTP
        // failures too.
        assert!(
            error.is_connect() || error.is_timeout(),
            "ECH rejection must be fallback-safe: {error:?}"
        );

        let plain_client = reqwest::Client::builder()
            .use_preconfigured_tls(session.reqwest_plain_tls().unwrap())
            .dns_resolver(resolver)
            .no_proxy()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();
        let response = plain_client
            .get("https://www.qq.com/")
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .send()
            .await
            .unwrap();
        println!(
            "www.qq.com plain TLS: status={} version={:?}",
            response.status(),
            response.version()
        );
        assert_eq!(response.status(), reqwest::StatusCode::OK);
    }
}
