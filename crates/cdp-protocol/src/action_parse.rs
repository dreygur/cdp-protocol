//! Reading a [`BrowserAction`] out of the flat JSON an LLM tool call sends.

use serde::Deserialize;

use crate::action::BrowserAction;
use crate::error::{CdpError, Result};

/// How long `wait_for_selector` waits when the caller does not say.
const DEFAULT_SELECTOR_TIMEOUT_MS: u64 = 5000;

/// Every field any action accepts, flattened, because tool calls arrive as one
/// object with an `action` discriminator rather than as a tagged union.
#[derive(Debug, Deserialize)]
struct RawAction {
    action: String,
    url: Option<String>,
    selector: Option<String>,
    value: Option<String>,
    text: Option<String>,
    key: Option<String>,
    path: Option<String>,
    ms: Option<u64>,
    x: Option<f64>,
    y: Option<f64>,
    expression: Option<String>,
    width: Option<i32>,
    height: Option<i32>,
    mobile: Option<bool>,
    timeout_ms: Option<u64>,
}

/// Parse one tool call, e.g. `{"action": "navigate", "url": "..."}`.
///
/// Action names accept a short alias alongside the canonical name (`title` for
/// `get_title`), since models reach for both.
pub fn parse_action(json_str: &str) -> Result<BrowserAction> {
    let a: RawAction = serde_json::from_str(json_str)?;

    macro_rules! need {
        ($field:expr, $name:literal) => {
            $field.ok_or_else(|| CdpError::Protocol(concat!($name, " is required").into()))?
        };
    }

    Ok(match a.action.as_str() {
        "navigate" => BrowserAction::Navigate {
            url: need!(a.url, "url"),
        },
        "back" | "go_back" => BrowserAction::GoBack,
        "forward" | "go_forward" => BrowserAction::GoForward,
        "reload" => BrowserAction::Reload,
        "click" => BrowserAction::Click {
            selector: a.selector,
            x: a.x,
            y: a.y,
        },
        "type" => BrowserAction::Type {
            text: need!(a.text, "text"),
            selector: a.selector,
        },
        "fill" => BrowserAction::Fill {
            selector: need!(a.selector, "selector"),
            value: need!(a.value, "value"),
        },
        "submit" => BrowserAction::Submit {
            selector: a.selector,
        },
        "press_key" | "key" => BrowserAction::PressKey {
            key: need!(a.key, "key"),
        },
        "get_title" | "title" => BrowserAction::GetTitle,
        "get_url" | "url" => BrowserAction::GetUrl,
        "get_text" | "text" => BrowserAction::GetText,
        "get_content" | "content" => BrowserAction::GetContent {
            selector: a.selector,
        },
        "get_links" | "links" => BrowserAction::GetLinks,
        "get_attributes" | "attributes" => BrowserAction::GetAttributes {
            selector: need!(a.selector, "selector"),
        },
        "exists" => BrowserAction::Exists {
            selector: need!(a.selector, "selector"),
        },
        "screenshot" => BrowserAction::Screenshot { path: a.path },
        "evaluate" | "eval" => BrowserAction::Evaluate {
            expression: need!(a.expression, "expression"),
        },
        "wait" => BrowserAction::Wait {
            ms: need!(a.ms, "ms"),
        },
        "wait_for_selector" => BrowserAction::WaitForSelector {
            selector: need!(a.selector, "selector"),
            timeout_ms: a.timeout_ms.unwrap_or(DEFAULT_SELECTOR_TIMEOUT_MS),
        },
        "scroll" => BrowserAction::Scroll {
            x: a.x.unwrap_or(0.0),
            y: a.y.unwrap_or(0.0),
        },
        "set_viewport" => BrowserAction::SetViewport {
            width: need!(a.width, "width"),
            height: need!(a.height, "height"),
            mobile: a.mobile.unwrap_or(false),
        },
        "get_metrics" | "metrics" => BrowserAction::GetMetrics,
        other => return Err(CdpError::Protocol(format!("unknown action: {other}"))),
    })
}
