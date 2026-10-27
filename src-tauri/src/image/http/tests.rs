use super::client::{ACCEPT_VALUE, USER_AGENT_VALUE, base_headers, ech_fallback_key};
use super::fetch::is_mirror_error;
use super::*;
use crate::error::AppError;
use reqwest::header::{ACCEPT, USER_AGENT};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use url::Url;

fn proxy(host: &str, port: u16) -> HttpProxyConfig {
    HttpProxyConfig {
        host: host.into(),
        port,
    }
}

#[test]
fn base_headers_look_like_a_browser() {
    let headers = base_headers();
    assert_eq!(headers[USER_AGENT].to_str().unwrap(), USER_AGENT_VALUE);
    assert_eq!(headers[ACCEPT].to_str().unwrap(), ACCEPT_VALUE);
}

#[test]
fn normalizes_ipv4_and_hostname_proxies() {
    assert_eq!(
        proxy("127.0.0.1", 7890).normalized_url().unwrap(),
        "http://127.0.0.1:7890/"
    );
    assert_eq!(
        proxy(" proxy.example.com ", 8080).normalized_url().unwrap(),
        "http://proxy.example.com:8080/"
    );
}

#[test]
fn normalizes_ipv6_proxies() {
    assert_eq!(
        proxy("::1", 7890).normalized_url().unwrap(),
        "http://[::1]:7890/"
    );
    assert_eq!(
        proxy("[::1]", 7890).normalized_url().unwrap(),
        "http://[::1]:7890/"
    );
}

#[test]
fn rejects_invalid_proxy_addresses() {
    for (host, port) in [
        ("", 7890),
        ("   ", 7890),
        ("http://127.0.0.1", 7890),
        ("127.0.0.1:7890", 7890),
        ("proxy.example.com/path", 7890),
        ("user@proxy.example.com", 7890),
        ("proxy example", 7890),
        ("127.0.0.1", 0),
    ] {
        let error = proxy(host, port).normalized_url().unwrap_err();
        assert!(
            matches!(error, AppError::InvalidInput { field: "proxy", .. }),
            "unexpected error for {host:?}: {error}"
        );
    }
}

#[test]
fn set_proxy_replaces_and_clears_the_cached_client() {
    let client = HttpClient::default();

    client.set_proxy(Some(proxy("127.0.0.1", 7890))).unwrap();
    assert_eq!(
        client.lock().unwrap().proxy.as_deref(),
        Some("http://127.0.0.1:7890/")
    );

    client.set_proxy(None).unwrap();
    assert!(client.lock().unwrap().proxy.is_none());
}

#[test]
fn rejected_proxy_keeps_the_previous_configuration() {
    let client = HttpClient::default();
    client.set_proxy(Some(proxy("127.0.0.1", 7890))).unwrap();

    let error = client
        .set_proxy(Some(proxy("http://bad", 7890)))
        .unwrap_err();
    assert!(matches!(
        error,
        AppError::InvalidInput { field: "proxy", .. }
    ));
    assert_eq!(
        client.lock().unwrap().proxy.as_deref(),
        Some("http://127.0.0.1:7890/")
    );
}

#[test]
fn reports_ech_disabled_by_default() {
    let client = HttpClient::default();

    let status = client.ech_status().unwrap();

    assert!(!status.enabled);
    assert!(!status.ready);
    assert!(status.config_domain.is_none());
    assert!(status.bootstrap_doh.is_none());
    assert!(status.expires_at.is_none());
}

#[test]
fn ech_fallback_key_is_scoped_to_the_https_origin() {
    assert_eq!(
        ech_fallback_key(&Url::parse("http://example.com/").unwrap()),
        None
    );
    assert_eq!(
        ech_fallback_key(&Url::parse("https://example.com/path").unwrap()).as_deref(),
        Some("example.com:443")
    );
    assert_eq!(
        ech_fallback_key(&Url::parse("https://EXAMPLE.com:8443/").unwrap()).as_deref(),
        Some("example.com:8443")
    );
}

#[test]
fn caches_and_expires_ech_fallbacks() {
    let client = HttpClient::default();
    let key = "example.com:443";

    assert!(!client.ech_fallback_is_active(key).unwrap());
    client.remember_ech_fallback(key).unwrap();
    assert!(client.ech_fallback_is_active(key).unwrap());

    client
        .lock()
        .unwrap()
        .ech_fallback_until
        .insert(key.into(), Instant::now() - Duration::from_secs(1));
    assert!(!client.ech_fallback_is_active(key).unwrap());
    assert!(client.lock().unwrap().ech_fallback_until.is_empty());
}

#[test]
fn disabling_ech_clears_cached_fallbacks() {
    let client = HttpClient::default();
    let key = "example.com:443";
    client.remember_ech_fallback(key).unwrap();

    client.disable_ech().unwrap();

    assert!(!client.ech_fallback_is_active(key).unwrap());
}

#[test]
fn disabling_ech_invalidates_an_in_flight_enable() {
    let client = HttpClient::default();
    let control = client.begin_ech_control().unwrap();

    client.disable_ech().unwrap();

    assert!(!client.ech_control_is_current(control).unwrap());
}

