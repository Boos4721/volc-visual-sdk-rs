//! Volcengine Signature V4 (HMAC-SHA256).
//!
//! This is a faithful Rust port of the algorithm implemented in the official
//! Python (`volcengine/auth/SignerV4.py`) and Java (`SignerV4Impl.java`) SDKs.
//! The steps are:
//!
//! 1. Build the *canonical request* from method, normalized URI, normalized
//!    query, the canonical (sorted, lower-cased) signed headers, the signed
//!    header list and the hex SHA-256 of the body.
//! 2. Build the *string to sign* from the algorithm, the `X-Date`, the
//!    credential scope (`date/region/service/request`) and the SHA-256 of the
//!    canonical request.
//! 3. Derive the signing key with four chained HMAC-SHA256 rounds
//!    (`date` -> `region` -> `service` -> `"request"`).
//! 4. HMAC-SHA256 the string to sign with that key; the hex digest is the
//!    signature placed into the `Authorization` header.

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

use crate::credentials::Credentials;

type HmacSha256 = Hmac<Sha256>;

/// The fixed algorithm label used in the string-to-sign and auth header.
pub const ALGORITHM: &str = "HMAC-SHA256";

/// A single query parameter (already in its raw, un-encoded form).
#[derive(Debug, Clone)]
pub struct QueryParam {
    /// Parameter name.
    pub key: String,
    /// Parameter value.
    pub value: String,
}

/// The minimal description of an HTTP request that the signer needs.
#[derive(Debug, Clone)]
pub struct SignableRequest {
    /// HTTP method, e.g. `POST`.
    pub method: String,
    /// Request path; an empty path is normalized to `/`.
    pub path: String,
    /// Host header value, e.g. `visual.volcengineapi.com`.
    pub host: String,
    /// Query parameters carried in the URL.
    pub query: Vec<QueryParam>,
    /// Raw request body bytes (may be empty).
    pub body: Vec<u8>,
    /// `Content-Type` header value.
    pub content_type: String,
}

/// The headers produced by signing, ready to attach to an outgoing request.
#[derive(Debug, Clone)]
pub struct SignedHeaders {
    /// `X-Date` in ISO8601 basic format, e.g. `20230515T101010Z`.
    pub x_date: String,
    /// Hex SHA-256 of the request body (`X-Content-Sha256`).
    pub x_content_sha256: String,
    /// The fully assembled `Authorization` header value.
    pub authorization: String,
    /// Optional `X-Security-Token` echoed back when a session token is used.
    pub x_security_token: Option<String>,
    /// `Content-Type` that was signed.
    pub content_type: String,
    /// `Host` that was signed.
    pub host: String,
}

/// Hex-encoded SHA-256 of `data`.
pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts keys of any size");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// Percent-encode a single component, keeping the unreserved set
/// `A-Z a-z 0-9 - _ . ~` and encoding everything else, with space as `%20`.
///
/// Mirrors the Go-aligned `signStringEncoder` in the Java SDK and the
/// `quote(..., safe='-_.~')` behaviour in the Python SDK.
fn sign_encode(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    for &b in source.as_bytes() {
        let unreserved = b.is_ascii_alphanumeric()
            || b == b'-'
            || b == b'_'
            || b == b'.'
            || b == b'~';
        if unreserved {
            out.push(b as char);
        } else if b == b' ' {
            out.push_str("%20");
        } else {
            out.push('%');
            out.push_str(&format!("{:02X}", b));
        }
    }
    out
}

/// Normalize the request URI: encode each `/`-separated segment but keep the
/// separators intact (matches `normUri`).
fn norm_uri(path: &str) -> String {
    if path.is_empty() {
        return "/".to_string();
    }
    path.split('/')
        .map(sign_encode)
        .collect::<Vec<_>>()
        .join("/")
}

/// Build the canonical query string: sort by encoded key then value, encode
/// both sides and join with `&` (matches `normQuery`).
fn norm_query(query: &[QueryParam]) -> String {
    let mut pairs: Vec<(String, String)> = query
        .iter()
        .map(|p| (sign_encode(&p.key), sign_encode(&p.value)))
        .collect();
    pairs.sort();
    pairs
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&")
}

/// Strip a default `:80`/`:443` port from the host, as the reference signers do.
fn canonical_host(host: &str) -> String {
    if let Some((name, port)) = host.split_once(':') {
        if port == "80" || port == "443" {
            return name.to_string();
        }
    }
    host.to_string()
}

