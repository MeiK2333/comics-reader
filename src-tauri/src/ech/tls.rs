//! rustls ECH and plain-TLS handshakes.
//!
//! Every target first gets an ECH attempt with the shared
//! `cloudflare-ech.com` config. If ECH fails, the same target IPs are retried
//! with ordinary TLS so non-Cloudflare sites remain reachable.

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use rustls::client::{EchConfig, EchMode, EchStatus};
use rustls::pki_types::{EchConfigListBytes, ServerName};
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use crate::ech::bootstrap::{EchConfig as DohEchConfig, decode_ech_config_list};
use crate::error::{AppError, Result};

const HTTPS_PORT: u16 = 443;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
const HTTP_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_HTTP_BYTES: usize = 64 * 1024;

/// Result of one target connection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EchHandshake {
    pub domain: String,
    /// The shared ECH config source (`cloudflare-ech.com`).
    pub config_domain: String,
    /// The bootstrap DoH endpoint that supplied the ECH config.
    pub doh_endpoint: String,
    /// `accepted`/`rejected`/`notOffered`/...; plain fallback is `notOffered`.
    pub ech_status: String,
    pub tls_version: String,
    pub alpn: Option<String>,
    /// Only present when the negotiated ALPN is HTTP/1.1 and a response line
    /// was received; a handshake-only failure stays `None`.
    pub http_status: Option<u16>,
    /// `true` when ECH failed and the connection was retried with plain TLS.
    pub fallback: bool,
}

/// Try ECH for every resolved IP, then retry with plain TLS if ECH fails.
pub(crate) async fn handshake(
    domain: &str,
    config: &DohEchConfig,
    ech_tls: &Arc<rustls::ClientConfig>,
    ips: Vec<IpAddr>,
) -> Result<EchHandshake> {
    let domain = domain.trim().trim_end_matches('.');
    if domain.is_empty() || domain.parse::<IpAddr>().is_ok() {
        return Err(AppError::InvalidInput {
            field: "domain",
            reason: "must be a DNS name, not an IP address".into(),
        });
    }
    if ips.is_empty() {
        return Err(AppError::Network {
            url: format!("https://{domain}/"),
            reason: "no target addresses to connect to".into(),
        });
    }

    let server_name =
        ServerName::try_from(domain.to_owned()).map_err(|_| AppError::InvalidInput {
            field: "domain",
            reason: "not a valid TLS server name".into(),
        })?;
    let mut last_error = None;

    for ip in &ips {
        match handshake_one(
            domain,
            &config.doh_endpoint,
            config,
            *ip,
            server_name.clone(),
            ech_tls.clone(),
            false,
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(error) => last_error = Some(error),
        }
    }

    let plain_tls = Arc::new(build_plain_tls_config()?);
    for ip in &ips {
        match handshake_one(
            domain,
            &config.doh_endpoint,
            config,
            *ip,
            server_name.clone(),
            plain_tls.clone(),
            true,
        )
        .await
        {
            Ok(result) => return Ok(result),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| AppError::Network {
        url: format!("https://{domain}/"),
        reason: "all ECH and plain TLS attempts failed".into(),
    }))
}

pub(crate) fn build_ech_tls_config(base64_ech_config: &str) -> Result<rustls::ClientConfig> {
    build_ech_tls_config_with_alpn(base64_ech_config, &[b"http/1.1"])
}

/// Build the ECH TLS config with an explicit ALPN list.
///
/// reqwest's `use_preconfigured_tls` keeps the supplied rustls config as-is
/// and never fills in `alpn_protocols`, so the reqwest client has to request
/// `h2` here; an empty list would silently pin the connection to HTTP/1.1.
pub(crate) fn build_ech_tls_config_with_alpn(
    base64_ech_config: &str,
    alpn: &[&[u8]],
) -> Result<rustls::ClientConfig> {
    let ech_bytes = decode_ech_config_list(base64_ech_config)?;
    let ech = EchConfig::new(
        EchConfigListBytes::from(ech_bytes),
        rustls::crypto::aws_lc_rs::hpke::ALL_SUPPORTED_SUITES,
    )
    .map_err(|error| AppError::Unexpected(format!("invalid ECH config: {error}")))?;

    let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_ech(EchMode::Enable(ech))
    .map_err(|error| AppError::Unexpected(format!("failed to enable ECH: {error}")))?
    .with_root_certificates(root_store())
    .with_no_client_auth();

    config.alpn_protocols = alpn.iter().map(|protocol| protocol.to_vec()).collect();
    Ok(config)
}

fn build_plain_tls_config() -> Result<rustls::ClientConfig> {
    let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::aws_lc_rs::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|error| AppError::Unexpected(format!("failed to select TLS versions: {error}")))?
    .with_root_certificates(root_store())
    .with_no_client_auth();

    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}

fn root_store() -> rustls::RootCertStore {
    rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    }
}

