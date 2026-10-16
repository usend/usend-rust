use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::{ApiError, Error, Result};
use crate::transport::{default_transport, HttpRequest, HttpResponse, Transport};

/// SDK version, sent in the `User-Agent` header.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// The hosted usend API.
pub const DEFAULT_BASE_URL: &str = "https://api.usend.et";

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_MAX_RETRIES: u32 = 2;
const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);

/// A blocking usend API client. Cheap to clone and safe to share between threads.
#[derive(Clone)]
pub struct Client {
    pub(crate) api_key: String,
    pub(crate) base_url: String,
    transport: Arc<dyn Transport>,
    max_retries: u32,
    timeout: Duration,
    user_agent: String,
}

impl fmt::Debug for Client {
    // Never print the API key.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client").field("base_url", &self.base_url).finish_non_exhaustive()
    }
}

/// Builds a [`Client`].
pub struct ClientBuilder {
    api_key: Option<String>,
    base_url: Option<String>,
    transport: Option<Arc<dyn Transport>>,
    max_retries: u32,
    timeout: Duration,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        ClientBuilder { api_key: None, base_url: None, transport: None, max_retries: DEFAULT_MAX_RETRIES, timeout: DEFAULT_TIMEOUT }
    }
}

impl ClientBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// The API key. Defaults to the `USEND_API_KEY` environment variable.
    pub fn api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// The API base URL. Defaults to `USEND_BASE_URL`, then `https://api.usend.et`.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// The HTTP transport. Defaults to `UreqTransport` or `ReqwestTransport` when one of those
    /// cargo features is on, else the plain-HTTP `StdTransport`.
    pub fn transport(mut self, transport: impl Transport + 'static) -> Self {
        self.transport = Some(Arc::new(transport));
        self
    }

    /// Retries on `429` and `5xx` (default 2, `0` disables). A `5xx` is only retried for requests
    /// that cannot cause a duplicate: `GET`, `DELETE`, and anything with an idempotency key.
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    /// Per-attempt timeout (default 30 seconds).
    pub fn timeout(mut self, d: Duration) -> Self {
        self.timeout = d;
        self
    }

    pub fn build(self) -> Result<Client> {
        let api_key = match self.api_key {
            Some(k) if !k.is_empty() => k,
            _ => std::env::var("USEND_API_KEY").unwrap_or_default(),
        };
        if api_key.is_empty() {
            return Err(Error::Config("missing API key; pass it to Client::new(\"us_...\") or set USEND_API_KEY".into()));
        }
        let base_url = self
            .base_url
            .or_else(|| std::env::var("USEND_BASE_URL").ok().filter(|v| !v.is_empty()))
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        Ok(Client {
            api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            transport: self.transport.unwrap_or_else(default_transport),
            max_retries: self.max_retries,
            timeout: self.timeout,
            user_agent: format!("usend-rust/{VERSION}"),
        })
    }
}

/// Extra settings for a single request.
#[derive(Debug, Clone, Default)]
pub struct RequestOptions {
    pub(crate) headers: Vec<(String, String)>,
}

impl RequestOptions {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets `Idempotency-Key`: the API replays the first response for the same key for 24 hours.
    pub fn idempotency_key(self, key: impl Into<String>) -> Self {
        self.header("Idempotency-Key", key)
    }

    /// Sets `x-batch-validation` (`"strict"` or `"permissive"`) on a batch send.
    pub fn batch_validation(self, mode: impl Into<String>) -> Self {
        self.header("x-batch-validation", mode)
    }

    /// Sets any request header.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

impl Client {
    /// Creates a client for the hosted API. An empty key falls back to `USEND_API_KEY`.
    pub fn new(api_key: impl Into<String>) -> Result<Client> {
        ClientBuilder::new().api_key(api_key).build()
    }

    /// Creates a client from `USEND_API_KEY` (and `USEND_BASE_URL`).
    pub fn from_env() -> Result<Client> {
        ClientBuilder::new().build()
    }

    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// The API base URL in use.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Escape hatch for endpoints without a typed method. `body` is sent as JSON when present; the
    /// JSON response is returned as-is.
    ///
    /// ```no_run
    /// # let client = usend::Client::new("us_...").unwrap();
    /// let logs = client.request("GET", "/logs?days=7", None, &usend::RequestOptions::new()).unwrap();
    /// ```
    pub fn request(&self, method: &str, path: &str, body: Option<&serde_json::Value>, opts: &RequestOptions) -> Result<serde_json::Value> {
        let bytes = match body {
            Some(v) => Some(serde_json::to_vec(v).map_err(|e| Error::Decode(e.to_string()))?),
            None => None,
        };
        let raw = self.execute(method, path, bytes, opts)?;
        parse_json(&raw)
    }

    /// Sends a request and decodes the response into `T`. `null` members are dropped first, so a
    /// field the API sends as `null` falls back to its `#[serde(default)]`.
    pub(crate) fn call<T: DeserializeOwned>(&self, method: &str, path: &str, body: Option<Vec<u8>>, opts: &RequestOptions) -> Result<T> {
        let raw = self.execute(method, path, body, opts)?;
        let mut value = parse_json(&raw)?;
        strip_nulls(&mut value);
        serde_json::from_value(value).map_err(|e| Error::Decode(e.to_string()))
    }

    pub(crate) fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.call("GET", path, None, &RequestOptions::default())
    }