#[tokio::test]
async fn refresh_is_a_noop_without_ech() {
    let client = HttpClient::default();

    client.refresh_ech_if_due().await;

    assert!(!client.ech_status().unwrap().enabled);
}

/// Exercises the real runtime: ECH succeeds for nhentai, a non-Cloudflare
/// host falls back to plain TLS, the config refreshes on demand, and the
/// proxy is rejected while ECH is on.
#[tokio::test]
#[ignore = "performs live ECH requests"]
async fn ech_client_fetches_and_falls_back() {
    let http = HttpClient::default();
    let session = Arc::new(
        crate::ech::EchSession::bootstrap("https://dns.alidns.com/dns-query")
            .await
            .unwrap(),
    );
    let control = http.begin_ech_control().unwrap();
    http.enable_ech_if_current(control, session).unwrap();

    let status = http.ech_status().unwrap();
    assert!(status.enabled);
    assert!(status.ready);
    assert_eq!(status.config_domain.as_deref(), Some("cloudflare-ech.com"));

    let nhentai = Url::parse("https://nhentai.net/").unwrap();
    let response = http
        .send(&nhentai, |client| client.get(nhentai.as_str()))
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let qq = Url::parse("https://www.qq.com/").unwrap();
    let response = http
        .send(&qq, |client| client.get(qq.as_str()))
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert!(http.ech_fallback_is_active("www.qq.com:443").unwrap());

    // Force the deadline so the next HTTPS request re-runs the bootstrap
    // and swaps in a fresh runtime.
    let generation = http.lock().unwrap().ech_generation;
    {
        let mut state = http.lock().unwrap();
        state.ech.as_mut().unwrap().refresh_after = SystemTime::now() - Duration::from_secs(1);
    }
    let response = http
        .send(&nhentai, |client| client.get(nhentai.as_str()))
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert!(http.lock().unwrap().ech_generation > generation);
    // The fallback decision belongs to the origin, not the ECH runtime.
    assert!(http.ech_fallback_is_active("www.qq.com:443").unwrap());
    let response = http
        .send(&qq, |client| client.get(qq.as_str()))
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let error = http.set_proxy(Some(proxy("127.0.0.1", 7890))).unwrap_err();
    assert!(matches!(
        error,
        AppError::InvalidInput { field: "proxy", .. }
    ));

    http.disable_ech().unwrap();
    assert!(!http.ech_status().unwrap().enabled);
    assert!(!http.ech_fallback_is_active("www.qq.com:443").unwrap());
    http.set_proxy(Some(proxy("127.0.0.1", 7890))).unwrap();
}

/// Full business path through ECH: nhentai search, detail, and the detail
/// thumbnail (served from a separate Cloudflare host).
#[tokio::test]
#[ignore = "performs a live nhentai search, detail and image download"]
async fn nhentai_search_detail_and_image_work_with_ech() {
    let http = HttpClient::default();
    let session = Arc::new(
        crate::ech::EchSession::bootstrap("https://dns.alidns.com/dns-query")
            .await
            .unwrap(),
    );
    let control = http.begin_ech_control().unwrap();
    http.enable_ech_if_current(control, session).unwrap();

    let platform = crate::platforms::find_catalog("nhentai").unwrap();

    let spec = platform.search_request("love", 1).unwrap();
    let fetched = fetch_document(&http, platform, &spec).await.unwrap();
    let search = platform.parse_search("love", 1, &fetched.body).unwrap();
    let item = search.result.first().expect("nhentai returned no results");

    let spec = platform.detail_request(&item.id).unwrap();
    let fetched = fetch_document(&http, platform, &spec).await.unwrap();
    let detail = platform.parse_detail(&item.id, &fetched.body).unwrap();
    assert!(!detail.title.is_empty());

    let thumb = Url::parse(&detail.thumb).unwrap();
    let download = download_image(&http, platform, &thumb).await.unwrap();
    assert!(!download.bytes.is_empty());
}

#[test]
fn mirror_errors_are_the_ones_another_host_can_fix() {
    assert!(is_mirror_error(&AppError::Network {
        url: "https://example.com".into(),
        reason: "timeout".into(),
    }));
    assert!(is_mirror_error(&AppError::EmptyResponse {
        url: "https://example.com".into(),
    }));

    for status in [403u16, 429, 500, 502, 520] {
        assert!(is_mirror_error(&AppError::HttpStatus {
            url: "https://example.com".into(),
            status,
        }));
    }
    for status in [400u16, 401, 404, 410] {
        assert!(!is_mirror_error(&AppError::HttpStatus {
            url: "https://example.com".into(),
            status,
        }));
    }

    // A valid but unusable body is not a mirror problem for documents;
    // the image pipeline adds `notImage` separately.
    assert!(!is_mirror_error(&AppError::NotImage {
        url: "https://example.com".into(),
        content_type: "text/html".into(),
    }));
    assert!(!is_mirror_error(&AppError::TooLarge {
        url: "https://example.com".into(),
        limit: 1,
    }));
}
