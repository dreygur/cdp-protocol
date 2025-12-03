use thiserror::Error;

#[derive(Error, Debug)]
pub enum CdpError {
    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("CDP protocol error: {code} - {message}")]
    Protocol { code: i64, message: String },

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Timeout waiting for response")]
    Timeout,

    #[error("No targets available")]
    NoTargets,

    #[error("Target not found: {0}")]
    TargetNotFound(String),

    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    #[error("Channel error: {0}")]
    Channel(String),
}

pub type Result<T> = std::result::Result<T, CdpError>;
