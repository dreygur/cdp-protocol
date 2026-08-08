//! The vocabulary of browser operations.

use serde::{Deserialize, Serialize};

/// A single browser operation, dispatched by
/// [`BrowserAgent::execute`](crate::agent::BrowserAgent::execute).
///
/// Serializes to/from the same shape
/// [`BrowserAgent::execute_json`](crate::agent::BrowserAgent::execute_json) parses
/// (field names map to lower_snake_case action names, e.g. `Navigate { url }` <->
/// `{"action": "navigate", "url": "..."}`), so it doubles as an LLM tool-call schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BrowserAction {
    Navigate {
        url: String,
    },
    GoBack,
    GoForward,
    Reload,

    Click {
        selector: Option<String>,
        x: Option<f64>,
        y: Option<f64>,
    },
    Type {
        text: String,
        selector: Option<String>,
    },
    Fill {
        selector: String,
        value: String,
    },
    Submit {
        selector: Option<String>,
    },
    PressKey {
        key: String,
    },

    GetTitle,
    GetUrl,
    GetText,
    GetContent {
        selector: Option<String>,
    },
    GetLinks,
    GetAttributes {
        selector: String,
    },
    Exists {
        selector: String,
    },

    Screenshot {
        path: Option<String>,
    },
    Evaluate {
        expression: String,
    },

    Wait {
        ms: u64,
    },
    WaitForSelector {
        selector: String,
        timeout_ms: u64,
    },

    Scroll {
        x: f64,
        y: f64,
    },
    SetViewport {
        width: i32,
        height: i32,
        mobile: bool,
    },
    GetMetrics,
}
