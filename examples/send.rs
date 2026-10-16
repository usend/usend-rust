//! Send one email.
//!
//!     USEND_API_KEY=us_... cargo run --example send --features ureq
//!
//! The `ureq` (or `reqwest`) feature gives the client HTTPS. Without one, only http:// works.

use usend::{Client, Error, SendEmail};

fn main() {
    let client = Client::from_env().expect("set USEND_API_KEY");
    let email = SendEmail::new("Acme <hello@acme.com>", ["ann@example.com"])
        .subject("Hello from Rust")
        .html("<p>It works.</p>")
        .tag("category", "welcome")
        .idempotency_key("welcome-ann-1");

    match client.emails().send(&email) {
        Ok(sent) => println!("queued {}", sent.id),
        Err(Error::Api(e)) => eprintln!("usend rejected the email: {} ({}): {}", e.name, e.status_code, e.message),
        Err(e) => eprintln!("failed: {e}"),
    }
}
