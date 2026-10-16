//! The official Rust SDK for [usend](https://usend.et), the email API for developers.
//!
//! A blocking client with no async runtime requirement. The only required dependencies are
//! `serde` and `serde_json`; HTTP is pluggable through the [`Transport`] trait.
//!
//! ```no_run
//! use usend::{Client, SendEmail};
//!
//! # fn main() -> usend::Result<()> {
//! let client = Client::new("us_...")?; // "" reads USEND_API_KEY
//! let sent = client.emails().send(
//!     &SendEmail::new("Acme <hello@acme.com>", ["ann@example.com"])
//!         .subject("Hello")
//!         .html("<p>It works.</p>")
//!         .idempotency_key("welcome-ann-1"),
//! )?;
//! println!("queued {}", sent.id);
//! # Ok(())
//! # }
//! ```
//!
//! # HTTPS
//!
//! The built-in [`StdTransport`] speaks plain `http://` only (local testing). For
//! `https://api.usend.et` enable the cargo feature `ureq` or `reqwest` (the client then uses it
//! by default), or implement [`Transport`] for the HTTP client you already have.
//!
//! # Async
//!
//! Calls block. In an async runtime run them on a blocking thread, for example
//! `tokio::task::spawn_blocking(move || client.emails().send(&email))` (`Client` is `Clone`).

mod client;
mod crypto;
mod error;
mod resources;
mod smtp;
mod transport;
mod types;
mod webhook;

pub use client::{Client, ClientBuilder, RequestOptions, DEFAULT_BASE_URL, VERSION};
pub use error::{ApiError, Error, Result};
pub use resources::*;
pub use smtp::{smtp_config, SmtpConfig, DEFAULT_SMTP_HOST};
pub use transport::{BoxError, HttpRequest, HttpResponse, StdTransport, Transport};
#[cfg(feature = "reqwest")]
pub use transport::ReqwestTransport;
#[cfg(feature = "ureq")]
pub use transport::UreqTransport;
pub use types::*;
pub use webhook::{verify_webhook, verify_webhook_at, verify_webhook_headers, WebhookError, WebhookEvent, DEFAULT_TOLERANCE};
