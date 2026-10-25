//! Bootstrap ECH configuration from a caller-supplied DoH endpoint.
//!
//! AliDNS/360 are only used here: query `cloudflare-ech.com`'s HTTPS RR and
//! extract the shared ECHConfigList. Target domains are never resolved through
//! this module.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use hickory_resolver::Resolver;
use hickory_resolver::config::{
    ConnectionConfig, NameServerConfig, ResolverConfig, ResolverOpts, ServerOrderingStrategy,
};
use hickory_resolver::net::runtime::TokioRuntimeProvider;
use hickory_resolver::proto::rr::{
    RData, Record, RecordType,
    rdata::svcb::{SvcParamKey, SvcParamValue},
};
use serde::Serialize;
use url::{Host, Url};

use crate::error::{AppError, Result};

/// The HTTPS RRSet that publishes the ECHConfigList used when ECH is enabled.
pub(crate) const ECH_DOMAIN: &str = "cloudflare-ech.com";
/// Path used when the caller passes only `https://host`.
const DEFAULT_DOH_PATH: &str = "/dns-query";
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);

/// One HTTPS record returned by the DoH endpoint.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HttpsRecord {
    pub priority: u16,
    pub target: String,
    /// Base64 ECHConfigList when this record carries the `ech` SvcParam.
    pub ech_config_list: Option<String>,
}

/// ECH configuration of [`ECH_DOMAIN`] as published over DoH.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EchConfig {
    /// Normalized endpoint that answered the query.
    pub doh_endpoint: String,
    pub domain: String,
    /// Base64 ECHConfigList from the first HTTPS record that has one.
    pub ech_config_list: String,
    /// Every HTTPS record in the answer, ordered by priority.
    pub records: Vec<HttpsRecord>,
}

pub(crate) async fn query_ech_config(doh_endpoint: &str) -> Result<EchConfig> {
    query_ech_config_for(ECH_DOMAIN, doh_endpoint).await
}

/// Decode the base64 `ech` SvcParam into the wire-format ECHConfigList that
/// rustls expects, including its two-byte list-length prefix.
pub(crate) fn decode_ech_config_list(base64: &str) -> Result<Vec<u8>> {
    data_encoding::BASE64
        .decode(base64.as_bytes())
        .map_err(|error| AppError::Unexpected(format!("invalid ECHConfigList base64: {error}")))
}

async fn query_ech_config_for(domain: &str, doh_endpoint: &str) -> Result<EchConfig> {
    let endpoint = DohEndpoint::parse(doh_endpoint)?;
    let ips = endpoint.resolve().await?;
    let config = resolver_config_for(&endpoint.host, endpoint.port, &endpoint.path, &ips);

    query_with_config(domain, &endpoint.url, config).await
}

/// Build a DoH resolver config for the already-resolved endpoint IPs.
///
/// hickory's `ResolverConfig::https` helper always uses port 443, so the
/// name servers are assembled manually to keep an explicit `:port` from the
/// endpoint URL.
fn resolver_config_for(host: &str, port: u16, path: &str, ips: &[IpAddr]) -> ResolverConfig {
    let name_servers = ips
        .iter()
        .map(|&ip| {
            let mut connection = ConnectionConfig::https(Arc::from(host), Some(Arc::from(path)));
            connection.port = port;
            NameServerConfig::new(ip, true, vec![connection])
        })
        .collect();

    ResolverConfig::from_name_servers(name_servers)
}

/// Force an FQDN so hickory does not append search domains from the host.
fn fqdn(domain: &str) -> String {
    if domain.ends_with('.') {
        domain.to_owned()
    } else {
        format!("{domain}.")
    }
}

fn resolver_options() -> ResolverOpts {
    let mut options = ResolverOpts::default();
    options.timeout = LOOKUP_TIMEOUT;
    options.attempts = 1;
    // `DohEndpoint::resolve` sorts IPv4 first; serial attempts then avoid
    // waiting on a broken IPv6 path when the host is dual-stack.
    options.server_ordering_strategy = ServerOrderingStrategy::UserProvidedOrder;
    options.num_concurrent_reqs = 1;
    options
}

fn build_resolver(config: ResolverConfig) -> Result<Resolver<TokioRuntimeProvider>> {
    Resolver::builder_with_config(config, TokioRuntimeProvider::default())
        .with_options(resolver_options())
        .build()
        .map_err(|error| AppError::Unexpected(format!("failed to build the DoH resolver: {error}")))
}