/// Derive the V4 signing key via four chained HMAC-SHA256 rounds.
pub fn signing_key(secret_key: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_sha256(secret_key.as_bytes(), date.as_bytes());
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"request")
}

/// Internal: build the canonical request string and the sorted signed-header
/// list. Returned together because the header list feeds into both the
/// canonical request and the `Authorization` header.
fn build_canonical_request(
    req: &SignableRequest,
    x_date: &str,
    body_hash: &str,
    session_token: Option<&str>,
) -> (String, String) {
    let host = canonical_host(&req.host);

    // Collect the headers that participate in the signature. The reference
    // implementations sign Content-Type, Host and every `X-*` header.
    let mut headers: Vec<(String, String)> = vec![
        ("content-type".to_string(), req.content_type.clone()),
        ("host".to_string(), host),
        ("x-content-sha256".to_string(), body_hash.to_string()),
        ("x-date".to_string(), x_date.to_string()),
    ];
    if let Some(token) = session_token {
        headers.push(("x-security-token".to_string(), token.to_string()));
    }
    headers.sort_by(|a, b| a.0.cmp(&b.0));

    let mut canonical_headers = String::new();
    for (k, v) in &headers {
        canonical_headers.push_str(k);
        canonical_headers.push(':');
        canonical_headers.push_str(v.trim());
        canonical_headers.push('\n');
    }
    let signed_header_list = headers
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join(";");

    let path = if req.path.is_empty() {
        "/".to_string()
    } else {
        norm_uri(&req.path)
    };

    let canonical_request = [
        req.method.as_str(),
        path.as_str(),
        norm_query(&req.query).as_str(),
        canonical_headers.as_str(),
        signed_header_list.as_str(),
        body_hash,
    ]
    .join("\n");

    (canonical_request, signed_header_list)
}

/// Sign `req` with `creds` at instant `x_date` (ISO8601 basic, `YYYYMMDDTHHMMSSZ`).
///
/// This is the deterministic core used by both the unit tests and the client.
/// The client supplies the current time; tests supply a fixed timestamp.
pub fn sign_with_date(
    req: &SignableRequest,
    creds: &Credentials,
    x_date: &str,
) -> SignedHeaders {
    let date = &x_date[..8];
    let body_hash = sha256_hex(&req.body);
    let session = creds.session_token.as_deref().filter(|t| !t.is_empty());

    let (canonical_request, signed_header_list) =
        build_canonical_request(req, x_date, &body_hash, session);
    let hashed_canonical = sha256_hex(canonical_request.as_bytes());

    let credential_scope = format!("{}/{}/{}/request", date, creds.region, creds.service);
    let string_to_sign = [
        ALGORITHM,
        x_date,
        credential_scope.as_str(),
        hashed_canonical.as_str(),
    ]
    .join("\n");

    let key = signing_key(&creds.secret_key, date, &creds.region, &creds.service);
    let signature = hex::encode(hmac_sha256(&key, string_to_sign.as_bytes()));

    let authorization = format!(
        "{} Credential={}/{}, SignedHeaders={}, Signature={}",
        ALGORITHM, creds.access_key, credential_scope, signed_header_list, signature
    );

    SignedHeaders {
        x_date: x_date.to_string(),
        x_content_sha256: body_hash,
        authorization,
        x_security_token: session.map(|t| t.to_string()),
        content_type: req.content_type.clone(),
        host: canonical_host(&req.host),
    }
}

