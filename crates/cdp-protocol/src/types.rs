//! Plain data types returned by CDP commands and events.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Response body of `GET /json/version`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BrowserVersion {
    /// Browser name and version, e.g. `"HeadlessChrome/120.0.0.0"`.
    #[serde(rename = "Browser")]
    pub browser: String,
    /// The CDP protocol version Chrome speaks.
    #[serde(rename = "Protocol-Version")]
    pub protocol_version: String,
    /// WebSocket URL for the browser-level debugger, if exposed.
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_debugger_url: Option<String>,
}

/// A debuggable target (tab, worker, etc.) as reported by `GET /json/list`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Target {
    /// Chrome's opaque target id.
    pub id: String,
    /// Target kind, e.g. `"page"`. [`CdpClient::connect_to_page`](crate::client::CdpClient::connect_to_page)
    /// only connects to targets of this type.
    #[serde(rename = "type")]
    pub target_type: String,
    /// Current page/tab title.
    pub title: String,
    /// Current page/tab URL.
    pub url: String,
    /// WebSocket URL to open a CDP session against this target, if debuggable.
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_debugger_url: Option<String>,
}

/// Result of `Page.navigate`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NavigationResult {
    /// The frame that started navigating.
    #[serde(rename = "frameId")]
    pub frame_id: String,
    /// The loader driving this navigation, if one has been assigned yet.
    #[serde(rename = "loaderId")]
    pub loader_id: Option<String>,
}

/// A JS value as CDP's `Runtime` domain represents it.
#[derive(Debug, Clone, Deserialize)]
pub struct RemoteObject {
    /// JS `typeof` of the value, e.g. `"string"`, `"object"`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// The value itself, present when `returnByValue: true` was requested.
    pub value: Option<Value>,
    /// Human-readable description Chrome generates for the value.
    pub description: Option<String>,
}

/// Result of `Runtime.evaluate`.
#[derive(Debug, Clone, Deserialize)]
pub struct EvaluateResult {
    /// The evaluated expression's value.
    pub result: RemoteObject,
    /// Present when the expression threw; contains Chrome's exception details.
    #[serde(rename = "exceptionDetails")]
    pub exception_details: Option<Value>,
}

/// A DOM node as returned by `DOM.getDocument`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DocumentNode {
    /// CDP node id, used by other `DOM.*` commands.
    #[serde(rename = "nodeId")]
    pub node_id: i64,
    /// Node tag/name, e.g. `"HTML"` for the document element.
    #[serde(rename = "nodeName")]
    pub node_name: String,
    /// Number of direct children, if known.
    #[serde(rename = "childNodeCount")]
    pub child_node_count: Option<i64>,
}

/// A browser cookie as returned by `Network.getCookies`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Cookie {
    /// Cookie name.
    pub name: String,
    /// Cookie value.
    pub value: String,
    /// Domain the cookie applies to.
    pub domain: String,
    /// Path the cookie applies to.
    pub path: String,
}

/// A single metric from `Performance.getMetrics`.
#[derive(Debug, Clone, Deserialize)]
pub struct PerformanceMetric {
    /// Metric name, e.g. `"JSHeapUsedSize"`.
    pub name: String,
    /// Metric value.
    pub value: f64,
}

/// A `console.*` call captured via [`BrowserAgent::capture_console`](crate::agent::BrowserAgent::capture_console).
#[derive(Debug, Clone)]
pub struct ConsoleMessage {
    /// Console method used, e.g. `"log"`, `"warn"`, `"error"`.
    pub level: String,
    /// First stringified argument passed to the console call.
    pub text: String,
    /// Source file the call originated from, if available.
    pub url: Option<String>,
    /// Source line the call originated from, if available.
    pub line: Option<u64>,
}