async fn query_with_config(
    domain: &str,
    endpoint_url: &str,
    config: ResolverConfig,
) -> Result<EchConfig> {
    let resolver = build_resolver(config)?;

    let lookup = resolver
        .lookup(fqdn(domain), RecordType::HTTPS)
        .await
        .map_err(|error| {
            if error.is_nx_domain() || error.is_no_records_found() {
                AppError::NotFound {
                    resource: format!("HTTPS record for {domain}"),
                }
            } else {
                AppError::Network {
                    url: endpoint_url.to_owned(),
                    reason: error.to_string(),
                }
            }
        })?;

    let mut records: Vec<HttpsRecord> = lookup.answers().iter().filter_map(https_record).collect();
    records.sort_by_key(|record| record.priority);

    let ech_config_list = records
        .iter()
        .find_map(|record| record.ech_config_list.clone())
        .ok_or_else(|| AppError::NotFound {
            resource: format!("ech SvcParam for {domain}"),
        })?;

    Ok(EchConfig {
        doh_endpoint: endpoint_url.to_owned(),
        domain: domain.to_owned(),
        ech_config_list,
        records,
    })
}

/// Project one DNS record into the wire shape; non-HTTPS answers are ignored.
fn https_record(record: &Record) -> Option<HttpsRecord> {
    let RData::HTTPS(https) = &record.data else {
        return None;
    };

    let svc = &https.0;
    let ech_config_list = svc.svc_params.iter().find_map(|(key, value)| {
        if *key != SvcParamKey::EchConfigList {
            return None;
        }
        let SvcParamValue::EchConfigList(config) = value else {
            return None;
        };
        // Encode the raw ECHConfigList. hickory's Display wraps the value in
        // SVCB presentation quotes, which are not part of the ECH blob.
        Some(data_encoding::BASE64.encode(&config.0))
    });

    Some(HttpsRecord {
        priority: svc.svc_priority,
        target: svc.target_name.to_string(),
        ech_config_list,
    })
}

/// Parsed DoH endpoint. Parsing is separated from resolution so it can be
/// unit-tested without touching the network.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DohEndpoint {
    /// Normalized URL, also used as the error context.
    url: String,
    /// Hostname or IP literal used for the TLS server name.
    host: String,
    port: u16,
    path: String,
}

impl DohEndpoint {
    fn parse(raw: &str) -> Result<Self> {
        let url = Url::parse(raw.trim())
            .map_err(|_| invalid_endpoint("must be an absolute https URL"))?;

        if url.scheme() != "https" {
            return Err(invalid_endpoint("scheme must be https"));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(invalid_endpoint("must not include credentials"));
        }
        if url.query().is_some() {
            return Err(invalid_endpoint("must not include a query string"));
        }
        if url.fragment().is_some() {
            return Err(invalid_endpoint("must not include a fragment"));
        }

        let host = match url.host() {
            Some(Host::Ipv4(ip)) => ip.to_string(),
            // hickory passes the same string to rustls and to the HTTP
            // authority; a bare IPv6 address cannot be both a valid TLS name
            // and a bracketed :authority, so require a hostname instead.
            Some(Host::Ipv6(_)) => {
                return Err(invalid_endpoint(
                    "IPv6 literal endpoints are not supported; use a hostname",
                ));
            }
            Some(Host::Domain(domain)) => domain.to_owned(),
            None => return Err(invalid_endpoint("host must not be empty")),
        };
        let path = match url.path() {
            "" | "/" => DEFAULT_DOH_PATH.to_owned(),
            path => path.to_owned(),
        };
        let port = url.port_or_known_default().unwrap_or(443);

        Ok(Self {
            url: url.to_string(),
            host,
            port,
            path,
        })
    }

    /// Resolve the endpoint host into the IPs hickory needs for its DoH name
    /// server. IP literals bypass the OS resolver entirely.
    async fn resolve(&self) -> Result<Vec<IpAddr>> {
        if let Ok(ip) = self.host.parse::<IpAddr>() {
            return Ok(vec![ip]);
        }

        let addresses = tokio::net::lookup_host((self.host.as_str(), self.port))
            .await
            .map_err(|error| AppError::Network {
                url: self.url.clone(),
                reason: format!("could not resolve the DoH host: {error}"),
            })?;

        let mut ips: Vec<IpAddr> = addresses.map(|address| address.ip()).collect();
        ips.sort();
        ips.dedup();

        if ips.is_empty() {
            return Err(AppError::Network {
                url: self.url.clone(),
                reason: "the DoH host resolved to no addresses".into(),
            });
        }

        Ok(ips)
    }
}

