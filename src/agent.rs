use crate::client::CdpClient;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::info;

/// High-level browser action for AI agents
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BrowserAction {
    /// Navigate to a URL
    Navigate { url: String },
    /// Click at element (by selector) or coordinates
    Click {
        #[serde(default)]
        selector: Option<String>,
        #[serde(default)]
        x: Option<f64>,
        #[serde(default)]
        y: Option<f64>,
    },
    /// Type text (optionally into a selector)
    Type {
        text: String,
        #[serde(default)]
        selector: Option<String>,
    },
    /// Press a key
    PressKey { key: String },
    /// Take a screenshot
    Screenshot {
        #[serde(default)]
        path: Option<String>,
    },
    /// Execute JavaScript
    Evaluate { expression: String },
    /// Get page content/HTML
    GetContent {
        #[serde(default)]
        selector: Option<String>,
    },
    /// Get page title
    GetTitle,
    /// Get current URL
    GetUrl,
    /// Wait for selector
    WaitForSelector {
        selector: String,
        #[serde(default = "default_timeout")]
        timeout_ms: u64,
    },
    /// Wait milliseconds
    Wait { ms: u64 },
    /// Scroll page
    Scroll {
        #[serde(default)]
        x: f64,
        #[serde(default)]
        y: f64,
    },
    /// Go back
    GoBack,
    /// Go forward
    GoForward,
    /// Reload page
    Reload,
    /// Set viewport size
    SetViewport {
        width: i32,
        height: i32,
        #[serde(default)]
        mobile: bool,
    },
    /// Get all text content
    GetText,
    /// Get all links on page
    GetLinks,
    /// Fill a form field
    Fill { selector: String, value: String },
    /// Submit a form
    Submit {
        #[serde(default)]
        selector: Option<String>,
    },
    /// Get element attributes
    GetAttributes { selector: String },
    /// Check if element exists
    Exists { selector: String },
    /// Get page metrics/performance
    GetMetrics,
}

fn default_timeout() -> u64 {
    5000
}

