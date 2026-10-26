//! JM's mobile (APP) API: request signing, response decryption and payloads.
//!
//! The HTML site is region-locked on many networks (it answers 403 even for
//! search), while the APP endpoints keep working from the same hosts. The
//! protocol mirrors `JMComic-Crawler-Python`: `token`/`tokenparam` headers, a
//! base64 + AES-256-ECB payload keyed by `md5(timestamp + secret)`, and cover
//! URLs derived from the album id.
//!
//! Only protocol details live here; [`super::jm`] owns the platform plumbing.

use std::sync::{LazyLock, RwLock};

use aes::Aes256;
use aes::cipher::{BlockDecrypt, KeyInit};
use data_encoding::BASE64;
use md5::{Digest, Md5};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use url::Url;

use crate::error::{AppError, Result};

/// Version signed into `tokenparam` at build time. JM keeps raising the
/// minimum it accepts, so the effective version is refreshed from `/setting`
/// at runtime instead of being trusted forever.
const BUILTIN_APP_VERSION: &str = "2.1.12";
/// Set once `/setting` reports a newer app version.
static APP_VERSION: LazyLock<RwLock<String>> =
    LazyLock::new(|| RwLock::new(BUILTIN_APP_VERSION.to_string()));
/// APP API hosts. They are interchangeable and rotate between releases.
pub(super) const API_DOMAINS: &[&str] = &[
    "www.cdnhjk.net",
    "www.cdngwc.cc",
    "www.cdngwc.net",
    "www.cdngwc.club",
];
/// Image CDNs. `jmapiproxy.com` is intentionally absent: its TLS handshake
/// fails from some networks while these hosts answer.
const IMAGE_DOMAINS: &[&str] = &[
    "cdn-msp.jmapiproxy1.cc",
    "cdn-msp.jmapiproxy2.cc",
    "cdn-msp2.jmapiproxy2.cc",
    "cdn-msp3.jmapiproxy2.cc",
    "cdn-msp.jmapinodeudzn.net",
    "cdn-msp3.jmapinodeudzn.net",
];

const TOKEN_SECRET: &str = "185Hcomic3PAPP7R";
const DATA_SECRET: &str = "185Hcomic3PAPP7R";
/// `/chapter_view_template` signs with a different secret than the rest of
/// the APP API, matching upstream's `APP_TOKEN_SECRET_2`.
const SCRAMBLE_SECRET: &str = "18comicAPPContent";
/// Fallback when the scramble endpoint stops exposing `scramble_id`. Upstream
/// uses the same constant; it is the threshold most albums were published at.
pub(super) const DEFAULT_SCRAMBLE_ID: i64 = 220_980;
/// The image CDN serves the browser UA a 403; the APP one is expected.
pub(super) const APP_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 9; V1938CT Build/PQ3A.190705.11211812; wv) AppleWebKit/537.36 (KHTML, like Gecko) Version/4.0 Chrome/91.0.4472.114 Safari/537.36";
const APP_ACCEPT_LANGUAGE: &str = "zh-CN,zh;q=0.9,en-US;q=0.8,en;q=0.7";
const APP_REQUESTED_WITH: &str = "com.JMComic3.app";

/// Timestamp used by both the token and the decryption key.
///
/// It is fixed for the process lifetime, matching upstream's
/// `FLAG_USE_FIX_TIMESTAMP`: the parse step only sees the response body, so the
/// request and the decryption must agree on one value without threading it
/// through the platform trait.
pub(super) fn timestamp() -> u64 {
    static FIXED: LazyLock<u64> = LazyLock::new(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or(0)
    });
    *FIXED
}

pub(super) fn api_origin() -> String {
    format!("https://{}", API_DOMAINS[0])
}

/// Referer and `X-Requested-With` value the APP sends; also used for images.
pub(super) fn referer() -> String {
    format!("https://{}", API_DOMAINS[0])
}

pub(super) const fn requested_with() -> &'static str {
    APP_REQUESTED_WITH
}

pub(super) const fn accept_language() -> &'static str {
    APP_ACCEPT_LANGUAGE
}

/// App version currently signed into `tokenparam`.
pub(super) fn app_version() -> String {
    APP_VERSION
        .read()
        .map(|version| version.clone())
        .unwrap_or_else(|_| BUILTIN_APP_VERSION.to_string())
}