fn invalid_endpoint(reason: impl Into<String>) -> AppError {
    AppError::InvalidInput {
        field: "dohEndpoint",
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    use hickory_resolver::proto::rr::{
        domain::Name,
        rdata::{
            https::HTTPS,
            svcb::{EchConfigList, SVCB},
        },
    };

    fn parse(raw: &str) -> DohEndpoint {
        DohEndpoint::parse(raw).unwrap()
    }

    #[test]
    fn parses_a_doh_url_into_connection_parts() {
        let endpoint = parse("https://cloudflare-dns.com/dns-query");

        assert_eq!(endpoint.host, "cloudflare-dns.com");
        assert_eq!(endpoint.port, 443);
        assert_eq!(endpoint.path, "/dns-query");
        assert_eq!(endpoint.url, "https://cloudflare-dns.com/dns-query");
    }

    #[test]
    fn defaults_the_path_and_keeps_explicit_ports() {
        let endpoint = parse("https://1.1.1.1");
        assert_eq!(endpoint.host, "1.1.1.1");
        assert_eq!(endpoint.path, DEFAULT_DOH_PATH);

        let endpoint = parse("https://dns.google:8443/custom");
        assert_eq!(endpoint.port, 8443);
        assert_eq!(endpoint.path, "/custom");
    }

    #[test]
    fn rejects_non_https_or_ambiguous_endpoints() {
        for raw in [
            "http://dns.google/dns-query",
            "dns.google",
            "https://user:password@dns.google/dns-query",
            "https://dns.google/dns-query?name=cloudflare-ech.com",
            "https://dns.google/dns-query#fragment",
            "https://[2606:4700:4700::1111]/dns-query",
        ] {
            let error = DohEndpoint::parse(raw).unwrap_err();
            assert!(
                matches!(
                    error,
                    AppError::InvalidInput {
                        field: "dohEndpoint",
                        ..
                    }
                ),
                "unexpected error for {raw}: {error}"
            );
        }
    }

    #[test]
    fn resolver_config_keeps_the_endpoint_port() {
        let config = resolver_config_for(
            "dns.google",
            8443,
            "/dns-query",
            &[IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)],
        );

        assert_eq!(config.name_servers.len(), 1);
        assert_eq!(config.name_servers[0].connections[0].port, 8443);
    }

    #[test]
    fn extracts_the_ech_svc_param() {
        let target = Name::from_str("cloudflare-ech.com.").unwrap();
        let svc = SVCB::new(
            1,
            target.clone(),
            vec![(
                SvcParamKey::EchConfigList,
                SvcParamValue::EchConfigList(EchConfigList(vec![0x00, 0x03, 0xaa, 0xbb, 0xcc])),
            )],
        );
        let record = Record::from_rdata(target, 300, RData::HTTPS(HTTPS(svc)));

        let parsed = https_record(&record).unwrap();

        assert_eq!(parsed.priority, 1);
        assert_eq!(parsed.target, "cloudflare-ech.com.");
        assert_eq!(parsed.ech_config_list.as_deref(), Some("AAOqu8w="));
    }

    #[test]
    fn keeps_https_records_without_ech() {
        let target = Name::from_str("cloudflare-ech.com.").unwrap();
        let svc = SVCB::new(1, target.clone(), Vec::new());
        let record = Record::from_rdata(target, 300, RData::HTTPS(HTTPS(svc)));

        let parsed = https_record(&record).unwrap();

        assert!(parsed.ech_config_list.is_none());
    }

    #[test]
    fn ignores_non_https_answers() {
        let target = Name::from_str("cloudflare-ech.com.").unwrap();
        let record = Record::from_rdata(target, 300, RData::A("104.18.10.118".parse().unwrap()));

        assert!(https_record(&record).is_none());
    }

    #[test]
    fn serializes_the_frontend_contract() {
        let config = EchConfig {
            doh_endpoint: "https://cloudflare-dns.com/dns-query".into(),
            domain: ECH_DOMAIN.into(),
            ech_config_list: "AAOqu8w=".into(),
            records: vec![HttpsRecord {
                priority: 1,
                target: "cloudflare-ech.com.".into(),
                ech_config_list: Some("AAOqu8w=".into()),
            }],
        };

        let value = serde_json::to_value(&config).unwrap();

        assert_eq!(value["dohEndpoint"], "https://cloudflare-dns.com/dns-query");
        assert_eq!(value["domain"], ECH_DOMAIN);
        assert_eq!(value["echConfigList"], "AAOqu8w=");
        assert_eq!(value["records"][0]["echConfigList"], "AAOqu8w=");
    }

    /// AliDNS/360 are bootstrap-only: they must return the shared
    /// `cloudflare-ech.com` ECH config.
    #[tokio::test]
    #[ignore = "queries public DoH endpoints"]
    async fn queries_cloudflare_ech_over_doh() {
        for doh_endpoint in [
            "https://dns.alidns.com/dns-query",
            "https://doh.360.cn/dns-query",
        ] {
            let config = query_ech_config(doh_endpoint).await.unwrap();

            assert_eq!(config.domain, ECH_DOMAIN);
            assert!(!config.records.is_empty());
            assert!(!config.ech_config_list.is_empty());
            assert!(
                config
                    .records
                    .iter()
                    .any(|record| record.ech_config_list.is_some())
            );
        }
    }
}
