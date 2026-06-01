use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::client::CdpClient;
use crate::config::Config;
use crate::error::{CdpError, Result};

fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s.replace('"', "\\\"")))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BrowserAction {
    Navigate { url: String },
    GoBack,
    GoForward,
    Reload,

    Click { selector: Option<String>, x: Option<f64>, y: Option<f64> },
    Type { text: String, selector: Option<String> },
    Fill { selector: String, value: String },
    Submit { selector: Option<String> },
    PressKey { key: String },

    GetTitle,
    GetUrl,
    GetText,
    GetContent { selector: Option<String> },
    GetLinks,
    GetAttributes { selector: String },
    Exists { selector: String },

    Screenshot { path: Option<String> },
    Evaluate { expression: String },

    Wait { ms: u64 },
    WaitForSelector { selector: String, timeout_ms: u64 },

    Scroll { x: f64, y: f64 },
    SetViewport { width: i32, height: i32, mobile: bool },
    GetMetrics,
}

#[derive(Debug, Clone)]
pub struct ActionResult {
    pub success: bool,
    pub value: Option<Value>,
    pub error: Option<String>,
}

impl ActionResult {
    pub fn is_success(&self) -> bool {
        self.success
    }
}

impl std::fmt::Display for ActionResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.success {
            write!(f, "Ok({:?})", self.value)
        } else {
            write!(f, "Err({})", self.error.as_deref().unwrap_or("unknown"))
        }
    }
}

pub struct BrowserAgent {
    client: CdpClient,
}

impl BrowserAgent {
    pub async fn connect(host: &str, port: u16) -> Result<Self> {
        let client = CdpClient::connect_to_page(host, port).await?;
        for domain in ["Page", "Runtime", "DOM", "Network"] {
            client.enable_domain(domain).await?;
        }
        Ok(BrowserAgent { client })
    }

    pub async fn connect_with_config(config: &Config) -> Result<Self> {
        let agent = Self::connect(&config.host, config.port).await?;
        agent.client.set_viewport(config.viewport_width, config.viewport_height, false).await?;
        Ok(agent)
    }

    pub async fn execute(&self, action: BrowserAction) -> ActionResult {
        match self.dispatch(action).await {
            Ok(value) => ActionResult { success: true, value: Some(value), error: None },
            Err(e) => ActionResult { success: false, value: None, error: Some(e.to_string()) },
        }
    }

    pub async fn execute_many(&self, actions: Vec<BrowserAction>) -> Vec<ActionResult> {
        let mut results = Vec::with_capacity(actions.len());
        for action in actions {
            results.push(self.execute(action).await);
        }
        results
    }

    pub async fn execute_json(&self, json_str: &str) -> ActionResult {
        match parse_action(json_str) {
            Ok(action) => self.execute(action).await,
            Err(e) => ActionResult { success: false, value: None, error: Some(e.to_string()) },
        }
    }

