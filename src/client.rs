use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{broadcast, oneshot, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::error::{CdpError, Result};
use crate::types::*;

type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>;

pub struct CdpClient {
    tx: tokio::sync::mpsc::UnboundedSender<Message>,
    pending: PendingMap,
    next_id: Arc<AtomicU64>,
    events_tx: broadcast::Sender<(String, Value)>,
}

impl CdpClient {
    pub async fn connect(ws_url: &str) -> Result<Self> {
        let (ws_stream, _) = connect_async(ws_url).await?;
        let (mut sink, mut stream) = ws_stream.split();

        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let pending_clone = pending.clone();

        let (events_tx, _) = broadcast::channel::<(String, Value)>(256);
        let events_tx_clone = events_tx.clone();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if sink.send(msg).await.is_err() {
                    break;
                }
            }
        });

        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        let Ok(val) = serde_json::from_str::<Value>(&text) else {
                            continue;
                        };
                        let Some(id) = val.get("id").and_then(|v| v.as_u64()) else {
                            if let (Some(method), params) = (
                                val.get("method").and_then(|v| v.as_str()).map(str::to_owned),
                                val.get("params").cloned().unwrap_or(Value::Null),
                            ) {
                                let _ = events_tx_clone.send((method, params));
                            }
                            continue;
                        };
                        let outcome = if val.get("error").is_some() {
                            let msg = val["error"]["message"]
                                .as_str()
                                .unwrap_or("protocol error")
                                .to_string();
                            Err(CdpError::Protocol(msg))
                        } else {
                            Ok(val.get("result").cloned().unwrap_or(Value::Null))
                        };
                        let mut map = pending_clone.lock().await;
                        if let Some(tx) = map.remove(&id) {
                            let _ = tx.send(outcome);
                        }
                    }
                    Ok(Message::Close(_)) | Err(_) => {
                        let mut map = pending_clone.lock().await;
                        for (_, tx) in map.drain() {
                            let _ = tx.send(Err(CdpError::Protocol("connection closed".into())));
                        }
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(CdpClient { tx, pending, next_id: Arc::new(AtomicU64::new(1)), events_tx })
    }

    pub(crate) async fn send_command(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();

        self.pending.lock().await.insert(id, tx);

        self.tx
            .send(Message::Text(
                json!({ "id": id, "method": method, "params": params }).to_string(),
            ))
            .map_err(|e| CdpError::Protocol(e.to_string()))?;

        rx.await.map_err(|_| CdpError::Protocol("response channel closed".into()))?
    }

    pub fn subscribe_events(&self) -> broadcast::Receiver<(String, Value)> {
        self.events_tx.subscribe()
    }

    pub async fn wait_for_event(&self, method: &str, timeout_ms: u64) -> Result<Value> {
        let mut rx = self.events_tx.subscribe();
        tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            async move {
                loop {
                    match rx.recv().await {
                        Ok((m, params)) if m == method => return Ok(params),
                        Ok(_) => continue,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => return Err(CdpError::Protocol("event channel closed".into())),
                    }
                }
            },
        )
        .await
        .map_err(|_| CdpError::Timeout)?
    }

    pub async fn enable_domain(&self, domain: &str) -> Result<()> {
        self.send_command(&format!("{domain}.enable"), json!({})).await?;
        Ok(())
    }

    pub async fn navigate(&self, url: &str) -> Result<NavigationResult> {
        let result = self.send_command("Page.navigate", json!({ "url": url })).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn navigate_and_wait(&self, url: &str, timeout_ms: u64) -> Result<NavigationResult> {
        let mut rx = self.events_tx.subscribe();
        let nav = self.navigate(url).await?;
        tokio::time::timeout(
            std::time::Duration::from_millis(timeout_ms),
            async move {
                loop {
                    match rx.recv().await {
                        Ok((m, _)) if m == "Page.loadEventFired" => return Ok(nav),
                        Ok(_) => continue,
                        Err(broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => return Err(CdpError::Protocol("event channel closed".into())),
                    }
                }
            },
        )
        .await
        .map_err(|_| CdpError::Timeout)?
    }

    pub async fn eval(&self, expression: &str) -> Result<String> {
        let result = self.evaluate(expression).await?;
        Ok(result.result.value.map(|v| match v {
            Value::String(s) => s,
            other => other.to_string(),
        }).unwrap_or_default())
    }

    pub async fn evaluate(&self, expression: &str) -> Result<EvaluateResult> {
        let result = self
            .send_command("Runtime.evaluate", json!({ "expression": expression, "returnByValue": true }))
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_document(&self) -> Result<DocumentNode> {
        let result = self.send_command("DOM.getDocument", json!({ "depth": 0 })).await?;
        let root = result["root"].clone();
        if root.is_null() {
            return Err(CdpError::Protocol("DOM.getDocument returned no root".into()));
        }
        Ok(serde_json::from_value(root)?)
    }

    pub async fn query_selector(&self, node_id: i64, selector: &str) -> Result<i64> {
        let result = self
            .send_command("DOM.querySelector", json!({ "nodeId": node_id, "selector": selector }))
            .await?;
        Ok(result["nodeId"].as_i64().unwrap_or(0))
    }

    pub async fn get_outer_html(&self, node_id: i64) -> Result<String> {
        let result = self
            .send_command("DOM.getOuterHTML", json!({ "nodeId": node_id }))
            .await?;
        Ok(result["outerHTML"].as_str().unwrap_or("").to_string())
    }

    pub async fn close(&self) -> Result<()> {
        // Ignore errors, connection drops immediately after the tab closes
        let _ = self.send_command("Page.close", json!({})).await;
        Ok(())
    }

    pub async fn screenshot(&self) -> Result<Vec<u8>> {
        let result = self
            .send_command("Page.captureScreenshot", json!({ "format": "png", "fromSurface": true }))
            .await?;
        png_bytes_from(&result)
    }

    pub async fn screenshot_to_file(&self, path: &str) -> Result<()> {
        tokio::fs::write(path, self.screenshot().await?).await?;
        Ok(())
    }

    pub async fn full_page_screenshot(&self) -> Result<Vec<u8>> {
        let size = self.evaluate(
            "(() => ({ \
                w: Math.max(document.body.scrollWidth, document.documentElement.scrollWidth), \
                h: Math.max(document.body.scrollHeight, document.documentElement.scrollHeight) \
            }))()"
        ).await?;

        let dims = size.result.value.as_ref();
        let w = dims.and_then(|v| v["w"].as_i64()).unwrap_or(1920) as i32;
        let h = dims.and_then(|v| v["h"].as_i64()).unwrap_or(1200) as i32;
        self.set_viewport(w.max(1920), h.max(1200), false).await?;

        let result = self
            .send_command("Page.captureScreenshot", json!({
                "format": "png",
                "captureBeyondViewport": true,
                "fromSurface": true,
            }))
            .await?;
        png_bytes_from(&result)
    }

    pub async fn full_page_screenshot_to_file(&self, path: &str) -> Result<()> {
        tokio::fs::write(path, self.full_page_screenshot().await?).await?;
        Ok(())
    }

    pub async fn set_viewport(&self, width: i32, height: i32, mobile: bool) -> Result<()> {
        self.send_command(
            "Emulation.setDeviceMetricsOverride",
            json!({ "width": width, "height": height, "deviceScaleFactor": 1, "mobile": mobile }),
        ).await?;
        Ok(())
    }

    pub async fn get_cookies(&self) -> Result<Vec<Cookie>> {
        let result = self.send_command("Network.getCookies", json!({})).await?;
        Ok(serde_json::from_value(result["cookies"].clone()).unwrap_or_default())
    }

    pub async fn get_version(host: &str, port: u16) -> Result<BrowserVersion> {
        let url = format!("http://{host}:{port}/json/version");
        Ok(reqwest::get(&url).await?.json().await?)
    }

    pub async fn list_targets(host: &str, port: u16) -> Result<Vec<Target>> {
        let url = format!("http://{host}:{port}/json/list");
        Ok(reqwest::get(&url).await?.json().await?)
    }

    pub async fn connect_to_page(host: &str, port: u16) -> Result<Self> {
        let targets = Self::list_targets(host, port).await?;
        let page = targets
            .into_iter()
            .find(|t| t.target_type == "page")
            .ok_or(CdpError::NoTarget)?;
        let ws_url = page.web_socket_debugger_url
            .ok_or_else(|| CdpError::InvalidUrl("target has no debugger URL".into()))?;
        Self::connect(&ws_url).await
    }

    pub async fn create_tab(host: &str, port: u16, url: Option<&str>) -> Result<Target> {
        // Chrome requires PUT for /json/new (GET returns 405 in modern versions)
        let endpoint = match url {
            Some(u) => format!("http://{host}:{port}/json/new?{u}"),
            None => format!("http://{host}:{port}/json/new"),
        };
        Ok(reqwest::Client::new().put(&endpoint).send().await?.json().await?)
    }
}

fn png_bytes_from(result: &Value) -> Result<Vec<u8>> {
    let data = result["data"]
        .as_str()
        .ok_or_else(|| CdpError::Protocol("screenshot response has no data".into()))?;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| CdpError::Protocol(e.to_string()))
}