    /// Like `delete`, for endpoints whose body (usually `{}`) is not needed.
    pub(crate) fn delete_ok(&self, path: &str) -> Result<()> {
        self.execute("DELETE", path, None, &RequestOptions::default()).map(|_| ())
    }

    pub(crate) fn post_empty_ok(&self, path: &str) -> Result<()> {
        self.execute("POST", path, None, &RequestOptions::default()).map(|_| ())
    }

    pub(crate) fn delete<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.call("DELETE", path, None, &RequestOptions::default())
    }

    pub(crate) fn post_empty<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        self.call("POST", path, None, &RequestOptions::default())
    }

    pub(crate) fn post<T: DeserializeOwned, B: Serialize + ?Sized>(&self, path: &str, body: &B) -> Result<T> {
        self.send_json("POST", path, body, &RequestOptions::default())
    }

    pub(crate) fn patch<T: DeserializeOwned, B: Serialize + ?Sized>(&self, path: &str, body: &B) -> Result<T> {
        self.send_json("PATCH", path, body, &RequestOptions::default())
    }

    pub(crate) fn send_json<T: DeserializeOwned, B: Serialize + ?Sized>(&self, method: &str, path: &str, body: &B, opts: &RequestOptions) -> Result<T> {
        let bytes = serde_json::to_vec(body).map_err(|e| Error::Decode(e.to_string()))?;
        self.call(method, path, Some(bytes), opts)
    }

    fn execute(&self, method: &str, path: &str, body: Option<Vec<u8>>, opts: &RequestOptions) -> Result<Vec<u8>> {
        let mut headers = vec![
            ("Authorization".to_string(), format!("Bearer {}", self.api_key)),
            ("User-Agent".to_string(), self.user_agent.clone()),
            ("Accept".to_string(), "application/json".to_string()),
        ];
        if body.is_some() {
            headers.push(("Content-Type".to_string(), "application/json".to_string()));
        }
        headers.extend(opts.headers.iter().cloned());
        let has_key = opts.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("idempotency-key"));
        let safe_retry = matches!(method, "GET" | "DELETE" | "PUT") || has_key;
        let url = format!("{}{}", self.base_url, path);

        let mut attempt = 0u32;
        loop {
            let req = HttpRequest { method: method.to_string(), url: url.clone(), headers: headers.clone(), body: body.clone(), timeout: self.timeout };
            let (retry, retry_after, err) = match self.transport.send(&req) {
                Ok(resp) if (200..300).contains(&resp.status) => return Ok(resp.body),
                Ok(resp) => {
                    let api = parse_error(&resp);
                    let retry = resp.status == 429 || (resp.status >= 500 && safe_retry);
                    (retry, api.retry_after, Error::Api(api))
                }
                Err(e) => (safe_retry, None, Error::Transport(e.to_string())),
            };
            if !retry || attempt >= self.max_retries {
                return Err(err);
            }
            let mut delay = backoff(attempt);
            if let Some(ra) = retry_after {
                if ra > MAX_RETRY_DELAY {
                    return Err(err); // the server wants us to wait too long
                }
                delay = ra;
            }
            std::thread::sleep(delay);
            attempt += 1;
        }
    }
}

fn parse_json(raw: &[u8]) -> Result<serde_json::Value> {
    if raw.iter().all(|b| b.is_ascii_whitespace()) {
        return Ok(serde_json::Value::Null);
    }
    serde_json::from_slice(raw).map_err(|e| Error::Decode(e.to_string()))
}

/// Removes object members whose value is `null` (recursively).
fn strip_nulls(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(map) => {
            map.retain(|_, x| !x.is_null());
            for x in map.values_mut() {
                strip_nulls(x);
            }
        }
        serde_json::Value::Array(items) => {
            for x in items {
                strip_nulls(x);
            }
        }
        _ => {}
    }
}

fn parse_error(resp: &HttpResponse) -> ApiError {
    let retry_after = resp
        .header("retry-after")
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|s| *s >= 0.0 && *s < 86_400.0) // also rejects NaN and values that would overflow
        .map(Duration::from_secs_f64);
    let parsed: Option<serde_json::Value> = serde_json::from_slice(&resp.body).ok();
    let name = parsed.as_ref().and_then(|v| v.get("name")).and_then(|v| v.as_str());
    match name {
        Some(name) => ApiError {
            status_code: resp.status,
            name: name.to_string(),
            message: parsed.as_ref().and_then(|v| v.get("message")).and_then(|v| v.as_str()).unwrap_or("").to_string(),
            retry_after,
        },
        None => ApiError { status_code: resp.status, name: "application_error".into(), message: format!("HTTP {}", resp.status), retry_after },
    }
}

fn backoff(attempt: u32) -> Duration {
    Duration::from_millis(500u64 << attempt.min(4)).min(Duration::from_secs(8))
}

/// Percent-encodes a path segment or query value.
pub(crate) fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Appends `?k=v&...` for the given pairs (empty values are skipped).
pub(crate) fn with_query(path: &str, pairs: Vec<(&str, String)>) -> String {
    let q: Vec<String> = pairs.into_iter().filter(|(_, v)| !v.is_empty()).map(|(k, v)| format!("{}={}", k, enc(&v))).collect();
    if q.is_empty() {
        path.to_string()
    } else {
        format!("{}?{}", path, q.join("&"))
    }
}
