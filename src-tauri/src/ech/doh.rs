//! Target DNS resolution through Cloudflare DoH over ECH.
//!
//! After the bootstrap step, Cloudflare is the only resolver used for target
//! A/AAAA records. The fixed addresses below are the Cloudflare DoH anycast
//! endpoints and avoid relying on the local system resolver for
//! `cloudflare-dns.com` itself.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use hickory_resolver::proto::op::Message;
use hickory_resolver::proto::rr::RData;

use crate::error::{AppError, Result};

const CLOUDFLARE_DOH_HOST: &str = "cloudflare-dns.com";
const CLOUDFLARE_DOH_PATH: &str = "/dns-query";
/// Cloudflare DoH addresses supplied by the caller; both are tried by reqwest.
const CLOUDFLARE_DOH_IPS: [Ipv4Addr; 2] = [
    Ipv4Addr::new(104, 16, 248, 249),
    Ipv4Addr::new(104, 16, 249, 249),
];
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const DNS_TYPE_A: u16 = 1;
const DNS_TYPE_AAAA: u16 = 28;

/// Cloudflare DoH client protected by the shared ECH config.
pub(crate) struct CloudflareDoh {
    client: reqwest::Client,
}

impl CloudflareDoh {
    /// Build the DoH client once per ECH session. The URL still uses
    /// `cloudflare-dns.com` as its host, while `resolve_to_addrs` forces the
    /// connection to the fixed Cloudflare DoH IPs.
    pub(crate) fn new(ech_tls: Arc<rustls::ClientConfig>) -> Result<Self> {
        let addrs: Vec<SocketAddr> = CLOUDFLARE_DOH_IPS
            .iter()
            .map(|ip| SocketAddr::new(IpAddr::V4(*ip), 443))
            .collect();

        let client = reqwest::Client::builder()
            .use_preconfigured_tls((*ech_tls).clone())
            .resolve_to_addrs(CLOUDFLARE_DOH_HOST, &addrs)
            .no_proxy()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| {
                AppError::Unexpected(format!(
                    "failed to build the Cloudflare DoH client: {error}"
                ))
            })?;

        Ok(Self { client })
    }

    /// Resolve `domain`'s A and AAAA records over Cloudflare DoH. IPv4
    /// addresses are sorted first so callers can prefer them.
    pub(crate) async fn resolve(&self, domain: &str) -> Result<Vec<IpAddr>> {
        let mut ips = Vec::new();
        let mut last_error = None;

        for query_type in [DNS_TYPE_A, DNS_TYPE_AAAA] {
            match self.query(domain, query_type).await {
                Ok(mut addresses) => ips.append(&mut addresses),
                Err(error) => last_error = Some(error),
            }
        }

        ips.retain(is_usable_ip);
        ips.sort();
        ips.dedup();

        if ips.is_empty() {
            return Err(last_error.unwrap_or_else(|| AppError::Network {
                url: doh_url(),
                reason: format!("Cloudflare DoH returned no usable addresses for {domain}"),
            }));
        }

        Ok(ips)
    }

    async fn query(&self, domain: &str, query_type: u16) -> Result<Vec<IpAddr>> {
        let url = doh_url();
        let response = self
            .client
            .post(&url)
            .header("content-type", "application/dns-message")
            .header("accept", "application/dns-message")
            .body(dns_query(domain, query_type))
            .send()
            .await
            .map_err(|error| AppError::Network {
                url: url.clone(),
                reason: format!("Cloudflare DoH request failed: {error}"),
            })?;

        let status = response.status();
        if !status.is_success() {
            return Err(AppError::HttpStatus {
                url,
                status: status.as_u16(),
            });
        }

        let bytes = response.bytes().await.map_err(|error| AppError::Network {
            url: doh_url(),
            reason: format!("Cloudflare DoH response body failed: {error}"),
        })?;
        let message = Message::from_vec(&bytes)
            .map_err(|error| AppError::Unexpected(format!("invalid DoH response: {error}")))?;

        Ok(message
            .answers
            .iter()
            .filter_map(|record| match &record.data {
                RData::A(ip) => Some(IpAddr::V4(ip.0)),
                RData::AAAA(ip) => Some(IpAddr::V6(ip.0)),
                _ => None,
            })
            .collect())
    }
}

fn doh_url() -> String {
    format!("https://{CLOUDFLARE_DOH_HOST}{CLOUDFLARE_DOH_PATH}")
}

fn dns_query(domain: &str, query_type: u16) -> Vec<u8> {
    let mut query = vec![
        0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    for label in domain.trim_end_matches('.').split('.') {
        query.push(label.len() as u8);
        query.extend_from_slice(label.as_bytes());
    }

    query.push(0);
    query.extend_from_slice(&query_type.to_be_bytes());
    query.extend_from_slice(&1u16.to_be_bytes());
    query
}

fn is_usable_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => !ip.is_unspecified(),
        IpAddr::V6(ip) => {
            if ip.is_unspecified() {
                return false;
            }
            // `::ffff:0.0.0.0` is how some polluted answers encode 0.0.0.0.
            !ip.to_ipv4_mapped().is_some_and(|ip| ip.is_unspecified())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_dns_wire_query() {
        let query = dns_query("nhentai.net", DNS_TYPE_A);

        assert_eq!(&query[..2], &[0x12, 0x34]);
        assert_eq!(u16::from_be_bytes([query[4], query[5]]), 1);
        assert!(query.ends_with(&[0x00, 0x00, 0x01, 0x00, 0x01]));
    }

    #[test]
    fn filters_unspecified_addresses() {
        assert!(!is_usable_ip(&IpAddr::V4(Ipv4Addr::UNSPECIFIED)));
        assert!(!is_usable_ip(&IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED)));
        assert!(is_usable_ip(&IpAddr::V4(Ipv4Addr::new(104, 16, 248, 249))));
    }
}
