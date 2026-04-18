//! Centralized HTTP plumbing for the SpeleoDB v2 API.
//!
//! All endpoint functions in this crate go through `send_json` (for JSON
//! responses) or `send_raw` (for binary responses such as ZIP downloads).
//! Status codes map to typed `Error` variants in a single place; success
//! bodies deserialize directly into the caller's target type without any
//! `data:` wrapper, matching the v2 contract.

use common::{ApiInfo, Error};
use log::error;
use reqwest::{RequestBuilder, Response, StatusCode};
use serde::{Deserialize, de::DeserializeOwned};
use url::Url;

/// V2 path prefix appended to the configured `instance` URL.
const API_V2_PREFIX: &str = "api/v2/";

/// Build a fully-qualified v2 URL by joining `instance + "api/v2/" + path`.
///
/// Force a trailing slash on the instance path before joining: per RFC 3986
/// reference resolution, `Url::join` *replaces* the last path segment when
/// the base does not end in `/`. Without this guard, a self-hosted instance
/// at `https://example.com/speleodb` would silently route requests to
/// `https://example.com/api/v2/...`, dropping the `/speleodb` prefix.
///
/// Panics on join failure since all callers pass static, well-formed paths;
/// a runtime error here indicates a programmer bug, not a user-facing
/// condition.
pub(crate) fn v2_url(instance: &Url, path: &str) -> Url {
    let mut base = instance.clone();
    if !base.path().ends_with('/') {
        let with_slash = format!("{}/", base.path());
        base.set_path(&with_slash);
    }
    base.join(API_V2_PREFIX)
        .and_then(|u| u.join(path))
        .expect("static API path must join cleanly with instance URL")
}

/// Inject the bearer token header, or fail fast if the user is unauthenticated.
pub(crate) fn authenticated(
    builder: RequestBuilder,
    api_info: &ApiInfo,
) -> Result<RequestBuilder, Error> {
    let token = api_info.oauth_token().ok_or(Error::NoAuthToken)?;
    Ok(builder.header("Authorization", format!("Token {token}")))
}

/// Send a request and deserialize a JSON success body into `T`.
///
/// On non-2xx responses, the body is parsed for the v2 `{"error": "..."}`
/// shape and dispatched to the typed error variant matching the status.
pub(crate) async fn send_json<T: DeserializeOwned>(builder: RequestBuilder) -> Result<T, Error> {
    let resp = builder
        .send()
        .await
        .map_err(|e| Error::NetworkRequest(e.to_string()))?;
    let status = resp.status();
    if status.is_success() {
        return resp.json::<T>().await.map_err(|e| {
            error!("Failed to deserialize success response: {e}");
            Error::Deserialization(e.to_string())
        });
    }
    Err(error_from_response(status, resp).await)
}

/// Send a request whose success body is consumed by the caller (e.g. raw bytes).
///
/// Returns the underlying `Response` on 2xx so the caller can call
/// `.bytes()`, stream, etc. Non-2xx responses go through the same status
/// mapping as `send_json`.
pub(crate) async fn send_raw(builder: RequestBuilder) -> Result<Response, Error> {
    let resp = builder
        .send()
        .await
        .map_err(|e| Error::NetworkRequest(e.to_string()))?;
    let status = resp.status();
    if status.is_success() {
        return Ok(resp);
    }
    Err(error_from_response(status, resp).await)
}

/// Drain the failed response body and convert it to a typed `Error`.
async fn error_from_response(status: StatusCode, resp: Response) -> Error {
    let body = resp.text().await.ok();
    map_status_to_error(status, body.as_deref())
}

/// Maximum length (in bytes) of an error message stored in `Error` variants.
///
/// Servers fronted by CDNs (Cloudflare, nginx) can return multi-kilobyte
/// HTML error pages on 502/503; without a cap, the entire blob ends up in
/// the user-facing modal and in Sentry breadcrumbs. 512 bytes is enough for
/// any reasonable JSON error message and short enough to render cleanly.
const MAX_ERROR_MESSAGE_BYTES: usize = 512;

