//! Turning core `CdpError`s into the errors JS sees.

use cdp_driver::CdpError;
use napi::bindgen_prelude::Error;

/// Stable machine-readable code for a `CdpError`, surfaced to JS.
///
/// The thrown `Error.message` is `"[CODE] human message"`, so callers can
/// branch on kind, e.g. `if (err.message.startsWith('[TIMEOUT]'))`.
pub(crate) fn error_code(e: &CdpError) -> &'static str {
    match e {
        CdpError::WebSocket(_) => "WEBSOCKET",
        CdpError::Http(_) => "HTTP",
        CdpError::Json(_) => "JSON",
        CdpError::Io(_) => "IO",
        CdpError::InvalidUrl(_) => "INVALID_URL",
        // Both kinds stay "PROTOCOL" so JS callers already branching on that
        // prefix keep working; the numeric CDP code is in the message.
        CdpError::Browser { .. } => "PROTOCOL",
        CdpError::Protocol(_) => "PROTOCOL",
        CdpError::Timeout => "TIMEOUT",
        CdpError::NoTarget => "NO_TARGET",
    }
}

pub(crate) fn to_napi(e: CdpError) -> Error {
    Error::from_reason(format!("[{}] {e}", error_code(&e)))
}

pub(crate) fn json_err(e: serde_json::Error) -> Error {
    Error::from_reason(e.to_string())
}
