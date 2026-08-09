//! Evaluating JavaScript in the page.

use serde_json::{json, Value};

use crate::client::CdpClient;
use crate::error::{CdpError, Result};
use crate::types::EvaluateResult;

/// What to say about an expression that threw, given Chrome's `exceptionDetails`.
///
/// A thrown `Error` arrives as an object whose `description` is its message
/// followed by a stack trace, so only the first line is kept. Anything else JS
/// can throw (a string, a number) arrives as a plain `value` instead, and if
/// Chrome sends neither, its own summary `text` is all there is.
fn thrown_message(details: &Value) -> String {
    let exception = &details["exception"];
    if let Some(description) = exception["description"].as_str() {
        return description
            .lines()
            .next()
            .unwrap_or(description)
            .to_string();
    }
    match &exception["value"] {
        Value::Null => details["text"].as_str().unwrap_or("uncaught").to_string(),
        Value::String(thrown) => thrown.clone(),
        thrown => thrown.to_string(),
    }
}

impl CdpClient {
    /// Evaluate a JS expression and return its result stringified.
    ///
    /// An expression that throws is a [`CdpError::Protocol`] carrying the thrown
    /// message, since a returned string cannot tell an empty result apart from a
    /// failure. Use [`evaluate`](Self::evaluate) when you want the structured
    /// result, or an exception as data rather than as an error.
    pub async fn eval(&self, expression: &str) -> Result<String> {
        let outcome = self.evaluate(expression).await?;
        if let Some(details) = &outcome.exception_details {
            return Err(CdpError::Protocol(format!(
                "evaluation threw: {}",
                thrown_message(details)
            )));
        }
        Ok(outcome
            .result
            .value
            .map(|v| match v {
                Value::String(s) => s,
                other => other.to_string(),
            })
            .unwrap_or_default())
    }

    /// Evaluate a JS expression via `Runtime.evaluate` and return the full result,
    /// including any exception details.
    pub async fn evaluate(&self, expression: &str) -> Result<EvaluateResult> {
        let result = self
            .send_command(
                "Runtime.evaluate",
                json!({ "expression": expression, "returnByValue": true }),
            )
            .await?;
        Ok(serde_json::from_value(result)?)
    }
}
