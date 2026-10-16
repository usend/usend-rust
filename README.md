# usend for Rust

The official Rust SDK for [usend](https://usend.et), the email API for developers.

- Blocking client, no async runtime required.
- **The only required dependencies are `serde` and `serde_json`.** HTTP, SHA-256/HMAC (webhooks) and base64 are implemented in the crate.
- HTTP is pluggable: a `Transport` trait, a plain-`http://` transport built on `std::net::TcpStream` (for localhost testing), and optional cargo features `ureq` and `reqwest` that add HTTPS. Those two are optional dependencies that are off by default.

> Status: this crate has not been compiled or tested yet (the toolchain was not available when it was written). Expect to fix small compile errors on the first `cargo build`.

## Install

```toml
[dependencies]
usend = { version = "0.1", features = ["ureq"] }   # or features = ["reqwest"]
```

`https://api.usend.et` needs one of the two features, or your own `Transport` (below). With neither, the client only talks to `http://` URLs.

## Quick start

```rust
use usend::{Client, SendEmail};

fn main() -> usend::Result<()> {
    let client = Client::new(std::env::var("USEND_API_KEY").unwrap())?; // "" also reads USEND_API_KEY

    let sent = client.emails().send(
        &SendEmail::new("Acme <hello@acme.com>", ["ann@example.com"])
            .subject("Hello")
            .html("<p>It works.</p>")
            .idempotency_key("welcome-ann-1"),
    )?;
    println!("queued {}", sent.id);
    Ok(())
}
```

`Client` is cheap to clone and `Send + Sync`. In an async runtime, call it from a blocking thread: `tokio::task::spawn_blocking(move || client.emails().send(&email))`.

### Options

```rust
use std::time::Duration;
let client = usend::Client::builder()
    .api_key("us_...")
    .base_url("https://api.example.com") // default https://api.usend.et (or USEND_BASE_URL)
    .timeout(Duration::from_secs(10))     // per attempt, default 30s
    .max_retries(3)                       // default 2, 0 disables
    .build()?;
```

Requests that fail with `429` or `5xx` are retried with exponential backoff, and `Retry-After` (seconds) is honored (waits over 60 seconds are not attempted). A `429` is always retried. A `5xx` is retried only when a duplicate is impossible: `GET`, `DELETE`, or a request with an idempotency key.

## Resources

| Accessor | Methods |
|---|---|
| `client.emails()` | `send`, `batch`, `get`, `list`, `update` (reschedule), `cancel`, `events`, `attachments`, `attachment`, `metrics`, `share`, `send_test`, `insights` |
| `client.domains()` | `create`, `list`, `get`, `update`, `verify`, `auto_configure`, `remove`, `claim`, `get_claim`, `verify_claim`, `insights` |
| `client.api_keys()` | `create`, `list`, `update`, `remove` |
| `client.templates()` | `create`, `list`, `get`, `update`, `publish`, `duplicate`, `remove`, `versions`, `get_version`, `restore_version` |
| `client.contacts()` | `create`, `list`, `get`, `update`, `remove`, `import`, `create_import`, `get_import`, `list_imports`, `stats`, `segments`, `add_to_segment`, `remove_from_segment`, `topics`, `update_topics` |
| `client.segments()`, `client.audiences()` | `create`, `list`, `get`, `update`, `remove`, `contacts`, `metrics`, plus nested contact calls on audiences |
| `client.topics()`, `client.properties()` | topics and contact properties |
| `client.broadcasts()` | `create`, `list`, `get`, `update`, `send`, `test`, `remove`, `cancel`, `duplicate`, `recipients`, `clicked_links`, `performance` |
| `client.events()` | custom events: `create`, `list`, `get`, `update`, `remove`, `send` |
| `client.automations()` | `create`, `list`, `get`, `update`, `remove`, `duplicate`, `stop`, `runs`, `get_run` |
| `client.webhooks()` | `create`, `list`, `get`, `update`, `remove`, `deliveries`, `replay`, `rotate_secret`, `events`, `get_event`, `replay_event`, `event_attempts` |
| `client.suppressions()` | `list`, `get`, `create`, `batch_create`, `batch_remove`, `remove` |
| `client.inbound()` | `list`, `get` (received mail) |
| `client.inboxes()` | inboxes, labels, threads, messages, `reply`, `forward`, drafts |
| `client.stats(days)`, `client.dmarc_check(domain)` | |

Lists return `List<T>` with `data` and `has_more`; paginate with `ListParams::new().limit(50).after(id)`.

```rust
use usend::{RecipientType, BroadcastRecipientsParams, RequestOptions};

// The event type is required.
let clicked = client.broadcasts().recipients("<id>", RecipientType::Clicked, &BroadcastRecipientsParams::default())?;

// Batch with permissive validation.
let res = client.emails().batch(&emails, &RequestOptions::new().batch_validation("permissive"))?;
```

Parameter structs implement `Default`, so use struct-update syntax: `CreateContact { first_name: Some("Ann".into()), ..CreateContact::new("ann@example.com") }`. Timestamps are RFC 3339 strings.

### Anything else

```rust
let logs = client.request("GET", "/logs?days=7", None, &usend::RequestOptions::new())?; // serde_json::Value
```

OAuth access tokens (`usa_...`) work as the API key. The OAuth endpoints themselves and session-only account calls (two-factor, billing, org settings) have no typed methods; use `request`.

## SMTP

```rust
let cfg = client.smtp_config(Some(587))?; // 465 = implicit TLS, 587 = STARTTLS
// cfg.host ("smtp.usend.et"), cfg.port, cfg.username ("usend"), cfg.password (your API key), cfg.implicit_tls
```

Pass these to any SMTP crate such as `lettre`. The host is derived from the base URL; `USEND_SMTP_HOST` overrides it.

## Webhooks

Verify against the raw body (Standard Webhooks, secret `whsec_...`, 5 minutes tolerance):

```rust
use usend::verify_webhook;

// axum: `body: Bytes`, `headers: HeaderMap`
let event = verify_webhook(
    &body,
    |name| headers.get(name).and_then(|v| v.to_str().ok()).map(str::to_string),
    &std::env::var("USEND_WEBHOOK_SECRET").unwrap(),
)
.map_err(|_| StatusCode::BAD_REQUEST)?;

match event.event_type.as_str() {
    "email.delivered" => { /* event.data["email_id"] */ }
    _ => {}
}
```

`WebhookError` says why it failed: `MissingHeaders`, `Timestamp`, `Secret`, `Signature`, `Payload`. `verify_webhook_at` takes an explicit clock and tolerance.

## Errors

```rust
match client.emails().send(&email) {
    Err(usend::Error::Api(e)) if e.status_code == 429 => { /* e.retry_after */ }
    Err(usend::Error::Api(e)) => eprintln!("{} {}: {}", e.status_code, e.name, e.message),
    Err(e) => eprintln!("{e}"), // Transport, Decode, Config
    Ok(_) => {}
}
```

## Using your own HTTP client

```rust
use usend::{BoxError, HttpRequest, HttpResponse, Transport};

struct MyTransport;
impl Transport for MyTransport {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, BoxError> {
        // Perform req.method / req.url / req.headers / req.body.
        // Return Ok for every HTTP status (including 4xx/5xx); Err only if there was no response.
        todo!()
    }
}
let client = usend::Client::builder().api_key("us_...").transport(MyTransport).build()?;
```

## Plain HTTP

The SDK is a thin layer over the REST API; you can always call it directly with `Authorization: Bearer us_...`. See the [API reference](https://usend.et/docs/api-reference/introduction).

## Development

```bash
cd sdk/rust && cargo test            # unit tests + a localhost mock-server test
cargo test --features ureq           # also compiles the ureq transport
```

MIT licensed.