pub(super) fn set_app_version(version: &str) {
    if let Ok(mut current) = APP_VERSION.write() {
        *current = version.to_string();
    }
}

/// Compare dotted versions numerically; a malformed candidate is never newer,
/// so a bad `/setting` response cannot downgrade the signed version.
pub(super) fn version_is_newer(candidate: &str, current: &str) -> bool {
    fn parts(version: &str) -> Option<Vec<u64>> {
        version.split('.').map(|part| part.parse().ok()).collect()
    }

    let (Some(mut candidate), Some(mut current)) = (parts(candidate), parts(current)) else {
        return false;
    };
    let length = candidate.len().max(current.len());
    candidate.resize(length, 0);
    current.resize(length, 0);
    candidate > current
}

/// `{ ts, version }` pair JM expects in the `tokenparam` header.
fn tokenparam(ts: u64) -> String {
    format!("{ts},{}", app_version())
}

/// `md5(ts + secret)` in lowercase hex.
fn token(ts: u64, secret: &str) -> String {
    md5_hex(&format!("{ts}{secret}"))
}

fn data_key(ts: u64) -> String {
    md5_hex(&format!("{ts}{DATA_SECRET}"))
}

fn md5_hex(input: &str) -> String {
    hex::encode(Md5::digest(input.as_bytes()))
}

/// Signing headers for one API call. They stay out of [`Platform::configure`]
/// so image requests, which do not need them, keep a minimal header set.
pub(super) fn document_headers(ts: u64) -> HeaderMap {
    signing_headers(ts, TOKEN_SECRET)
}

/// Signing headers for the scramble endpoint, which uses its own secret.
pub(super) fn scramble_headers(ts: u64) -> HeaderMap {
    signing_headers(ts, SCRAMBLE_SECRET)
}

fn signing_headers(ts: u64, secret: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("token"),
        HeaderValue::from_str(&token(ts, secret)).expect("hex digest is a valid header value"),
    );
    headers.insert(
        HeaderName::from_static("tokenparam"),
        HeaderValue::from_str(&tokenparam(ts)).expect("tokenparam is a valid header value"),
    );
    headers
}

pub(super) fn search_url(domain: &str, query: &str, page: u32) -> Result<Url> {
    let mut url = parse_url(&format!("https://{domain}/search"))?;
    url.query_pairs_mut()
        .append_pair("main_tag", "0")
        .append_pair("search_query", query)
        .append_pair("page", &page.to_string())
        .append_pair("o", "mr")
        .append_pair("t", "a");
    Ok(url)
}

pub(super) fn album_url(domain: &str, id: &str) -> Result<Url> {
    parse_url(&format!("https://{domain}/album?id={id}"))
}

/// `/setting` reports the app version JM currently expects.
pub(super) fn setting_url(domain: &str) -> Result<Url> {
    parse_url(&format!("https://{domain}/setting"))
}

pub(super) fn chapter_url(domain: &str, id: &str) -> Result<Url> {
    parse_url(&format!("https://{domain}/chapter?id={id}"))
}

/// The scramble endpoint reports the album's threshold as a JS variable. It
/// also accepts a `v` timestamp, which JM uses as a cache buster.
pub(super) fn scramble_url(domain: &str, photo_id: &str, ts: u64) -> Result<Url> {
    let mut url = parse_url(&format!("https://{domain}/chapter_view_template"))?;
    url.query_pairs_mut()
        .append_pair("id", photo_id)
        .append_pair("mode", "vertical")
        .append_pair("page", "0")
        .append_pair("app_img_shunt", "1")
        .append_pair("express", "off")
        .append_pair("v", &ts.to_string());
    Ok(url)
}

/// URL of one page image on the primary CDN.
pub(super) fn page_url(photo_id: &str, filename: &str) -> String {
    format!(
        "https://{}/media/photos/{photo_id}/{filename}",
        IMAGE_DOMAINS[0]
    )
}

/// Split a page-image URL into the photo id and the file stem (no extension,
/// as JM's scramble algorithm hashes the stem).
pub(super) fn page_image(url: &Url) -> Option<(String, String)> {
    if !url.path().starts_with("/media/photos/") {
        return None;
    }
    if !IMAGE_DOMAINS.contains(&url.host_str()?) {
        return None;
    }

    let mut segments = url.path_segments()?;
    segments.next()?; // media
    segments.next()?; // photos
    let photo_id = segments.next()?.to_string();
    let filename = segments.next()?;
    let (stem, _extension) = filename.rsplit_once('.')?;

    if !photo_id.chars().all(|character| character.is_ascii_digit()) || !is_safe_filename(stem) {
        return None;
    }

    Some((photo_id, stem.to_string()))
}

