//! HTTP transports. The SDK talks to the API through the [`Transport`] trait so you can plug in
//! any blocking HTTP client.
//!
//! * [`StdTransport`] is built in: plain `http://` over `std::net::TcpStream`. It has no TLS, so it
//!   is meant for local testing and tests against a mock server (`http://localhost:...`).
//! * `UreqTransport` (cargo feature `ureq`) and `ReqwestTransport` (feature `reqwest`) speak
//!   HTTPS. With either feature on, `Client` uses it by default.
//! * Implement [`Transport`] yourself to use any other client.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

/// Boxed error returned by transports.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// One HTTP request, fully built by the client.
#[derive(Debug, Clone)]
pub struct HttpRequest {
    /// Upper-case method: `GET`, `POST`, `PATCH`, `DELETE`.
    pub method: String,
    /// Absolute URL.
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    /// Applies to connecting and to each read/write.
    pub timeout: Duration,
}

/// One HTTP response.
#[derive(Debug, Clone, Default)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpResponse {
    /// Case-insensitive header lookup.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// Sends HTTP requests. Implementations must return `Ok` for every HTTP status, including 4xx and
/// 5xx (the client interprets them), and `Err` only when no response was received.
pub trait Transport: Send + Sync {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, BoxError>;
}

/// Plain-HTTP transport using only the standard library. **No TLS**: `https://` URLs are refused.
#[derive(Debug, Default, Clone, Copy)]
pub struct StdTransport;

impl Transport for StdTransport {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, BoxError> {
        let target = parse_http_url(&req.url)?;
        for (k, v) in &req.headers {
            if k.contains(['\r', '\n', ':']) || v.contains(['\r', '\n']) {
                return Err("invalid header".into());
            }
        }
        let addr = (target.host.as_str(), target.port)
            .to_socket_addrs()?
            .next()
            .ok_or("could not resolve host")?;
        let mut stream = TcpStream::connect_timeout(&addr, req.timeout)?;
        stream.set_read_timeout(Some(req.timeout))?;
        stream.set_write_timeout(Some(req.timeout))?;

        let host_header = if target.port == 80 { target.host.clone() } else { format!("{}:{}", target.host, target.port) };
        let mut head = format!("{} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n", req.method, target.path, host_header);
        for (k, v) in &req.headers {
            head.push_str(&format!("{k}: {v}\r\n"));
        }
        if let Some(b) = &req.body {
            head.push_str(&format!("Content-Length: {}\r\n", b.len()));
        }
        head.push_str("\r\n");
        stream.write_all(head.as_bytes())?;
        if let Some(b) = &req.body {
            stream.write_all(b)?;
        }
        stream.flush()?;

        let mut raw = Vec::new();
        stream.read_to_end(&mut raw)?;
        parse_response(&raw)
    }
}

struct Target {
    host: String,
    port: u16,
    path: String,
}

fn parse_http_url(url: &str) -> Result<Target, BoxError> {
    if url.starts_with("https://") {
        return Err("the built-in transport speaks plain http:// only; enable the `ureq` or `reqwest` cargo feature, or pass your own Transport, to use https".into());
    }
    let rest = url.strip_prefix("http://").ok_or("URL must start with http://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], rest[i..].to_string()),
        None => (rest, "/".to_string()),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse::<u16>().map_err(|_| "invalid port")?),
        None => (authority.to_string(), 80),
    };
    if host.is_empty() {
        return Err("missing host".into());
    }
    Ok(Target { host, port, path })
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn parse_response(raw: &[u8]) -> Result<HttpResponse, BoxError> {
    let split = find(raw, b"\r\n\r\n").ok_or("malformed HTTP response")?;
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let body = &raw[split + 4..];
    let mut lines = head.split("\r\n");
    let status_line = lines.next().ok_or("empty response")?;
    let status: u16 = status_line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).ok_or("malformed status line")?;
    let mut headers = Vec::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let mut resp = HttpResponse { status, headers, body: Vec::new() };
    let chunked = resp.header("transfer-encoding").map(|v| v.to_ascii_lowercase().contains("chunked")).unwrap_or(false);
    resp.body = if chunked {
        dechunk(body)?
    } else if let Some(n) = resp.header("content-length").and_then(|v| v.parse::<usize>().ok()) {
        body[..n.min(body.len())].to_vec()
    } else {
        body.to_vec()
    };
    Ok(resp)
}