/// Single source of truth for HTTP-status → `Error` mapping.
///
/// Pulls the message from a JSON `{"error": "..."}` body when present;
/// otherwise falls back to the raw body text or the status canonical reason.
pub(crate) fn map_status_to_error(status: StatusCode, body: Option<&str>) -> Error {
    let raw = extract_message(body)
        .or_else(|| body.map(str::to_owned))
        .unwrap_or_else(|| {
            status
                .canonical_reason()
                .unwrap_or("unknown error")
                .to_string()
        });
    let message = cap_error_message(raw);
    match status.as_u16() {
        401 | 403 => Error::Unauthorized(message),
        404 => Error::NotFound(message),
        409 | 423 => Error::Conflict(message),
        422 => Error::Unprocessable(message),
        s => Error::Api { status: s, message },
    }
}

/// Truncate `s` to at most `MAX_ERROR_MESSAGE_BYTES`, snapping back to the
/// nearest UTF-8 char boundary so we never produce invalid UTF-8.
fn cap_error_message(s: String) -> String {
    if s.len() <= MAX_ERROR_MESSAGE_BYTES {
        return s;
    }
    let mut end = MAX_ERROR_MESSAGE_BYTES;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    let mut truncated = s[..end].to_owned();
    truncated.push_str("…[truncated]");
    truncated
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    error: Option<String>,
}