/// Current UTC time formatted as ISO8601 basic (`YYYYMMDDTHHMMSSZ`).
pub fn current_x_date() -> String {
    chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fixed inputs shared by the golden-vector tests. The expected values were
    // produced by running the reference algorithm from the official Python SDK
    // (volcengine/auth/SignerV4.py + util/Util.py) over these same inputs, so a
    // match here proves byte-for-byte parity with the upstream signer.
    const AK: &str = "AKTPtestaccesskey000000000000";
    const SK: &str = "dGVzdHNlY3JldGtleTAwMDAwMDAwMDAwMDAwMDA=";
    const X_DATE: &str = "20230515T101010Z";
    const BODY: &str = r#"{"req_key":"high_aes_general_v21_L","prompt":"a cat"}"#;

    fn fixture_request() -> SignableRequest {
        SignableRequest {
            method: "POST".to_string(),
            path: "/".to_string(),
            host: "visual.volcengineapi.com".to_string(),
            query: vec![
                QueryParam {
                    key: "Action".to_string(),
                    value: "CVProcess".to_string(),
                },
                QueryParam {
                    key: "Version".to_string(),
                    value: "2022-08-31".to_string(),
                },
            ],
            body: BODY.as_bytes().to_vec(),
            content_type: "application/json".to_string(),
        }
    }

    #[test]
    fn body_sha256_matches_reference() {
        assert_eq!(
            sha256_hex(BODY.as_bytes()),
            "b89b2b4d469713b0e2b588447761edd2e542d8082ef37720605a71caf55f0559"
        );
    }

    #[test]
    fn signing_key_matches_reference() {
        let key = signing_key(SK, "20230515", "cn-north-1", "cv");
        assert_eq!(
            hex::encode(key),
            "f6b24c110dd2b7de26f037bfd00008afb9ae62f519bd7b1e9f4c7f2d4ad4508a"
        );
    }

    #[test]
    fn canonical_request_hash_matches_reference() {
        let req = fixture_request();
        let body_hash = sha256_hex(&req.body);
        let (canonical, signed_headers) =
            build_canonical_request(&req, X_DATE, &body_hash, None);

        assert_eq!(
            signed_headers,
            "content-type;host;x-content-sha256;x-date"
        );
        assert_eq!(
            sha256_hex(canonical.as_bytes()),
            "d572f00ec33bd4497e54a3d67cd10bd2bc59e68e6044767731e6055c86bb0404"
        );
    }

    #[test]
    fn full_signature_matches_reference() {
        let req = fixture_request();
        let creds = Credentials::new(AK, SK);
        let signed = sign_with_date(&req, &creds, X_DATE);

        assert_eq!(signed.x_date, X_DATE);
        assert_eq!(
            signed.x_content_sha256,
            "b89b2b4d469713b0e2b588447761edd2e542d8082ef37720605a71caf55f0559"
        );
        assert_eq!(
            signed.authorization,
            "HMAC-SHA256 Credential=AKTPtestaccesskey000000000000/20230515/cn-north-1/cv/request, \
             SignedHeaders=content-type;host;x-content-sha256;x-date, \
             Signature=635ae6b8792921876ed6c2147a9d0c14f4e9c7983ae5d691e150cb752234545d"
        );
        assert!(signed.x_security_token.is_none());
    }

    #[test]
    fn signature_with_security_token_matches_reference() {
        let req = fixture_request();
        let mut creds = Credentials::new(AK, SK);
        creds.session_token = Some("STS2eyJMVEFjY2Vzc0tleUlkIjoiQUtUUCJ9".to_string());
        let signed = sign_with_date(&req, &creds, X_DATE);

        // The Authorization signature must change once the token joins the
        // signed header set, and the token is echoed back for the client.
        assert_eq!(
            signed.x_security_token.as_deref(),
            Some("STS2eyJMVEFjY2Vzc0tleUlkIjoiQUtUUCJ9")
        );
        assert!(signed.authorization.contains(
            "SignedHeaders=content-type;host;x-content-sha256;x-date;x-security-token"
        ));
        assert!(signed.authorization.ends_with(
            "Signature=3b09326cc5c960dc751ab45229a916b4d4cd969c0ec147b353b845f4128bc52f"
        ));
    }

    #[test]
    fn sign_encode_escapes_reserved_characters() {
        assert_eq!(sign_encode("a b"), "a%20b");
        assert_eq!(sign_encode("a/b"), "a%2Fb");
        assert_eq!(sign_encode("-_.~AZ09"), "-_.~AZ09");
        assert_eq!(sign_encode("中"), "%E4%B8%AD");
    }

    #[test]
    fn norm_query_sorts_and_encodes() {
        let q = vec![
            QueryParam {
                key: "Version".to_string(),
                value: "2022-08-31".to_string(),
            },
            QueryParam {
                key: "Action".to_string(),
                value: "CVProcess".to_string(),
            },
        ];
        assert_eq!(norm_query(&q), "Action=CVProcess&Version=2022-08-31");
    }

    #[test]
    fn canonical_host_strips_default_ports() {
        assert_eq!(canonical_host("example.com:443"), "example.com");
        assert_eq!(canonical_host("example.com:80"), "example.com");
        assert_eq!(canonical_host("example.com:8080"), "example.com:8080");
        assert_eq!(canonical_host("example.com"), "example.com");
    }
}
