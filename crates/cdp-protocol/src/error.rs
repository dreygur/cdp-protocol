//! The crate's error type and [`Result`] alias.

use std::fmt;

/// Errors returned by every fallible operation in this crate.
#[derive(Debug)]
pub enum CdpError {
    /// The CDP WebSocket connection failed or dropped.
    WebSocket(Box<tokio_tungstenite::tungstenite::Error>),
    /// A request to Chrome's HTTP endpoint (`/json/*`) failed.
    Http(reqwest::Error),
    /// A CDP payload failed to (de)serialize.
    Json(serde_json::Error),
    /// A filesystem operation (e.g. writing a screenshot) failed.
    Io(std::io::Error),
    /// A URL supplied by the caller or returned by Chrome was malformed or missing.
    InvalidUrl(String),
    /// Chrome returned a CDP protocol-level error, or a response was shaped
    /// unexpectedly.
    Protocol(String),
    /// A command or event wait exceeded its configured timeout.
    Timeout,
    /// No page target was available to connect to (e.g. `list_targets` returned none).
    NoTarget,
}

impl fmt::Display for CdpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CdpError::WebSocket(e) => write!(f, "WebSocket error: {e}"),
            CdpError::Http(e) => write!(f, "HTTP error: {e}"),
            CdpError::Json(e) => write!(f, "JSON error: {e}"),
            CdpError::Io(e) => write!(f, "IO error: {e}"),
            CdpError::InvalidUrl(s) => write!(f, "Invalid URL: {s}"),
            CdpError::Protocol(s) => write!(f, "Protocol error: {s}"),
            CdpError::Timeout => write!(f, "Operation timed out"),
            CdpError::NoTarget => write!(f, "No page target available"),
        }
    }
}

impl std::error::Error for CdpError {}

impl From<tokio_tungstenite::tungstenite::Error> for CdpError {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        CdpError::WebSocket(Box::new(e))
    }
}

impl From<reqwest::Error> for CdpError {
    fn from(e: reqwest::Error) -> Self {
        CdpError::Http(e)
    }
}

impl From<serde_json::Error> for CdpError {
    fn from(e: serde_json::Error) -> Self {
        CdpError::Json(e)
    }
}

impl From<std::io::Error> for CdpError {
    fn from(e: std::io::Error) -> Self {
        CdpError::Io(e)
    }
}

/// Convenience alias for `std::result::Result<T, CdpError>`.
pub type Result<T> = std::result::Result<T, CdpError>;