fn dechunk(mut data: &[u8]) -> Result<Vec<u8>, BoxError> {
    let mut out = Vec::new();
    loop {
        let eol = find(data, b"\r\n").ok_or("malformed chunked body")?;
        let size_str = String::from_utf8_lossy(&data[..eol]).into_owned();
        let size_str = size_str.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_str, 16).map_err(|_| "malformed chunk size")?;
        data = &data[eol + 2..];
        if size == 0 {
            return Ok(out);
        }
        if data.len() < size {
            return Err("truncated chunk".into());
        }
        out.extend_from_slice(&data[..size]);
        data = &data[size..];
        data = data.strip_prefix(b"\r\n").unwrap_or(data);
    }
}

/// HTTPS transport built on `ureq` 2 (cargo feature `ureq`).
#[cfg(feature = "ureq")]
pub struct UreqTransport {
    agent: ::ureq::Agent,
}

#[cfg(feature = "ureq")]
impl UreqTransport {
    pub fn new() -> Self {
        UreqTransport { agent: ::ureq::AgentBuilder::new().build() }
    }
}

#[cfg(feature = "ureq")]
impl Default for UreqTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "ureq")]
impl Transport for UreqTransport {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, BoxError> {
        let mut r = self.agent.request(&req.method, &req.url).timeout(req.timeout);
        for (k, v) in &req.headers {
            r = r.set(k, v);
        }
        let result = match &req.body {
            Some(b) => r.send_bytes(b),
            None => r.call(),
        };
        let resp = match result {
            Ok(resp) => resp,
            Err(::ureq::Error::Status(_, resp)) => resp,
            Err(e) => return Err(e.to_string().into()),
        };
        let status = resp.status();
        let headers: Vec<(String, String)> = resp
            .headers_names()
            .into_iter()
            .filter_map(|n| resp.header(&n).map(|v| (n.clone(), v.to_string())))
            .collect();
        let mut body = Vec::new();
        resp.into_reader().read_to_end(&mut body)?;
        Ok(HttpResponse { status, headers, body })
    }
}

/// HTTPS transport built on `reqwest::blocking` (cargo feature `reqwest`).
#[cfg(feature = "reqwest")]
pub struct ReqwestTransport {
    client: ::reqwest::blocking::Client,
}

#[cfg(feature = "reqwest")]
impl ReqwestTransport {
    pub fn new() -> Self {
        ReqwestTransport { client: ::reqwest::blocking::Client::new() }
    }

    /// Use an existing client (proxies, custom roots, ...).
    pub fn with_client(client: ::reqwest::blocking::Client) -> Self {
        ReqwestTransport { client }
    }
}

#[cfg(feature = "reqwest")]
impl Default for ReqwestTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "reqwest")]
impl Transport for ReqwestTransport {
    fn send(&self, req: &HttpRequest) -> Result<HttpResponse, BoxError> {
        let method = ::reqwest::Method::from_bytes(req.method.as_bytes())?;
        let mut r = self.client.request(method, &req.url).timeout(req.timeout);
        for (k, v) in &req.headers {
            r = r.header(k.as_str(), v.as_str());
        }
        if let Some(b) = &req.body {
            r = r.body(b.clone());
        }
        let resp = r.send()?;
        let status = resp.status().as_u16();
        let headers: Vec<(String, String)> = resp
            .headers()
            .iter()
            .filter_map(|(k, v)| v.to_str().ok().map(|v| (k.as_str().to_string(), v.to_string())))
            .collect();
        let body = resp.bytes()?.to_vec();
        Ok(HttpResponse { status, headers, body })
    }
}

#[cfg(feature = "ureq")]
pub(crate) fn default_transport() -> Arc<dyn Transport> {
    Arc::new(UreqTransport::new())
}

#[cfg(all(feature = "reqwest", not(feature = "ureq")))]
pub(crate) fn default_transport() -> Arc<dyn Transport> {
    Arc::new(ReqwestTransport::new())
}

#[cfg(not(any(feature = "ureq", feature = "reqwest")))]
pub(crate) fn default_transport() -> Arc<dyn Transport> {
    Arc::new(StdTransport)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_urls() {
        let t = parse_http_url("http://localhost:8080/emails?x=1").unwrap();
        assert_eq!((t.host.as_str(), t.port, t.path.as_str()), ("localhost", 8080, "/emails?x=1"));
        let t = parse_http_url("http://example.com").unwrap();
        assert_eq!((t.port, t.path.as_str()), (80, "/"));
        assert!(parse_http_url("https://api.usend.et").is_err());
    }

    #[test]
    fn parses_content_length_and_chunked() {
        let r = parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello-ignored").unwrap();
        assert_eq!((r.status, r.body.as_slice()), (200, &b"hello"[..]));
        let r = parse_response(b"HTTP/1.1 429 Too Many\r\nRetry-After: 2\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n").unwrap();
        assert_eq!(r.status, 429);
        assert_eq!(r.header("retry-after"), Some("2"));
        assert_eq!(r.body, b"abcde");
    }
}