/// Result of a browser action
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ActionResult {
    Success {
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    Error {
        message: String,
    },
}

impl ActionResult {
    pub fn success(data: Option<Value>, message: Option<&str>) -> Self {
        Self::Success {
            data,
            message: message.map(String::from),
        }
    }

    pub fn error(message: &str) -> Self {
        Self::Error {
            message: message.to_string(),
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }
}

/// Browser agent - high-level interface for AI control
pub struct BrowserAgent {
    client: CdpClient,
}

impl BrowserAgent {
    /// Create agent connected to browser
    pub async fn connect(host: &str, port: u16) -> Result<Self> {
        let client = CdpClient::connect_to_page(host, port).await?;

        // Enable essential domains
        client.enable_domain("Page").await?;
        client.enable_domain("Runtime").await?;
        client.enable_domain("DOM").await?;
        client.enable_domain("Network").await?;

        info!("Browser agent connected");
        Ok(Self { client })
    }

    /// Create agent with existing client
    pub fn with_client(client: CdpClient) -> Self {
        Self { client }
    }

    /// Get underlying CDP client
    pub fn client(&self) -> &CdpClient {
        &self.client
    }

    /// Execute a browser action
    pub async fn execute(&self, action: BrowserAction) -> ActionResult {
        match self.execute_inner(action).await {
            Ok(result) => result,
            Err(e) => ActionResult::error(&e.to_string()),
        }
    }

    /// Execute multiple actions in sequence
    pub async fn execute_many(&self, actions: Vec<BrowserAction>) -> Vec<ActionResult> {
        let mut results = Vec::with_capacity(actions.len());
        for action in actions {
            let result = self.execute(action).await;
            let is_error = !result.is_success();
            results.push(result);
            if is_error {
                break; // Stop on first error
            }
        }
        results
    }

    /// Parse and execute action from JSON
    pub async fn execute_json(&self, json: &str) -> ActionResult {
        match serde_json::from_str::<BrowserAction>(json) {
            Ok(action) => self.execute(action).await,
            Err(e) => ActionResult::error(&format!("Invalid action JSON: {}", e)),
        }
    }

    async fn execute_inner(&self, action: BrowserAction) -> Result<ActionResult> {
        match action {
            BrowserAction::Navigate { url } => {
                let result = self.client.navigate(&url).await?;
                if let Some(err) = result.error_text {
                    Ok(ActionResult::error(&err))
                } else {
                    Ok(ActionResult::success(None, Some(&format!("Navigated to {}", url))))
                }
            }

            BrowserAction::Click { selector, x, y } => {
                if let Some(sel) = selector {
                    // Click by selector
                    let script = format!(
                        r#"(() => {{
                            const el = document.querySelector('{}');
                            if (!el) return null;
                            const rect = el.getBoundingClientRect();
                            return {{ x: rect.x + rect.width/2, y: rect.y + rect.height/2 }};
                        }})()"#,
                        sel
                    );
                    let result = self.client.evaluate(&script).await?;
                    if let Some(coords) = result.result.value {
                        let cx = coords["x"].as_f64().unwrap_or(0.0);
                        let cy = coords["y"].as_f64().unwrap_or(0.0);
                        self.client.click(cx, cy).await?;
                        Ok(ActionResult::success(None, Some(&format!("Clicked {}", sel))))
                    } else {
                        Ok(ActionResult::error(&format!("Selector not found: {}", sel)))
                    }
                } else if let (Some(x), Some(y)) = (x, y) {
                    self.client.click(x, y).await?;
                    Ok(ActionResult::success(None, Some(&format!("Clicked at ({}, {})", x, y))))
                } else {
                    Ok(ActionResult::error("Click requires selector or x,y coordinates"))
                }
            }

            BrowserAction::Type { text, selector } => {
                if let Some(sel) = selector {
                    // Focus element first
                    let script = format!(
                        "document.querySelector('{}')?.focus()",
                        sel
                    );
                    self.client.evaluate(&script).await?;
                }
                self.client.type_text(&text).await?;
                Ok(ActionResult::success(None, Some(&format!("Typed: {}", text))))
            }

            BrowserAction::PressKey { key } => {
                self.client.press_key(&key).await?;
                Ok(ActionResult::success(None, Some(&format!("Pressed: {}", key))))
            }

            BrowserAction::Screenshot { path } => {
                let data = self.client.screenshot().await?;
                if let Some(p) = path {
                    self.client.screenshot_to_file(&p).await?;
                    Ok(ActionResult::success(None, Some(&format!("Screenshot saved to {}", p))))
                } else {
                    Ok(ActionResult::success(
                        Some(serde_json::json!({ "base64": data })),
                        Some("Screenshot captured"),
                    ))
                }
            }

            BrowserAction::Evaluate { expression } => {
                let result = self.client.evaluate(&expression).await?;
                Ok(ActionResult::success(
                    result.result.value,
                    Some("Expression evaluated"),
                ))
            }

            BrowserAction::GetContent { selector } => {
                let content = if let Some(sel) = selector {
                    let script = format!(
                        "document.querySelector('{}')?.outerHTML || ''",
                        sel
                    );
                    self.client.eval::<String>(&script).await?
                } else {
                    self.client.eval::<String>("document.documentElement.outerHTML").await?
                };
                Ok(ActionResult::success(
                    Some(serde_json::json!({ "html": content })),
                    None,
                ))
            }

            BrowserAction::GetTitle => {
                let title: String = self.client.eval("document.title").await?;
                Ok(ActionResult::success(
                    Some(serde_json::json!({ "title": title })),
                    None,
                ))
            }

            BrowserAction::GetUrl => {
                let url: String = self.client.eval("window.location.href").await?;
                Ok(ActionResult::success(
                    Some(serde_json::json!({ "url": url })),
                    None,
                ))
            }

            BrowserAction::WaitForSelector { selector, timeout_ms } => {
                self.client.wait_for_selector(&selector, timeout_ms).await?;
                Ok(ActionResult::success(None, Some(&format!("Found: {}", selector))))
            }

            BrowserAction::Wait { ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                Ok(ActionResult::success(None, Some(&format!("Waited {}ms", ms))))
            }

            BrowserAction::Scroll { x, y } => {
                let script = format!("window.scrollBy({}, {})", x, y);
                self.client.evaluate(&script).await?;
                Ok(ActionResult::success(None, Some("Scrolled")))
            }

            BrowserAction::GoBack => {
                self.client.evaluate("history.back()").await?;
                Ok(ActionResult::success(None, Some("Navigated back")))
            }

            BrowserAction::GoForward => {
                self.client.evaluate("history.forward()").await?;
                Ok(ActionResult::success(None, Some("Navigated forward")))
            }

            BrowserAction::Reload => {
                self.client.reload(false).await?;
                Ok(ActionResult::success(None, Some("Page reloaded")))
            }

            BrowserAction::SetViewport { width, height, mobile } => {
                self.client.set_viewport(width, height, mobile).await?;
                Ok(ActionResult::success(None, Some(&format!("Viewport set to {}x{}", width, height))))
            }

            BrowserAction::GetText => {
                let text: String = self.client.eval("document.body.innerText").await?;
                Ok(ActionResult::success(
                    Some(serde_json::json!({ "text": text })),
                    None,
                ))
            }

            BrowserAction::GetLinks => {
                let links: Vec<Value> = self.client.eval(
                    r#"Array.from(document.querySelectorAll('a[href]')).map(a => ({
                        href: a.href,
                        text: a.innerText.trim()
                    }))"#
                ).await?;
                Ok(ActionResult::success(
                    Some(serde_json::json!({ "links": links })),
                    None,
                ))
            }

            BrowserAction::Fill { selector, value } => {
                let script = format!(
                    r#"(() => {{
                        const el = document.querySelector('{}');
                        if (!el) return false;
                        el.focus();
                        el.value = '{}';
                        el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                        return true;
                    }})()"#,
                    selector,
                    value.replace('\'', "\\'")
                );
                let success: bool = self.client.eval(&script).await?;
                if success {
                    Ok(ActionResult::success(None, Some(&format!("Filled {} with value", selector))))
                } else {
                    Ok(ActionResult::error(&format!("Element not found: {}", selector)))
                }
            }

            BrowserAction::Submit { selector } => {
                let script = if let Some(sel) = selector {
                    format!("document.querySelector('{}')?.submit()", sel)
                } else {
                    "document.querySelector('form')?.submit()".to_string()
                };
                self.client.evaluate(&script).await?;
                Ok(ActionResult::success(None, Some("Form submitted")))
            }

            BrowserAction::GetAttributes { selector } => {
                let script = format!(
                    r#"(() => {{
                        const el = document.querySelector('{}');
                        if (!el) return null;
                        const attrs = {{}};
                        for (const attr of el.attributes) {{
                            attrs[attr.name] = attr.value;
                        }}
                        return attrs;
                    }})()"#,
                    selector
                );
                let attrs = self.client.evaluate(&script).await?;
                Ok(ActionResult::success(attrs.result.value, None))
            }

            BrowserAction::Exists { selector } => {
                let script = format!(
                    "document.querySelector('{}') !== null",
                    selector
                );
                let exists: bool = self.client.eval(&script).await?;
                Ok(ActionResult::success(
                    Some(serde_json::json!({ "exists": exists })),
                    None,
                ))
            }

            BrowserAction::GetMetrics => {
                let result = self.client.send("Performance.getMetrics", None).await?;
                Ok(ActionResult::success(Some(result), None))
            }
        }
    }
}

