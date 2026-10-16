//! Print the SMTP settings for your key (feed them to `lettre` or any SMTP client).
//!
//!     USEND_API_KEY=us_... cargo run --example smtp

use usend::Client;

fn main() {
    let client = Client::from_env().expect("set USEND_API_KEY");
    let cfg = client.smtp_config(Some(587)).unwrap();
    println!("host {}  port {}  starttls (implicit TLS: {})", cfg.host, cfg.port, cfg.implicit_tls);
    println!("username {}  password <your API key>", cfg.username);
}