    async fn dispatch(&self, action: BrowserAction) -> Result<Value> {
        match action {
            BrowserAction::Navigate { url } => {
                let nav = self.client.navigate(&url).await?;
                Ok(json!({ "frameId": nav.frame_id }))
            }
            BrowserAction::GoBack => self.client.send_command("Page.goBack", json!({})).await,
            BrowserAction::GoForward => self.client.send_command("Page.goForward", json!({})).await,
            BrowserAction::Reload => self.client.send_command("Page.reload", json!({})).await,

            BrowserAction::Click { selector, x, y } => {
                if let Some(sel) = selector {
                    self.client.eval(&format!(
                        "document.querySelector({})?.click()",
                        quote(&sel)
                    )).await?;
                } else if let (Some(cx), Some(cy)) = (x, y) {
                    for event_type in ["mousePressed", "mouseReleased"] {
                        self.client.send_command("Input.dispatchMouseEvent", json!({
                            "type": event_type, "x": cx, "y": cy,
                            "button": "left", "clickCount": 1,
                        })).await?;
                    }
                } else {
                    return Err(CdpError::Protocol("click: need selector or (x, y)".into()));
                }
                Ok(json!(null))
            }

            BrowserAction::Type { text, selector } => {
                if let Some(sel) = selector {
                    self.client.eval(&format!(
                        "document.querySelector({})?.focus()",
                        quote(&sel)
                    )).await?;
                }
                self.client.send_command("Input.insertText", json!({ "text": text })).await?;
                Ok(json!(null))
            }

            BrowserAction::Fill { selector, value } => {
                self.client.eval(&format!(
                    "(sel => {{ \
                        let el = document.querySelector(sel); \
                        if (!el) return; \
                        el.focus(); \
                        el.value = {}; \
                        el.dispatchEvent(new Event('input', {{bubbles:true}})); \
                        el.dispatchEvent(new Event('change', {{bubbles:true}})); \
                    }})({})",
                    quote(&value),
                    quote(&selector),
                )).await?;
                Ok(json!(null))
            }

            BrowserAction::Submit { selector } => {
                let sel = selector.as_deref().unwrap_or("form");
                self.client.eval(&format!(
                    "document.querySelector({})?.submit()",
                    quote(sel)
                )).await?;
                Ok(json!(null))
            }

            BrowserAction::PressKey { key } => {
                let (code, vk) = key_info(&key);
                self.client.send_command("Input.dispatchKeyEvent", json!({
                    "type": "keyDown", "key": key, "code": code,
                    "windowsVirtualKeyCode": vk,
                })).await?;
                self.client.send_command("Input.dispatchKeyEvent", json!({
                    "type": "keyUp", "key": key, "code": code,
                })).await?;
                Ok(json!(null))
            }

            BrowserAction::GetTitle => Ok(json!(self.client.eval("document.title").await?)),
            BrowserAction::GetUrl => Ok(json!(self.client.eval("window.location.href").await?)),
            BrowserAction::GetText => Ok(json!(self.client.eval("document.body.innerText").await?)),

            BrowserAction::GetContent { selector } => {
                let expr = match selector {
                    Some(sel) => format!(
                        "document.querySelector({})?.innerHTML",
                        quote(&sel)
                    ),
                    None => "document.documentElement.outerHTML".into(),
                };
                Ok(json!(self.client.eval(&expr).await?))
            }

            BrowserAction::GetLinks => {
                let ev = self.client.evaluate(
                    "Array.from(document.querySelectorAll('a'))\
                     .map(a => ({ href: a.href, text: a.innerText.trim() }))"
                ).await?;
                Ok(ev.result.value.unwrap_or(json!([])))
            }

            BrowserAction::GetAttributes { selector } => {
                let ev = self.client.evaluate(&format!(
                    "(sel => {{ \
                        let el = document.querySelector(sel); \
                        if (!el) return null; \
                        let attrs = {{}}; \
                        for (let a of el.attributes) attrs[a.name] = a.value; \
                        return attrs; \
                    }})({})",
                    quote(&selector)
                )).await?;
                Ok(ev.result.value.unwrap_or(json!(null)))
            }

            BrowserAction::Exists { selector } => {
                let ev = self.client.evaluate(&format!(
                    "!!document.querySelector({})",
                    quote(&selector)
                )).await?;
                Ok(ev.result.value.unwrap_or(json!(false)))
            }

            BrowserAction::Screenshot { path } => match path {
                Some(p) => {
                    self.client.full_page_screenshot_to_file(&p).await?;
                    Ok(json!(p))
                }
                None => Ok(json!(self.client.full_page_screenshot().await?.len())),
            },

            BrowserAction::Evaluate { expression } => {
                let ev = self.client.evaluate(&expression).await?;
                Ok(ev.result.value.unwrap_or(json!(null)))
            }

            BrowserAction::Wait { ms } => {
                tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
                Ok(json!(null))
            }

            BrowserAction::WaitForSelector { selector, timeout_ms } => {
                let deadline = std::time::Instant::now()
                    + std::time::Duration::from_millis(timeout_ms);
                let expr = format!(
                    "!!document.querySelector({})",
                    quote(&selector)
                );
                loop {
                    let ev = self.client.evaluate(&expr).await?;
                    if ev.result.value == Some(json!(true)) {
                        return Ok(json!(true));
                    }
                    if std::time::Instant::now() >= deadline {
                        return Err(CdpError::Timeout);
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            }

            BrowserAction::Scroll { x, y } => {
                self.client.eval(&format!("window.scrollTo({x}, {y})")).await?;
                Ok(json!(null))
            }

            BrowserAction::SetViewport { width, height, mobile } => {
                self.client.set_viewport(width, height, mobile).await?;
                Ok(json!(null))
            }

            BrowserAction::GetMetrics => {
                self.client.send_command("Performance.getMetrics", json!({})).await
            }
        }
    }
}

fn key_info(key: &str) -> (&str, u32) {
    match key {
        "Enter"      => ("Enter",      13),
        "Tab"        => ("Tab",         9),
        "Backspace"  => ("Backspace",   8),
        "Delete"     => ("Delete",     46),
        "Escape"     => ("Escape",     27),
        " " | "Space"=> ("Space",      32),
        "ArrowLeft"  => ("ArrowLeft",  37),
        "ArrowUp"    => ("ArrowUp",    38),
        "ArrowRight" => ("ArrowRight", 39),
        "ArrowDown"  => ("ArrowDown",  40),
        _            => (key,           0),
    }
}

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

fn parse_action(json_str: &str) -> Result<BrowserAction> {
    let a: RawAction = serde_json::from_str(json_str)?;

    macro_rules! need {
        ($field:expr, $name:literal) => {
            $field.ok_or_else(|| CdpError::Protocol(concat!($name, " is required").into()))?
        };
    }

    Ok(match a.action.as_str() {
        "navigate"               => BrowserAction::Navigate { url: need!(a.url, "url") },
        "back"     | "go_back"   => BrowserAction::GoBack,
        "forward"  | "go_forward"=> BrowserAction::GoForward,
        "reload"                 => BrowserAction::Reload,
        "click"                  => BrowserAction::Click { selector: a.selector, x: a.x, y: a.y },
        "type"                   => BrowserAction::Type { text: need!(a.text, "text"), selector: a.selector },
        "fill"                   => BrowserAction::Fill { selector: need!(a.selector, "selector"), value: need!(a.value, "value") },
        "submit"                 => BrowserAction::Submit { selector: a.selector },
        "press_key" | "key"      => BrowserAction::PressKey { key: need!(a.key, "key") },
        "get_title" | "title"    => BrowserAction::GetTitle,
        "get_url"   | "url"      => BrowserAction::GetUrl,
        "get_text"  | "text"     => BrowserAction::GetText,
        "get_content"| "content" => BrowserAction::GetContent { selector: a.selector },
        "get_links" | "links"    => BrowserAction::GetLinks,
        "get_attributes" | "attributes" => BrowserAction::GetAttributes { selector: need!(a.selector, "selector") },
        "exists"                 => BrowserAction::Exists { selector: need!(a.selector, "selector") },
        "screenshot"             => BrowserAction::Screenshot { path: a.path },
        "evaluate"  | "eval"     => BrowserAction::Evaluate { expression: need!(a.expression, "expression") },
        "wait"                   => BrowserAction::Wait { ms: need!(a.ms, "ms") },
        "wait_for_selector"      => BrowserAction::WaitForSelector {
            selector: need!(a.selector, "selector"),
            timeout_ms: a.timeout_ms.unwrap_or(5000),
        },
        "scroll"                 => BrowserAction::Scroll { x: a.x.unwrap_or(0.0), y: a.y.unwrap_or(0.0) },
        "set_viewport"           => BrowserAction::SetViewport {
            width: need!(a.width, "width"),
            height: need!(a.height, "height"),
            mobile: a.mobile.unwrap_or(false),
        },
        "get_metrics" | "metrics"=> BrowserAction::GetMetrics,
        other => return Err(CdpError::Protocol(format!("unknown action: {other}"))),
    })
}

pub struct ActionBuilder {
    actions: Vec<BrowserAction>,
}

impl ActionBuilder {
    pub fn new() -> Self {
        ActionBuilder { actions: Vec::new() }
    }

    pub fn navigate(mut self, url: &str) -> Self {
        self.actions.push(BrowserAction::Navigate { url: url.into() });
        self
    }

    pub fn wait(mut self, ms: u64) -> Self {
        self.actions.push(BrowserAction::Wait { ms });
        self
    }

    pub fn click(mut self, selector: &str) -> Self {
        self.actions.push(BrowserAction::Click { selector: Some(selector.into()), x: None, y: None });
        self
    }

    pub fn fill(mut self, selector: &str, value: &str) -> Self {
        self.actions.push(BrowserAction::Fill { selector: selector.into(), value: value.into() });
        self
    }

    pub fn press_key(mut self, key: &str) -> Self {
        self.actions.push(BrowserAction::PressKey { key: key.into() });
        self
    }

    pub fn screenshot(mut self, path: Option<&str>) -> Self {
        self.actions.push(BrowserAction::Screenshot { path: path.map(Into::into) });
        self
    }

    pub fn evaluate(mut self, expr: &str) -> Self {
        self.actions.push(BrowserAction::Evaluate { expression: expr.into() });
        self
    }

    pub fn scroll(mut self, x: f64, y: f64) -> Self {
        self.actions.push(BrowserAction::Scroll { x, y });
        self
    }

    pub fn get_title(mut self) -> Self {
        self.actions.push(BrowserAction::GetTitle);
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
