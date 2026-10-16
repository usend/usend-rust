//! Webhook signature verification (Standard Webhooks).

use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::crypto::{base64_decode, constant_time_eq, hmac_sha256};

/// How far a webhook timestamp may be from now.
pub const DEFAULT_TOLERANCE: Duration = Duration::from_secs(300);

/// A verified webhook payload. `data` depends on `event_type`; read it as JSON or
/// `serde_json::from_value` it into your own struct.
#[derive(Debug, Clone)]
pub struct WebhookEvent {
    /// The `webhook-id` header; unique per delivery.
    pub id: String,
    /// For example `email.delivered`.
    pub event_type: String,
    pub created_at: String,
    pub data: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebhookError {
    /// `webhook-id`, `webhook-timestamp` or `webhook-signature` is missing.
    MissingHeaders,
    /// The timestamp is too old or too new.
    Timestamp,
    /// The signing secret is not a valid `whsec_...` value.
    Secret,
    /// No signature matches.
    Signature,
    /// The signature is fine but the body is not the expected JSON.
    Payload(String),
}

impl fmt::Display for WebhookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WebhookError::MissingHeaders => f.write_str("usend: missing webhook-id, webhook-timestamp or webhook-signature header"),
            WebhookError::Timestamp => f.write_str("usend: webhook timestamp is too old or too new"),
            WebhookError::Secret => f.write_str("usend: invalid webhook signing secret"),
            WebhookError::Signature => f.write_str("usend: invalid webhook signature"),
            WebhookError::Payload(m) => write!(f, "usend: invalid webhook payload: {m}"),
        }
    }
}

impl std::error::Error for WebhookError {}

/// Verifies a webhook delivery and returns the parsed event.
///
/// * `payload` is the raw request body, byte for byte (do not parse and re-serialize it first).
/// * `header` looks up a request header by name, case-insensitively, for example with axum:
///   `|n| headers.get(n).and_then(|v| v.to_str().ok()).map(str::to_string)`.
/// * `secret` is the webhook's signing secret (`whsec_...`).
///
/// The timestamp must be within [`DEFAULT_TOLERANCE`] of now; use [`verify_webhook_at`] to change it.
pub fn verify_webhook<F>(payload: &[u8], header: F, secret: &str) -> Result<WebhookEvent, WebhookError>
where
    F: Fn(&str) -> Option<String>,
{
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    verify_webhook_at(payload, header, secret, now, DEFAULT_TOLERANCE)
}

/// Like [`verify_webhook`] with an explicit `now` (Unix seconds) and tolerance.
pub fn verify_webhook_at<F>(payload: &[u8], header: F, secret: &str, now: u64, tolerance: Duration) -> Result<WebhookEvent, WebhookError>
where
    F: Fn(&str) -> Option<String>,
{
    // usend sends both header sets: webhook-* and svix-*.
    let get = |name: &str| header(&format!("webhook-{name}")).or_else(|| header(&format!("svix-{name}")));
    let (id, ts, sigs) = match (get("id"), get("timestamp"), get("signature")) {
        (Some(a), Some(b), Some(c)) if !a.is_empty() && !b.is_empty() && !c.is_empty() => (a, b, c),
        _ => return Err(WebhookError::MissingHeaders),
    };
    let ts_secs: u64 = ts.trim().parse().map_err(|_| WebhookError::Timestamp)?;
    let diff = if now >= ts_secs { now - ts_secs } else { ts_secs - now };
    if diff > tolerance.as_secs() {
        return Err(WebhookError::Timestamp);
    }
    let key = base64_decode(secret.strip_prefix("whsec_").unwrap_or(secret)).filter(|k| !k.is_empty()).ok_or(WebhookError::Secret)?;

    let mut signed = Vec::with_capacity(id.len() + ts.len() + payload.len() + 2);
    signed.extend_from_slice(id.as_bytes());
    signed.push(b'.');
    signed.extend_from_slice(ts.as_bytes());
    signed.push(b'.');
    signed.extend_from_slice(payload);
    let expected = hmac_sha256(&key, &signed);

    let mut ok = false;
    for part in sigs.split_whitespace() {
        if let Some(sig) = part.strip_prefix("v1,") {
            if let Some(got) = base64_decode(sig) {
                // No early exit: every candidate is compared in constant time.
                ok |= constant_time_eq(&got, &expected);
            }
        }
    }
    if !ok {
        return Err(WebhookError::Signature);
    }

    let v: Value = serde_json::from_slice(payload).map_err(|e| WebhookError::Payload(e.to_string()))?;
    let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    Ok(WebhookEvent { id, event_type: text("type"), created_at: text("created_at"), data: v.get("data").cloned().unwrap_or(Value::Null) })
}