/// A file name taken from an API payload must not be able to leave the path
/// it is appended to.
pub(super) fn is_safe_filename(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

static SCRAMBLE_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"var scramble_id = (\d+);").expect("valid scramble regex"));

pub(super) fn parse_scramble_id(body: &[u8]) -> Option<i64> {
    let text = String::from_utf8_lossy(body);
    SCRAMBLE_RE
        .captures(&text)?
        .get(1)?
        .as_str()
        .parse::<i64>()
        .ok()
}

pub(super) fn is_api_host(host: &str) -> bool {
    API_DOMAINS.contains(&host)
}

/// The same image path on every other CDN host, in preference order. Photo
/// and cover URLs are interchangeable across these hosts.
pub(super) fn image_alternates(url: &Url) -> Vec<Url> {
    if !url.path().starts_with("/media/") {
        return Vec::new();
    }
    let Some(host) = url.host_str() else {
        return Vec::new();
    };
    if !IMAGE_DOMAINS.contains(&host) {
        return Vec::new();
    }

    IMAGE_DOMAINS
        .iter()
        .filter(|domain| **domain != host)
        .filter_map(|domain| {
            let mut candidate = url.clone();
            candidate.set_host(Some(domain)).ok()?;
            Some(candidate)
        })
        .collect()
}

fn parse_url(raw: &str) -> Result<Url> {
    Url::parse(raw).map_err(|error| AppError::Unexpected(format!("invalid jm api URL: {error}")))
}

/// Search results carry no cover; it is fully determined by the album id.
pub(super) fn cover_url(album_id: &str) -> String {
    format!(
        "https://{}/media/albums/{album_id}_3x4.jpg",
        IMAGE_DOMAINS[0]
    )
}

/// Decode one API envelope into `T`.
///
/// `Ok(None)` means JM answered successfully but without a payload (`[]`),
/// which is how a missing album or chapter is reported. A non-200 `code` is
/// surfaced as `httpStatus` so the existing error copy still applies.
pub(super) fn decode<T: DeserializeOwned>(body: &[u8], ts: u64) -> Result<Option<T>> {
    let envelope: Value = serde_json::from_slice(body)?;
    let code = envelope.get("code").and_then(Value::as_u64).unwrap_or(0);
    if code != 200 {
        return Err(api_error(&envelope, code));
    }

    match envelope.get("data") {
        // `data` is a base64 string whenever JM has something to say.
        Some(Value::String(encoded)) if !encoded.is_empty() => {
            let plain = decrypt(encoded, ts)?;
            serde_json::from_str(&plain)
                .map(Some)
                .map_err(AppError::from)
        }
        // Defensive: a few endpoints may return plain JSON objects.
        Some(data @ Value::Object(_)) => serde_json::from_value(data.clone())
            .map(Some)
            .map_err(AppError::from),
        _ => Ok(None),
    }
}

fn api_error(envelope: &Value, code: u64) -> AppError {
    let message = envelope
        .get("errorMsg")
        .or_else(|| envelope.get("msg"))
        .and_then(Value::as_str)
        .unwrap_or("no message");

    if (400..=599).contains(&code) {
        AppError::HttpStatus {
            url: api_origin(),
            status: code as u16,
        }
    } else {
        AppError::Unexpected(format!("jm api error {code}: {message}"))
    }
}

/// base64 → AES-256-ECB → strip padding → UTF-8.
fn decrypt(encoded: &str, ts: u64) -> Result<String> {
    let bytes = BASE64
        .decode(encoded.as_bytes())
        .map_err(|error| AppError::Unexpected(format!("jm api payload is not base64: {error}")))?;
    if bytes.is_empty() || bytes.len() % 16 != 0 {
        return Err(AppError::Unexpected(format!(
            "jm api payload has an invalid length: {}",
            bytes.len()
        )));
    }

    let cipher = Aes256::new_from_slice(data_key(ts).as_bytes())
        .map_err(|_| AppError::Unexpected("jm api key has an invalid length".into()))?;
    let mut plain = Vec::with_capacity(bytes.len());
    for chunk in bytes.chunks(16) {
        let mut block = aes::cipher::Block::<Aes256>::clone_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        plain.extend_from_slice(&block);
    }

    let padding = *plain
        .last()
        .ok_or_else(|| AppError::Unexpected("jm api payload is empty".into()))?
        as usize;
    if padding == 0 || padding > 16 || padding > plain.len() {
        return Err(AppError::Unexpected(format!(
            "jm api payload has invalid padding: {padding}"
        )));
    }
    plain.truncate(plain.len() - padding);

    String::from_utf8(plain)
        .map_err(|error| AppError::Unexpected(format!("jm api payload is not UTF-8: {error}")))
}