#[allow(clippy::too_many_arguments)]
async fn handshake_one(
    domain: &str,
    doh_endpoint: &str,
    config: &DohEchConfig,
    ip: IpAddr,
    server_name: ServerName<'static>,
    tls_config: Arc<rustls::ClientConfig>,
    fallback: bool,
) -> Result<EchHandshake> {
    let url = format!("https://{domain}/");
    let address = SocketAddr::new(ip, HTTPS_PORT);

    let tcp = tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(address))
        .await
        .map_err(|_| AppError::Network {
            url: url.clone(),
            reason: format!("TCP connect to {address} timed out"),
        })?
        .map_err(|error| AppError::Network {
            url: url.clone(),
            reason: format!("TCP connect to {address} failed: {error}"),
        })?;
    let _ = tcp.set_nodelay(true);

    let mut stream = tokio::time::timeout(
        HANDSHAKE_TIMEOUT,
        TlsConnector::from(tls_config).connect(server_name, tcp),
    )
    .await
    .map_err(|_| AppError::Network {
        url: url.clone(),
        reason: format!("TLS handshake with {address} timed out"),
    })?
    .map_err(|error| AppError::Network {
        url: url.clone(),
        reason: format!("TLS handshake with {address} failed: {error}"),
    })?;

    let (ech_status, tls_version, alpn) = {
        let connection = stream.get_ref().1;
        (
            ech_status_label(connection.ech_status()),
            tls_version_label(connection.protocol_version()),
            connection
                .alpn_protocol()
                .map(|value| String::from_utf8_lossy(value).into_owned()),
        )
    };

    let http_status = if alpn.as_deref() == Some("http/1.1") {
        http_status_for(&mut stream, domain).await
    } else {
        None
    };

    Ok(EchHandshake {
        domain: domain.to_owned(),
        config_domain: config.domain.clone(),
        doh_endpoint: doh_endpoint.to_owned(),
        ech_status,
        tls_version,
        alpn,
        http_status,
        fallback,
    })
}

/// Send a minimal HTTP/1.1 request so the probe can prove application data
/// flows after the handshake. Failures here do not invalidate the handshake.
async fn http_status_for(
    stream: &mut tokio_rustls::client::TlsStream<TcpStream>,
    domain: &str,
) -> Option<u16> {
    let request = format!(
        "GET / HTTP/1.1\r\nHost: {domain}\r\nUser-Agent: comics-reader-ech-probe\r\nConnection: close\r\n\r\n"
    );
    let write = tokio::time::timeout(HTTP_TIMEOUT, stream.write_all(request.as_bytes())).await;
    if !matches!(write, Ok(Ok(()))) {
        return None;
    }

    let mut response = Vec::new();
    let read = async {
        let mut buffer = [0u8; 4096];
        loop {
            let count = stream.read(&mut buffer).await.ok()?;
            if count == 0 {
                break;
            }
            response.extend_from_slice(&buffer[..count]);
            if response.len() >= MAX_HTTP_BYTES {
                break;
            }
        }
        Some(())
    };

    let _ = tokio::time::timeout(HTTP_TIMEOUT, read).await;
    parse_http_status(&response)
}

fn parse_http_status(response: &[u8]) -> Option<u16> {
    let line = std::str::from_utf8(response).ok()?.lines().next()?;
    let mut parts = line.split_whitespace();
    if !parts.next()?.starts_with("HTTP/") {
        return None;
    }
    parts.next()?.parse().ok()
}

fn ech_status_label(status: EchStatus) -> String {
    match status {
        EchStatus::Accepted => "accepted",
        EchStatus::Rejected => "rejected",
        EchStatus::Offered => "offered",
        EchStatus::NotOffered => "notOffered",
        EchStatus::Grease => "grease",
    }
    .into()
}

fn tls_version_label(version: Option<rustls::ProtocolVersion>) -> String {
    match version {
        Some(rustls::ProtocolVersion::TLSv1_3) => "TLSv1.3".into(),
        Some(rustls::ProtocolVersion::TLSv1_2) => "TLSv1.2".into(),
        Some(other) => format!("{other:?}"),
        None => "unknown".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real ECHConfigList captured from cloudflare-ech.com. It is only used to
    /// prove the decoder and rustls HPKE selection work without the network.
    const ECH_FIXTURE: &str = "AEX+DQBB3AAgACBqaORCMxC154Vcj7NCewasMUXDSfWaZ2y1zNZyrL7DbQAEAAEAAQASY2xvdWRmbGFyZS1lY2guY29tAAA=";

    #[test]
    fn builds_a_rustls_config_from_the_live_ech_fixture() {
        build_ech_tls_config(ECH_FIXTURE).unwrap();
    }

    #[test]
    fn builds_a_plain_tls_fallback_config() {
        build_plain_tls_config().unwrap();
    }

    #[test]
    fn builds_a_reqwest_ech_config_with_h2_alpn() {
        let config = build_ech_tls_config_with_alpn(ECH_FIXTURE, &[b"h2", b"http/1.1"]).unwrap();

        assert_eq!(
            config.alpn_protocols,
            vec![b"h2".to_vec(), b"http/1.1".to_vec()]
        );
    }

    #[test]
    fn rejects_invalid_ech_base64() {
        let error = build_ech_tls_config("not base64").unwrap_err();
        assert!(
            matches!(error, AppError::Unexpected(_)),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn parses_the_http_status_line() {
        assert_eq!(
            parse_http_status(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n"),
            Some(200)
        );
        assert_eq!(parse_http_status(b"not http"), None);
    }

    #[test]
    fn labels_ech_status() {
        assert_eq!(ech_status_label(EchStatus::Accepted), "accepted");
        assert_eq!(ech_status_label(EchStatus::Rejected), "rejected");
    }
}
