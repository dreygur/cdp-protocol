use crate::error::{CdpError, Result};
use crate::types::*;
use futures_util::{SinkExt, StreamExt};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, info};

type ResponseSender = oneshot::Sender<Result<Value>>;
type PendingRequests = Arc<Mutex<HashMap<u64, ResponseSender>>>;
type EventCallback = Arc<dyn Fn(CdpEvent) + Send + Sync>;

/// CDP Client for browser communication
pub struct CdpClient {
    ws_sender: mpsc::Sender<Message>,
    pending: PendingRequests,
    next_id: AtomicU64,
    event_handlers: Arc<Mutex<HashMap<String, Vec<EventCallback>>>>,
    _receiver_handle: tokio::task::JoinHandle<()>,
}

impl CdpClient {
    /// Connect to a Chrome instance at the given WebSocket URL
    pub async fn connect(ws_url: &str) -> Result<Self> {
        info!("Connecting to {}", ws_url);
        let (ws_stream, _) = connect_async(ws_url).await?;
        let (mut write, mut read) = ws_stream.split();

        let (tx, mut rx) = mpsc::channel::<Message>(100);
        let pending: PendingRequests = Arc::new(Mutex::new(HashMap::new()));
        let event_handlers: Arc<Mutex<HashMap<String, Vec<EventCallback>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let pending_clone = pending.clone();
        let handlers_clone = event_handlers.clone();

        // Spawn writer task
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = write.send(msg).await {
                    error!("WebSocket write error: {}", e);
                    break;
                }
            }
        });

        // Spawn reader task
        let receiver_handle = tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        Self::handle_message(&text, &pending_clone, &handlers_clone).await;
                    }
                    Ok(Message::Close(_)) => {
                        info!("WebSocket closed");
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket read error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(Self {
            ws_sender: tx,
            pending,
            next_id: AtomicU64::new(1),
            event_handlers,
            _receiver_handle: receiver_handle,
        })
    }

    /// Connect to the first available page target
    pub async fn connect_to_page(host: &str, port: u16) -> Result<Self> {
        let targets = Self::list_targets(host, port).await?;
        let page = targets
            .into_iter()
            .find(|t| t.target_type == "page")
            .ok_or(CdpError::NoTargets)?;

        let ws_url = page
            .web_socket_debugger_url
            .ok_or_else(|| CdpError::InvalidUrl("No WebSocket URL for target".into()))?;

        Self::connect(&ws_url).await
    }

    /// List available targets via HTTP endpoint
    pub async fn list_targets(host: &str, port: u16) -> Result<Vec<TargetInfo>> {
        let url = format!("http://{}:{}/json/list", host, port);
        let resp = reqwest::get(&url).await?;
        let targets: Vec<TargetInfo> = resp.json().await?;
        Ok(targets)
    }

    /// Get browser version info
    pub async fn get_version(host: &str, port: u16) -> Result<BrowserVersion> {
        let url = format!("http://{}:{}/json/version", host, port);
        let resp = reqwest::get(&url).await?;
        let version: BrowserVersion = resp.json().await?;
        Ok(version)
    }

    /// Create a new tab
    pub async fn create_tab(host: &str, port: u16, url: Option<&str>) -> Result<TargetInfo> {
        let endpoint = match url {
            Some(u) => format!("http://{}:{}/json/new?{}", host, port, u),
            None => format!("http://{}:{}/json/new", host, port),
        };
        let client = reqwest::Client::new();
        let resp = client.put(&endpoint).send().await?;
        let target: TargetInfo = resp.json().await?;
        Ok(target)
    }

    /// Send a CDP command and wait for response
    pub async fn send(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let cmd = CdpCommand {
            id,
            method: method.to_string(),
            params,
        };

        let json = serde_json::to_string(&cmd)?;
        debug!("Sending: {}", json);

        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        self.ws_sender
            .send(Message::Text(json))
            .await
            .map_err(|e| CdpError::Channel(e.to_string()))?;

        // Wait for response with timeout
        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(CdpError::Channel("Response channel closed".into())),
            Err(_) => Err(CdpError::Timeout),
        }
    }

    /// Register an event handler
    pub async fn on_event<F>(&self, method: &str, callback: F)
    where
        F: Fn(CdpEvent) + Send + Sync + 'static,
    {
        let mut handlers = self.event_handlers.lock().await;
        handlers
            .entry(method.to_string())
            .or_default()
            .push(Arc::new(callback));
    }

    async fn handle_message(
        text: &str,
        pending: &PendingRequests,
        handlers: &Arc<Mutex<HashMap<String, Vec<EventCallback>>>>,
    ) {
        debug!("Received: {}", text);

        // Try parsing as response first (has 'id' field)
        if let Ok(resp) = serde_json::from_str::<CdpResponse>(text) {
            if let Some(sender) = pending.lock().await.remove(&resp.id) {
                let result = if let Some(err) = resp.error {
                    Err(CdpError::Protocol {
                        code: err.code,
                        message: err.message,
                    })
                } else {
                    Ok(resp.result.unwrap_or(Value::Null))
                };
                let _ = sender.send(result);
            }
            return;
        }

        // Try parsing as event (no 'id' field)
        if let Ok(event) = serde_json::from_str::<CdpEvent>(text) {
            let handlers = handlers.lock().await;

            // Call specific handlers
            if let Some(cbs) = handlers.get(&event.method) {
                for cb in cbs {
                    cb(event.clone());
                }
            }

            // Call wildcard handlers
            if let Some(cbs) = handlers.get("*") {
                for cb in cbs {
                    cb(event.clone());
                }
            }
        }
    }

    // ============ Convenience methods ============

    /// Enable a domain
    pub async fn enable_domain(&self, domain: &str) -> Result<Value> {
        self.send(&format!("{}.enable", domain), None).await
    }

    /// Disable a domain
    pub async fn disable_domain(&self, domain: &str) -> Result<Value> {
        self.send(&format!("{}.disable", domain), None).await
    }
}