/// The inverse of [`decrypt`], used by tests to build realistic envelopes.
#[cfg(test)]
pub(super) fn encrypt_for_test(payload: &str, ts: u64) -> String {
    use aes::cipher::BlockEncrypt;

    let mut bytes = payload.as_bytes().to_vec();
    let padding = 16 - bytes.len() % 16;
    bytes.extend(std::iter::repeat_n(padding as u8, padding));

    let cipher = Aes256::new_from_slice(data_key(ts).as_bytes()).expect("test key is valid");
    let mut encrypted = Vec::with_capacity(bytes.len());
    for chunk in bytes.chunks(16) {
        let mut block = aes::cipher::Block::<Aes256>::clone_from_slice(chunk);
        cipher.encrypt_block(&mut block);
        encrypted.extend_from_slice(&block);
    }

    BASE64.encode(&encrypted)
}

/// Format a Unix timestamp as `YYYY-MM-DD` in JM's timezone (UTC+8), without
/// pulling in a date crate for one field.
pub(super) fn format_date(seconds: i64) -> Option<String> {
    if seconds <= 0 {
        return None;
    }

    let days = (seconds + 8 * 3600).div_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

/// Howard Hinnant's `civil_from_days`, with `days` counted from 1970-01-01.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };

    (year + i64::from(month <= 2), month, day)
}

/// A JSON scalar JM sometimes sends as a string (`"1655254028"`) and sometimes
/// as a number. Keeping one type for both avoids per-field deserializers.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(super) enum Scalar {
    Int(i64),
    Float(f64),
    Text(String),
}

impl Scalar {
    pub(super) fn text(&self) -> String {
        match self {
            Self::Int(value) => value.to_string(),
            Self::Float(value) => value.to_string(),
            Self::Text(value) => value.trim().to_string(),
        }
    }