/// Simple action builder for chaining
pub struct ActionBuilder {
    actions: Vec<BrowserAction>,
}

impl ActionBuilder {
    pub fn new() -> Self {
        Self { actions: vec![] }
    }

    pub fn navigate(mut self, url: &str) -> Self {
        self.actions.push(BrowserAction::Navigate { url: url.to_string() });
        self
    }

    pub fn click(mut self, selector: &str) -> Self {
        self.actions.push(BrowserAction::Click {
            selector: Some(selector.to_string()),
            x: None,
            y: None,
        });
        self
    }

    pub fn type_text(mut self, text: &str) -> Self {
        self.actions.push(BrowserAction::Type {
            text: text.to_string(),
            selector: None,
        });
        self
    }

    pub fn fill(mut self, selector: &str, value: &str) -> Self {
        self.actions.push(BrowserAction::Fill {
            selector: selector.to_string(),
            value: value.to_string(),
        });
        self
    }

    pub fn wait(mut self, ms: u64) -> Self {
        self.actions.push(BrowserAction::Wait { ms });
        self
    }

    pub fn wait_for(mut self, selector: &str) -> Self {
        self.actions.push(BrowserAction::WaitForSelector {
            selector: selector.to_string(),
            timeout_ms: 5000,
        });
        self
    }

    pub fn screenshot(mut self, path: Option<&str>) -> Self {
        self.actions.push(BrowserAction::Screenshot {
            path: path.map(String::from),
        });
        self
    }

    pub fn build(self) -> Vec<BrowserAction> {
        self.actions
    }
}

impl Default for ActionBuilder {
    fn default() -> Self {
        Self::new()
    }
}