/// Convenience for header lists: `verify_webhook_headers(body, &[("webhook-id", "msg_1"), ...], secret)`.
pub fn verify_webhook_headers(payload: &[u8], headers: &[(&str, &str)], secret: &str) -> Result<WebhookEvent, WebhookError> {
    verify_webhook(payload, |n| headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(n)).map(|(_, v)| v.to_string()), secret)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::base64_encode;

    fn sign(secret_b64: &str, id: &str, ts: u64, body: &str) -> String {
        let key = base64_decode(secret_b64).unwrap();
        let mac = hmac_sha256(&key, format!("{id}.{ts}.{body}").as_bytes());
        format!("v1,{}", base64_encode(&mac))
    }

    fn lookup<'a>(id: &'a str, ts: String, sig: String) -> impl Fn(&str) -> Option<String> + 'a {
        move |n| match n {
            "webhook-id" => Some(id.to_string()),
            "webhook-timestamp" => Some(ts.clone()),
            "webhook-signature" => Some(sig.clone()),
            _ => None,
        }
    }

    const BODY: &str = r#"{"type":"email.delivered","created_at":"2026-10-01T10:00:00Z","data":{"email_id":"e1"}}"#;

    #[test]
    fn accepts_valid_signature() {
        let secret_b64 = base64_encode(b"super-secret-key-bytes");
        let secret = format!("whsec_{secret_b64}");
        let ts = 1_760_000_000u64;
        let sig = sign(&secret_b64, "msg_1", ts, BODY);
        let ev = verify_webhook_at(BODY.as_bytes(), lookup("msg_1", ts.to_string(), sig), &secret, ts + 10, DEFAULT_TOLERANCE).unwrap();
        assert_eq!(ev.event_type, "email.delivered");
        assert_eq!(ev.id, "msg_1");
        assert_eq!(ev.data["email_id"], "e1");
    }

    #[test]
    fn accepts_second_signature_in_list() {
        let secret_b64 = base64_encode(b"k");
        let secret = format!("whsec_{secret_b64}");
        let ts = 1_760_000_000u64;
        let sig = format!("v1,AAAA {}", sign(&secret_b64, "m", ts, BODY));
        assert!(verify_webhook_at(BODY.as_bytes(), lookup("m", ts.to_string(), sig), &secret, ts, DEFAULT_TOLERANCE).is_ok());
    }

    #[test]
    fn rejects_tampering_wrong_secret_and_old_timestamps() {
        let secret_b64 = base64_encode(b"super-secret-key-bytes");
        let secret = format!("whsec_{secret_b64}");
        let ts = 1_760_000_000u64;
        let sig = sign(&secret_b64, "msg_1", ts, BODY);

        let tampered = format!("{BODY} ");
        let r = verify_webhook_at(tampered.as_bytes(), lookup("msg_1", ts.to_string(), sig.clone()), &secret, ts, DEFAULT_TOLERANCE);
        assert_eq!(r.unwrap_err(), WebhookError::Signature);

        let other = format!("whsec_{}", base64_encode(b"another"));
        let r = verify_webhook_at(BODY.as_bytes(), lookup("msg_1", ts.to_string(), sig.clone()), &other, ts, DEFAULT_TOLERANCE);
        assert_eq!(r.unwrap_err(), WebhookError::Signature);

        let r = verify_webhook_at(BODY.as_bytes(), lookup("msg_1", ts.to_string(), sig.clone()), &secret, ts + 3600, DEFAULT_TOLERANCE);
        assert_eq!(r.unwrap_err(), WebhookError::Timestamp);
        let r = verify_webhook_at(BODY.as_bytes(), lookup("msg_1", ts.to_string(), sig), &secret, ts + 3600, Duration::from_secs(7200));
        assert!(r.is_ok());
    }

    #[test]
    fn reports_missing_headers_and_bad_secret() {
        let r = verify_webhook_at(BODY.as_bytes(), |_| None, "whsec_abcd", 0, DEFAULT_TOLERANCE);
        assert_eq!(r.unwrap_err(), WebhookError::MissingHeaders);
        let r = verify_webhook_at(BODY.as_bytes(), lookup("m", "5".into(), "v1,AAAA".into()), "whsec_***", 5, DEFAULT_TOLERANCE);
        assert_eq!(r.unwrap_err(), WebhookError::Secret);
    }

    #[test]
    fn svix_headers_work_too() {
        let secret_b64 = base64_encode(b"k2");
        let secret = format!("whsec_{secret_b64}");
        let ts = 1_760_000_000u64;
        let sig = sign(&secret_b64, "m", ts, BODY);
        let headers = [("svix-id", "m".to_string()), ("svix-timestamp", ts.to_string()), ("svix-signature", sig)];
        let f = |n: &str| headers.iter().find(|(k, _)| *k == n).map(|(_, v)| v.clone());
        assert!(verify_webhook_at(BODY.as_bytes(), f, &secret, ts, DEFAULT_TOLERANCE).is_ok());
    }
}
