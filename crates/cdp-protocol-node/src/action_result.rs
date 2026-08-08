//! The outcome of one action, as JS receives it.

use napi_derive::napi;
use serde_json::Value;

/// Result of one browser action.
#[napi(object)]
pub struct ActionResult {
    pub success: bool,
    /// Action output as a JSON value (`null` when the action returns nothing).
    pub value: Option<Value>,
    pub error: Option<String>,
}

pub(crate) fn core_result(r: cdp_driver::ActionResult) -> ActionResult {
    ActionResult {
        success: r.success,
        value: r.value,
        error: r.error,
    }
}
