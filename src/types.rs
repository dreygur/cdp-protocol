use serde::{Deserialize, Serialize};
use serde_json::Value;

/// CDP command sent to browser
#[derive(Debug, Clone, Serialize)]
pub struct CdpCommand {
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

/// CDP response from browser
#[derive(Debug, Clone, Deserialize)]
pub struct CdpResponse {
    pub id: u64,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub error: Option<CdpError>,
}

/// CDP event from browser (no id)
#[derive(Debug, Clone, Deserialize)]
pub struct CdpEvent {
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// CDP error
#[derive(Debug, Clone, Deserialize)]
pub struct CdpError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<String>,
}

/// Incoming CDP message (either response or event)
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum CdpMessage {
    Response(CdpResponse),
    Event(CdpEvent),
}

impl CdpMessage {
    pub fn is_response(&self) -> bool {
        matches!(self, CdpMessage::Response(_))
    }

    pub fn is_event(&self) -> bool {
        matches!(self, CdpMessage::Event(_))
    }
}

/// Target info from /json/list
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    pub id: String,
    #[serde(rename = "type")]
    pub target_type: String,
    pub title: String,
    pub url: String,
    pub web_socket_debugger_url: Option<String>,
    #[serde(default)]
    pub dev_tools_frontend_url: Option<String>,
}

/// Browser version info from /json/version
#[derive(Debug, Clone, Deserialize)]
pub struct BrowserVersion {
    #[serde(rename = "Browser")]
    pub browser: String,
    #[serde(rename = "Protocol-Version")]
    pub protocol_version: String,
    #[serde(rename = "User-Agent")]
    pub user_agent: String,
    #[serde(rename = "V8-Version")]
    pub v8_version: String,
    #[serde(rename = "WebKit-Version")]
    pub webkit_version: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_debugger_url: String,
}

// ============ Domain-specific types ============

/// Page.navigate result
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NavigateResult {
    pub frame_id: String,
    #[serde(default)]
    pub loader_id: Option<String>,
    #[serde(default)]
    pub error_text: Option<String>,
}

/// Page.captureScreenshot result
#[derive(Debug, Clone, Deserialize)]
pub struct ScreenshotResult {
    pub data: String, // base64 encoded
}

/// Runtime.evaluate result
#[derive(Debug, Clone, Deserialize)]
pub struct EvaluateResult {
    pub result: RemoteObject,
    #[serde(default)]
    pub exception_details: Option<ExceptionDetails>,
}

/// Remote object from Runtime domain
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteObject {
    #[serde(rename = "type")]
    pub object_type: String,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub value: Option<Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub object_id: Option<String>,
}

/// Exception details
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionDetails {
    pub exception_id: i64,
    pub text: String,
    pub line_number: i64,
    pub column_number: i64,
}

/// DOM.getDocument result
#[derive(Debug, Clone, Deserialize)]
pub struct DocumentResult {
    pub root: DomNode,
}

/// DOM node
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DomNode {
    pub node_id: i64,
    pub backend_node_id: i64,
    pub node_type: i64,
    pub node_name: String,
    pub local_name: String,
    pub node_value: String,
    #[serde(default)]
    pub children: Option<Vec<DomNode>>,
    #[serde(default)]
    pub attributes: Option<Vec<String>>,
}

/// Network.Request
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkRequest {
    pub url: String,
    pub method: String,
    #[serde(default)]
    pub headers: Option<Value>,
    #[serde(default)]
    pub post_data: Option<String>,
}

/// Network.Response
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkResponse {
    pub url: String,
    pub status: i64,
    pub status_text: String,
    #[serde(default)]
    pub headers: Option<Value>,
    #[serde(default)]
    pub mime_type: Option<String>,
}
