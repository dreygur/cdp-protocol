use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BrowserVersion {
    #[serde(rename = "Browser")]
    pub browser: String,
    #[serde(rename = "Protocol-Version")]
    pub protocol_version: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_debugger_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Target {
    pub id: String,
    #[serde(rename = "type")]
    pub target_type: String,
    pub title: String,
    pub url: String,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_debugger_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NavigationResult {
    #[serde(rename = "frameId")]
    pub frame_id: String,
    #[serde(rename = "loaderId")]
    pub loader_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteObject {
    #[serde(rename = "type")]
    pub object_type: String,
    pub value: Option<Value>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EvaluateResult {
    pub result: RemoteObject,
    #[serde(rename = "exceptionDetails")]
    pub exception_details: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DocumentNode {
    #[serde(rename = "nodeId")]
    pub node_id: i64,
    #[serde(rename = "nodeName")]
    pub node_name: String,
    #[serde(rename = "childNodeCount")]
    pub child_node_count: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PerformanceMetric {
    pub name: String,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct ConsoleMessage {
    pub level: String,
    pub text: String,
    pub url: Option<String>,
    pub line: Option<u64>,
}
