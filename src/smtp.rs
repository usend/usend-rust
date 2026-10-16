//! SMTP settings for sending through usend with any SMTP crate (for example `lettre`).

use crate::client::Client;
use crate::error::{Error, Result};

/// The hosted usend SMTP submission host.
pub const DEFAULT_SMTP_HOST: &str = "smtp.usend.et";

/// SMTP settings. The API key is the password. Port 465 is implicit TLS, port 587 is STARTTLS;
/// usend never offers AUTH before TLS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    /// Always `usend`.
    pub username: String,
    /// The API key.
    pub password: String,
    /// `true` for port 465.
    pub implicit_tls: bool,
}

impl SmtpConfig {
    /// `host:port`
    pub fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Builds SMTP settings from an API key. `port` is 465 or 587 (`None` means 465). The host is
/// derived from `base_url` (`https://api.x.com` gives `smtp.x.com`; `None` gives
/// `smtp.usend.et`); the `USEND_SMTP_HOST` environment variable overrides it.
pub fn smtp_config(api_key: &str, base_url: Option<&str>, port: Option<u16>) -> Result<SmtpConfig> {
    let api_key = if api_key.is_empty() { std::env::var("USEND_API_KEY").unwrap_or_default() } else { api_key.to_string() };
    if api_key.is_empty() {
        return Err(Error::Config("missing API key for SMTP".into()));
    }
    let port = port.unwrap_or(465);
    if port != 465 && port != 587 {
        return Err(Error::Config("usend accepts SMTP on port 465 (TLS) or 587 (STARTTLS)".into()));
    }
    let host = match std::env::var("USEND_SMTP_HOST") {
        Ok(h) if !h.is_empty() => h,
        _ => smtp_host_for(base_url),
    };
    Ok(SmtpConfig { host, port, username: "usend".into(), password: api_key, implicit_tls: port == 465 })
}

impl Client {
    /// SMTP settings for this client's key and base URL. `port` is 465 or 587 (`None` means 465).
    pub fn smtp_config(&self, port: Option<u16>) -> Result<SmtpConfig> {
        smtp_config(&self.api_key, Some(&self.base_url), port)
    }
}

fn smtp_host_for(base_url: Option<&str>) -> String {
    let url = match base_url {
        Some(u) if !u.is_empty() => u,
        _ => return DEFAULT_SMTP_HOST.to_string(),
    };
    let rest = url.split_once("://").map(|(_, r)| r).unwrap_or(url);
    let authority = rest.split('/').next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = host.split(':').next().unwrap_or("");
    match host.strip_prefix("api.") {
        Some(rest) if !rest.is_empty() => format!("smtp.{rest}"),
        _ => DEFAULT_SMTP_HOST.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_host_and_validates_port() {
        std::env::remove_var("USEND_SMTP_HOST");
        let c = smtp_config("us_k", None, None).unwrap();
        assert_eq!((c.host.as_str(), c.port, c.implicit_tls, c.username.as_str()), ("smtp.usend.et", 465, true, "usend"));
        let c = smtp_config("us_k", Some("https://api.example.com/"), Some(587)).unwrap();
        assert_eq!((c.addr().as_str(), c.implicit_tls), ("smtp.example.com:587", false));
        assert_eq!(smtp_host_for(Some("http://localhost:8080")), "smtp.usend.et");
        assert!(smtp_config("us_k", None, Some(25)).is_err());
    }
}