    pub(super) fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Int(value) => u64::try_from(*value).ok(),
            Self::Float(value) if *value >= 0.0 => Some(*value as u64),
            Self::Float(_) => None,
            Self::Text(value) => value.trim().parse().ok(),
        }
    }

    pub(super) fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            Self::Float(value) => Some(*value as i64),
            Self::Text(value) => value.trim().parse().ok(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct Album {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub author: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub works: Vec<String>,
    #[serde(default)]
    pub actors: Vec<String>,
    #[serde(default)]
    pub series: Vec<SeriesItem>,
    #[serde(default)]
    pub total_photos: Option<Scalar>,
    #[serde(default)]
    pub addtime: Option<Scalar>,
    #[serde(default)]
    pub total_views: Option<Scalar>,
    #[serde(default)]
    pub likes: Option<Scalar>,
    #[serde(default)]
    pub comment_total: Option<Scalar>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SeriesItem {
    #[serde(default)]
    pub id: Option<Scalar>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sort: Option<Scalar>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Chapter {
    #[serde(default)]
    pub name: String,
    /// Album id; `0` for a single-chapter album, where the photo id is the
    /// album id.
    #[serde(default)]
    pub series_id: Option<Scalar>,
    #[serde(default)]
    pub series: Vec<SeriesItem>,
    #[serde(default)]
    pub images: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Setting {
    #[serde(default)]
    pub jm3_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SearchPage {
    #[serde(default)]
    pub total: Option<Scalar>,
    #[serde(default)]
    pub content: Vec<SearchItem>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SearchItem {
    #[serde(default)]
    pub id: Option<Scalar>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub category: Option<Category>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Category {
    #[serde(default)]
    pub title: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Captured from a live APP call: `{"id":1,"name":"test"}` encrypted with
    /// `md5("1700000000185Hcomic3PAPP7R")`.
    const CIPHERTEXT: &str = "1rFYokqcUimA1uq3/1OafhLihszcnULNDGwyo84tfUQ=";
    const PLAINTEXT: &str = r#"{"id":1,"name":"test"}"#;

    #[test]
    fn token_matches_the_known_vector() {
        assert_eq!(
            token(1_700_000_000, TOKEN_SECRET),
            "880c64833265ad47a928afcf1b1220f5"
        );
        assert_eq!(tokenparam(1_700_000_000), "1700000000,2.1.12");
    }

    #[test]
    fn document_headers_carry_the_signature() {
        let headers = document_headers(1_700_000_000);
        assert_eq!(
            headers.get("token").unwrap().to_str().unwrap(),
            "880c64833265ad47a928afcf1b1220f5"
        );
        assert_eq!(
            headers.get("tokenparam").unwrap().to_str().unwrap(),
            "1700000000,2.1.12"
        );
    }

    #[test]
    fn the_scramble_endpoint_signs_with_its_own_secret() {
        let document = document_headers(1_700_000_000);
        let scramble = scramble_headers(1_700_000_000);
        assert_ne!(
            document.get("token").unwrap(),
            scramble.get("token").unwrap()
        );
        assert_eq!(
            scramble.get("tokenparam").unwrap().to_str().unwrap(),
            "1700000000,2.1.12"
        );
    }

    #[test]
    fn builds_chapter_and_scramble_urls() {
        assert_eq!(
            chapter_url("www.cdngwc.cc", "350235").unwrap().as_str(),
            "https://www.cdngwc.cc/chapter?id=350235"
        );

        let scramble = scramble_url("www.cdnhjk.net", "350235", 1_700_000_000).unwrap();
        let url = scramble.as_str();
        assert!(url.starts_with("https://www.cdnhjk.net/chapter_view_template?"));
        assert!(url.contains("id=350235"));
        assert!(url.contains("mode=vertical"));
        assert!(url.contains("app_img_shunt=1"));
        assert!(url.contains("v=1700000000"));
    }

    #[test]
    fn parses_the_scramble_variable() {
        let body = br#"<script>var scramble_id = 220980;</script>"#;
        assert_eq!(parse_scramble_id(body), Some(220_980));
        assert_eq!(parse_scramble_id(b"<html>no variable</html>"), None);
    }

    #[test]
    fn splits_page_image_urls_into_photo_and_stem() {
        let page = Url::parse("https://cdn-msp2.jmapiproxy2.cc/media/photos/350235/00001.webp?v=1")
            .unwrap();
        assert_eq!(
            page_image(&page),
            Some(("350235".to_string(), "00001".to_string()))
        );

        // Covers are not scrambled pages.
        let cover =
            Url::parse("https://cdn-msp.jmapiproxy1.cc/media/albums/350235_3x4.jpg").unwrap();
        assert_eq!(page_image(&cover), None);

        // Unknown hosts and path traversal are rejected.
        let foreign = Url::parse("https://example.com/media/photos/350235/00001.webp").unwrap();
        assert_eq!(page_image(&foreign), None);
        let traversal =
            Url::parse("https://cdn-msp.jmapiproxy1.cc/media/photos/350235/..%2Fevil.webp")
                .unwrap();
        assert_eq!(page_image(&traversal), None);
    }

    #[test]
    fn decrypts_a_known_payload() {
        assert_eq!(decrypt(CIPHERTEXT, 1_700_000_000).unwrap(), PLAINTEXT);
    }

    #[test]
    fn decodes_an_encrypted_envelope() {
        let body = format!(r#"{{"code":200,"data":"{CIPHERTEXT}"}}"#);
        let album: Album = decode(body.as_bytes(), 1_700_000_000).unwrap().unwrap();
        assert_eq!(album.name, "test");
    }

    #[test]
    fn empty_data_means_missing() {
        let album: Option<Album> = decode(br#"{"code":200,"data":[]}"#, 1_700_000_000).unwrap();
        assert!(album.is_none());
    }

    #[test]
    fn envelope_errors_keep_the_http_status_code() {
        let error =
            decode::<Album>(br#"{"code":403,"errorMsg":"restricted"}"#, 1_700_000_000).unwrap_err();
        assert_eq!(error.code(), crate::error::ErrorCode::HttpStatus);
    }

    #[test]
    fn rejects_payloads_with_bad_padding() {
        let ciphertext = BASE64.encode(&[0u8; 16]);
        assert!(decrypt(&ciphertext, 1_700_000_000).is_err());
        assert!(decrypt("not base64!", 1_700_000_000).is_err());
        assert!(decrypt("AAAA", 1_700_000_000).is_err());
    }

    #[test]
    fn formats_dates_in_jm_timezone() {
        assert_eq!(format_date(1_655_254_028).as_deref(), Some("2022-06-15"));
        assert_eq!(format_date(0), None);
        assert_eq!(format_date(-1), None);
    }

    #[test]
    fn builds_api_urls() {
        let search = search_url("www.cdngwc.cc", "test 1", 2).unwrap();
        let url = search.as_str();
        assert!(url.starts_with("https://www.cdngwc.cc/search?"));
        assert!(url.contains("search_query=test+1"));
        assert!(url.contains("main_tag=0"));
        assert!(url.contains("o=mr"));
        assert!(url.contains("t=a"));
        assert!(url.contains("page=2"));

        assert_eq!(
            album_url("www.cdnhjk.net", "350234").unwrap().as_str(),
            "https://www.cdnhjk.net/album?id=350234"
        );
    }

    #[test]
    fn image_alternates_keep_the_path_and_change_only_the_host() {
        let url = Url::parse("https://cdn-msp.jmapiproxy1.cc/media/photos/350235/00001.webp?v=1")
            .unwrap();
        let alternates = image_alternates(&url);

        assert_eq!(alternates.len(), IMAGE_DOMAINS.len() - 1);
        assert!(
            alternates
                .iter()
                .all(|candidate| candidate.path() == url.path())
        );
        assert!(
            alternates
                .iter()
                .all(|candidate| candidate.query() == url.query())
        );
        assert!(
            alternates
                .iter()
                .all(|candidate| candidate.host_str() != url.host_str())
        );

        assert!(image_alternates(&Url::parse("https://example.com/a.jpg").unwrap()).is_empty());
        assert!(image_alternates(&Url::parse("https://www.cdnhjk.net/a.jpg").unwrap()).is_empty());
    }

    #[test]
    fn recognizes_api_hosts() {
        assert!(is_api_host("www.cdnhjk.net"));
        assert!(is_api_host("www.cdngwc.club"));
        assert!(!is_api_host("cdn-msp.jmapiproxy1.cc"));
        assert!(!is_api_host("18comic.vip"));
    }

    #[test]
    fn derives_cover_urls_from_the_album_id() {
        assert_eq!(
            cover_url("1480638"),
            "https://cdn-msp.jmapiproxy1.cc/media/albums/1480638_3x4.jpg"
        );
    }

    #[test]
    fn compares_dotted_versions() {
        assert!(version_is_newer("2.1.13", "2.1.12"));
        assert!(version_is_newer("2.10.0", "2.9.9"));
        assert!(version_is_newer("3", "2.99.99"));
        assert!(!version_is_newer("2.1.12", "2.1.12"));
        assert!(!version_is_newer("2.1.11", "2.1.12"));
        // A malformed response must not downgrade the signed version.
        assert!(!version_is_newer("not-a-version", "2.1.12"));
        assert!(!version_is_newer("2.1.13", "not-a-version"));
    }

    #[test]
    fn decodes_the_setting_version() {
        let ts = 1_700_000_000;
        let payload = r#"{"jm3_version":"2.1.20","app_img_shunt":1}"#;
        let body = format!(
            r#"{{"code":200,"data":"{}"}}"#,
            encrypt_for_test(payload, ts)
        );

        let setting: Setting = decode(body.as_bytes(), ts).unwrap().unwrap();
        assert_eq!(setting.jm3_version.as_deref(), Some("2.1.20"));
    }

    #[test]
    fn lenient_scalars_accept_numbers_and_strings() {
        assert_eq!(Scalar::Int(42).as_u64(), Some(42));
        assert_eq!(Scalar::Text("42".into()).as_u64(), Some(42));
        assert_eq!(Scalar::Text(" 42 ".into()).as_i64(), Some(42));
        assert_eq!(Scalar::Int(-1).as_u64(), None);
        assert_eq!(Scalar::Text("x".into()).as_u64(), None);
        assert_eq!(Scalar::Int(42).text(), "42");
    }
}
