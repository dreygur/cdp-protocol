use std::fmt;

#[derive(Debug)]
pub enum CdpError {
    WebSocket(tokio_tungstenite::tungstenite::Error),
    Http(reqwest::Error),
    Json(serde_json::Error),
    Io(std::io::Error),
    InvalidUrl(String),
    Protocol(String),
    Timeout,
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
        CdpError::WebSocket(e)
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

pub type Result<T> = std::result::Result<T, CdpError>;