// ============ Domain-specific implementations ============

impl CdpClient {
    /// Navigate to URL
    pub async fn navigate(&self, url: &str) -> Result<NavigateResult> {
        let params = serde_json::json!({ "url": url });
        let result = self.send("Page.navigate", Some(params)).await?;
        Ok(serde_json::from_value(result)?)
    }

    /// Reload page
    pub async fn reload(&self, ignore_cache: bool) -> Result<()> {
        let params = serde_json::json!({ "ignoreCache": ignore_cache });
        self.send("Page.reload", Some(params)).await?;
        Ok(())
    }

    /// Capture screenshot (returns base64 PNG)
    pub async fn screenshot(&self) -> Result<String> {
        let params = serde_json::json!({
            "format": "png",
            "captureBeyondViewport": true
        });
        let result = self.send("Page.captureScreenshot", Some(params)).await?;
        let screenshot: ScreenshotResult = serde_json::from_value(result)?;
        Ok(screenshot.data)
    }

    /// Capture screenshot and save to file
    pub async fn screenshot_to_file(&self, path: &str) -> Result<()> {
        let data = self.screenshot().await?;
        let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &data)
            .map_err(|e| CdpError::Channel(e.to_string()))?;
        std::fs::write(path, bytes).map_err(|e| CdpError::Channel(e.to_string()))?;
        Ok(())
    }

    /// Print page to PDF (returns base64)
    pub async fn print_pdf(&self) -> Result<String> {
        let result = self.send("Page.printToPDF", None).await?;
        Ok(result["data"].as_str().unwrap_or_default().to_string())
    }

    /// Evaluate JavaScript expression
    pub async fn evaluate(&self, expression: &str) -> Result<EvaluateResult> {
        let params = serde_json::json!({
            "expression": expression,
            "returnByValue": true,
            "awaitPromise": true
        });
        let result = self.send("Runtime.evaluate", Some(params)).await?;
        Ok(serde_json::from_value(result)?)
    }

    /// Evaluate and return simple value
    pub async fn eval<T: serde::de::DeserializeOwned>(&self, expression: &str) -> Result<T> {
        let result = self.evaluate(expression).await?;
        if let Some(val) = result.result.value {
            Ok(serde_json::from_value(val)?)
        } else {
            Err(CdpError::Protocol {
                code: -1,
                message: "No value returned".into(),
            })
        }
    }

    /// Get document root
    pub async fn get_document(&self) -> Result<DomNode> {
        let params = serde_json::json!({ "depth": -1 });
        let result = self.send("DOM.getDocument", Some(params)).await?;
        let doc: DocumentResult = serde_json::from_value(result)?;
        Ok(doc.root)
    }

    /// Query selector
    pub async fn query_selector(&self, node_id: i64, selector: &str) -> Result<i64> {
        let params = serde_json::json!({
            "nodeId": node_id,
            "selector": selector
        });
        let result = self.send("DOM.querySelector", Some(params)).await?;
        Ok(result["nodeId"].as_i64().unwrap_or(0))
    }

    /// Get outer HTML of a node
    pub async fn get_outer_html(&self, node_id: i64) -> Result<String> {
        let params = serde_json::json!({ "nodeId": node_id });
        let result = self.send("DOM.getOuterHTML", Some(params)).await?;
        Ok(result["outerHTML"].as_str().unwrap_or_default().to_string())
    }

    /// Click at coordinates
    pub async fn click(&self, x: f64, y: f64) -> Result<()> {
        // Mouse down
        let params = serde_json::json!({
            "type": "mousePressed",
            "x": x,
            "y": y,
            "button": "left",
            "clickCount": 1
        });
        self.send("Input.dispatchMouseEvent", Some(params)).await?;

        // Mouse up
        let params = serde_json::json!({
            "type": "mouseReleased",
            "x": x,
            "y": y,
            "button": "left",
            "clickCount": 1
        });
        self.send("Input.dispatchMouseEvent", Some(params)).await?;
        Ok(())
    }

    /// Type text
    pub async fn type_text(&self, text: &str) -> Result<()> {
        let params = serde_json::json!({ "text": text });
        self.send("Input.insertText", Some(params)).await?;
        Ok(())
    }

    /// Press a key
    pub async fn press_key(&self, key: &str) -> Result<()> {
        // Key down
        let params = serde_json::json!({
            "type": "keyDown",
            "key": key
        });
        self.send("Input.dispatchKeyEvent", Some(params)).await?;

        // Key up
        let params = serde_json::json!({
            "type": "keyUp",
            "key": key
        });
        self.send("Input.dispatchKeyEvent", Some(params)).await?;
        Ok(())
    }

    /// Set viewport size
    pub async fn set_viewport(&self, width: i32, height: i32, mobile: bool) -> Result<()> {
        let params = serde_json::json!({
            "width": width,
            "height": height,
            "deviceScaleFactor": 1,
            "mobile": mobile
        });
        self.send("Emulation.setDeviceMetricsOverride", Some(params))
            .await?;
        Ok(())
    }

    /// Set user agent
    pub async fn set_user_agent(&self, user_agent: &str) -> Result<()> {
        let params = serde_json::json!({ "userAgent": user_agent });
        self.send("Emulation.setUserAgentOverride", Some(params))
            .await?;
        Ok(())
    }

    /// Get cookies
    pub async fn get_cookies(&self) -> Result<Vec<Value>> {
        let result = self.send("Network.getCookies", None).await?;
        Ok(result["cookies"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    /// Set a cookie
    pub async fn set_cookie(&self, name: &str, value: &str, url: &str) -> Result<bool> {
        let params = serde_json::json!({
            "name": name,
            "value": value,
            "url": url
        });
        let result = self.send("Network.setCookie", Some(params)).await?;
        Ok(result["success"].as_bool().unwrap_or(false))
    }

    /// Clear browser cookies
    pub async fn clear_cookies(&self) -> Result<()> {
        self.send("Network.clearBrowserCookies", None).await?;
        Ok(())
    }

    /// Get response body for a request
    pub async fn get_response_body(&self, request_id: &str) -> Result<String> {
        let params = serde_json::json!({ "requestId": request_id });
        let result = self.send("Network.getResponseBody", Some(params)).await?;
        Ok(result["body"].as_str().unwrap_or_default().to_string())
    }

    /// Wait for page load
    pub async fn wait_for_load(&self) -> Result<()> {
        self.evaluate("new Promise(r => { if (document.readyState === 'complete') r(); else window.addEventListener('load', r); })").await?;
        Ok(())
    }

    /// Wait for selector to appear
    pub async fn wait_for_selector(&self, selector: &str, timeout_ms: u64) -> Result<()> {
        let script = format!(
            r#"
            new Promise((resolve, reject) => {{
                const el = document.querySelector('{}');
                if (el) return resolve(true);
                const observer = new MutationObserver(() => {{
                    const el = document.querySelector('{}');
                    if (el) {{ observer.disconnect(); resolve(true); }}
                }});
                observer.observe(document.body, {{ childList: true, subtree: true }});
                setTimeout(() => {{ observer.disconnect(); reject(new Error('Timeout')); }}, {});
            }})
            "#,
            selector, selector, timeout_ms
        );
        self.evaluate(&script).await?;
        Ok(())
    }
}
