use std::fmt;
use std::time::Duration;

/// An error returned by the usend API, in the API's `{ statusCode, name, message }` shape.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiError {
    pub status_code: u16,
    /// For example `validation_error`, `rate_limit_exceeded`, `daily_quota_exceeded`.
    pub name: String,
    pub message: String,
    /// The delay the server asked for (`Retry-After`, in seconds), if any.
    pub retry_after: Option<Duration>,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "usend: {} (HTTP {}): {}", self.name, self.status_code, self.message)
    }
}

/// Everything that can go wrong in the SDK.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The API answered with a non-2xx status.
    Api(ApiError),
    /// The request could not be sent or the response could not be read.
    Transport(String),
    /// The response body was not the JSON the SDK expected.
    Decode(String),
    /// Missing or invalid configuration (for example no API key).
    Config(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Api(e) => e.fmt(f),
            Error::Transport(m) => write!(f, "usend: transport error: {m}"),
            Error::Decode(m) => write!(f, "usend: could not decode response: {m}"),
            Error::Config(m) => write!(f, "usend: {m}"),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    /// The API error, when this is one.
    pub fn as_api(&self) -> Option<&ApiError> {
        match self {
            Error::Api(e) => Some(e),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