fn extract_message(body: Option<&str>) -> Option<String> {
    serde_json::from_str::<ErrorBody>(body?).ok()?.error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_unauthorized_status_with_json_body() {
        let err = map_status_to_error(StatusCode::UNAUTHORIZED, Some(r#"{"error": "bad token"}"#));
        assert!(matches!(err, Error::Unauthorized(ref m) if m == "bad token"));
    }

    #[test]
    fn map_forbidden_collapses_to_unauthorized() {
        let err = map_status_to_error(StatusCode::FORBIDDEN, Some(r#"{"error": "nope"}"#));
        assert!(matches!(err, Error::Unauthorized(ref m) if m == "nope"));
    }

    #[test]
    fn map_not_found_status() {
        let err = map_status_to_error(StatusCode::NOT_FOUND, Some(r#"{"error": "missing"}"#));
        assert!(matches!(err, Error::NotFound(ref m) if m == "missing"));
    }

    #[test]
    fn map_unprocessable_status() {
        let err = map_status_to_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            Some(r#"{"error": "bad"}"#),
        );
        assert!(matches!(err, Error::Unprocessable(ref m) if m == "bad"));
    }

    #[test]
    fn map_conflict_and_locked_collapse_to_conflict() {
        let conflict = map_status_to_error(StatusCode::CONFLICT, Some(r#"{"error": "c"}"#));
        let locked = map_status_to_error(StatusCode::LOCKED, Some(r#"{"error": "l"}"#));
        assert!(matches!(conflict, Error::Conflict(ref m) if m == "c"));
        assert!(matches!(locked, Error::Conflict(ref m) if m == "l"));
    }

    #[test]
    fn map_server_error_falls_through_to_api_variant() {
        let err = map_status_to_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            Some(r#"{"error": "boom"}"#),
        );
        assert!(matches!(
            err,
            Error::Api { status: 500, ref message } if message == "boom"
        ));
    }

    #[test]
    fn falls_back_to_raw_body_when_not_json() {
        let err = map_status_to_error(StatusCode::BAD_REQUEST, Some("plain text"));
        assert!(matches!(
            err,
            Error::Api { status: 400, ref message } if message == "plain text"
        ));
    }

    #[test]
    fn falls_back_to_canonical_reason_when_body_missing() {
        let err = map_status_to_error(StatusCode::BAD_GATEWAY, None);
        assert!(matches!(
            err,
            Error::Api { status: 502, ref message } if message == "Bad Gateway"
        ));
    }

    #[test]
    fn handles_empty_string_body() {
        let err = map_status_to_error(StatusCode::INTERNAL_SERVER_ERROR, Some(""));
        assert!(matches!(err, Error::Api { status: 500, .. }));
    }

    #[test]
    fn caps_oversized_html_error_body() {
        // Simulate a Cloudflare-style HTML 502 page. Without a cap, the full
        // multi-kilobyte payload would land in the user-facing modal.
        let html = "<!DOCTYPE html><html><body>".to_string() + &"x".repeat(8192) + "</body></html>";
        let err = map_status_to_error(StatusCode::BAD_GATEWAY, Some(&html));
        let Error::Api { message, .. } = err else {
            panic!("expected Api variant for 502");
        };
        assert!(
            message.len() < 1024,
            "message must be capped, got {} bytes",
            message.len()
        );
        assert!(
            message.ends_with("…[truncated]"),
            "truncated marker missing: {message}"
        );
    }

    #[test]
    fn caps_oversized_json_error_message() {
        // Even a JSON-parsed message is capped — a buggy server returning a
        // huge `{"error": "..."}` would otherwise bypass the safeguard.
        let big = "y".repeat(4096);
        let body = format!(r#"{{"error": "{big}"}}"#);
        let err = map_status_to_error(StatusCode::INTERNAL_SERVER_ERROR, Some(&body));
        let Error::Api { message, .. } = err else {
            panic!("expected Api variant for 500");
        };
        assert!(message.len() < 1024, "message must be capped: {message}");
        assert!(message.ends_with("…[truncated]"));
    }

    #[test]
    fn cap_respects_utf8_char_boundaries() {
        // Multi-byte chars near the cut point must not produce invalid UTF-8.
        // 'é' is 2 bytes; pad with multi-byte chars so the naive cut would
        // land mid-codepoint.
        let s = "é".repeat(MAX_ERROR_MESSAGE_BYTES);
        let capped = cap_error_message(s);
        // String is valid UTF-8 by construction; the assertion is that we
        // didn't panic during the slice. Sanity-check the suffix.
        assert!(capped.ends_with("…[truncated]"));
    }

    #[test]
    fn v2_url_appends_path_correctly() {
        let instance = Url::parse("https://stage.speleodb.org").unwrap();
        let url = v2_url(&instance, "projects/");
        assert_eq!(url.as_str(), "https://stage.speleodb.org/api/v2/projects/");
    }

    #[test]
    fn v2_url_handles_nested_paths() {
        let instance = Url::parse("https://stage.speleodb.org/").unwrap();
        let url = v2_url(&instance, "projects/abc-123/acquire/");
        assert_eq!(
            url.as_str(),
            "https://stage.speleodb.org/api/v2/projects/abc-123/acquire/"
        );
    }

    #[test]
    fn v2_url_preserves_path_prefix_without_trailing_slash() {
        // Self-hosted SpeleoDB at a sub-path: the auth screen trims trailing
        // slashes on blur, so this is the shape that actually reaches the
        // backend. Without the trailing-slash guard, Url::join would replace
        // the "speleodb" segment and drop the prefix entirely.
        let instance = Url::parse("https://intranet.example.com/speleodb").unwrap();
        let url = v2_url(&instance, "projects/");
        assert_eq!(
            url.as_str(),
            "https://intranet.example.com/speleodb/api/v2/projects/"
        );
    }

    #[test]
    fn v2_url_preserves_path_prefix_with_trailing_slash() {
        let instance = Url::parse("https://intranet.example.com/speleodb/").unwrap();
        let url = v2_url(&instance, "projects/abc-123/acquire/");
        assert_eq!(
            url.as_str(),
            "https://intranet.example.com/speleodb/api/v2/projects/abc-123/acquire/"
        );
    }
}
