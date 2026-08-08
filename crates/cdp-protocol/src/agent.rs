//! Running [`BrowserAction`]s against a tab and reporting how each one went.

use serde_json::{json, Value};
use tokio::sync::broadcast;

use crate::action_parse::parse_action;
use crate::client::CdpClient;
use crate::config::Config;
use crate::error::{CdpError, Result};
use crate::keys::key_info;
use crate::types::ConsoleMessage;

// Re-exported so `crate::agent::{BrowserAction, ActionBuilder}` keeps resolving
// now that these live in their own modules.
pub use crate::action::BrowserAction;
pub use crate::action_builder::ActionBuilder;

/// How often `WaitForSelector` re-checks the page.
const SELECTOR_POLL_MS: u64 = 100;

/// How many console messages may queue before the oldest are dropped.
const CONSOLE_BACKLOG: usize = 64;

/// Render `s` as a JavaScript string literal, so a selector or value containing
/// quotes cannot break out of the expression it is spliced into.
fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s.replace('"', "\\\"")))
}

/// Outcome of [`BrowserAgent::execute`]. Errors are captured as strings rather than
/// propagated so a batch of actions ([`BrowserAgent::execute_many`]) can run to
/// completion and report per-action success/failure.
#[derive(Debug, Clone)]
pub struct ActionResult {
    /// Whether the action completed without error.
    pub success: bool,
    /// The action's return value, when it produces one and succeeded.
    pub value: Option<Value>,
    /// The error message, when `success` is `false`.
    pub error: Option<String>,
}

impl ActionResult {
    /// Shorthand for `self.success`.
    pub fn is_success(&self) -> bool {
        self.success
    }

    /// The outcome of an action that raised `error`.
    fn failed(error: impl std::fmt::Display) -> Self {
        ActionResult {
            success: false,
            value: None,
            error: Some(error.to_string()),
        }
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

/// Drives a single browser tab via [`BrowserAction`]s. Wraps a [`CdpClient`] with
/// `Page`, `Runtime`, `DOM`, and `Network` already enabled.
pub struct BrowserAgent {
    client: CdpClient,
}

impl BrowserAgent {
    /// Connect to the first available page target and enable the domains actions
    /// depend on.
    pub async fn connect(host: &str, port: u16) -> Result<Self> {
        let client = CdpClient::connect_to_page(host, port).await?;
        for domain in ["Page", "Runtime", "DOM", "Network"] {
            client.enable_domain(domain).await?;
        }
        Ok(BrowserAgent { client })
    }

    /// [`connect`](Self::connect) using `config.host`/`config.port`, then apply
    /// `config`'s viewport size.
    pub async fn connect_with_config(config: &Config) -> Result<Self> {
        let agent = Self::connect(&config.host, config.port).await?;
        agent
            .client
            .set_viewport(config.viewport_width, config.viewport_height, false)
            .await?;
        Ok(agent)
    }

    /// Wrap an already-connected client (domains are the caller's responsibility).
    pub fn from_client(client: CdpClient) -> Self {
        BrowserAgent { client }
    }

    /// Close the underlying tab.
    pub async fn close(&self) -> Result<()> {
        self.client.close().await
    }

    /// Start capturing `console.*` calls from the page. Returns a receiver that
    /// yields each call as a [`ConsoleMessage`]; capture continues until the agent
    /// (and its underlying client) is dropped.
    pub fn capture_console(&self) -> broadcast::Receiver<ConsoleMessage> {
        let (tx, rx) = broadcast::channel(CONSOLE_BACKLOG);
        let mut events = self.client.subscribe_events();
        tokio::spawn(async move {
            while let Ok((method, params)) = events.recv().await {
                if method != "Runtime.consoleAPICalled" {
                    continue;
                }
                let level = params["type"].as_str().unwrap_or("log").to_string();
                let text = params["args"]
                    .as_array()
                    .and_then(|args| args.first())
                    .and_then(|a| a["value"].as_str())
                    .unwrap_or("")
                    .to_string();
                let url = params["stackTrace"]["callFrames"][0]["url"]
                    .as_str()
                    .map(str::to_owned);
                let line = params["stackTrace"]["callFrames"][0]["lineNumber"].as_u64();
                let _ = tx.send(ConsoleMessage {
                    level,
                    text,
                    url,
                    line,
                });
            }
        });
        rx
    }

    /// Run one action, capturing success/failure into an [`ActionResult`] instead of
    /// returning `Result`.
    pub async fn execute(&self, action: BrowserAction) -> ActionResult {
        match self.dispatch(action).await {
            Ok(value) => ActionResult {
                success: true,
                value: Some(value),
                error: None,
            },
            Err(e) => ActionResult::failed(e),
        }
    }

    /// Run each action in `actions` in order, continuing even if one fails.
    pub async fn execute_many(&self, actions: Vec<BrowserAction>) -> Vec<ActionResult> {
        let mut results = Vec::with_capacity(actions.len());
        for action in actions {
            results.push(self.execute(action).await);
        }
        results
    }

    /// Parse `json_str` as a [`BrowserAction`] (e.g. `{"action": "navigate", "url":
    /// "..."}`) and run it. Suited for dispatching LLM tool calls directly.
    pub async fn execute_json(&self, json_str: &str) -> ActionResult {
        match parse_action(json_str) {
            Ok(action) => self.execute(action).await,
            Err(e) => ActionResult::failed(e),
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
                    self.client
                        .eval(&format!("document.querySelector({})?.click()", quote(&sel)))
                        .await?;
                } else if let (Some(cx), Some(cy)) = (x, y) {
                    for event_type in ["mousePressed", "mouseReleased"] {
                        self.client
                            .send_command(
                                "Input.dispatchMouseEvent",
                                json!({
                                    "type": event_type, "x": cx, "y": cy,
                                    "button": "left", "clickCount": 1,
                                }),
                            )
                            .await?;
                    }
                } else {
                    return Err(CdpError::Protocol("click: need selector or (x, y)".into()));
                }
                Ok(json!(null))
            }

            BrowserAction::Type { text, selector } => {
                if let Some(sel) = selector {
                    self.client
                        .eval(&format!("document.querySelector({})?.focus()", quote(&sel)))
                        .await?;
                }
                self.client
                    .send_command("Input.insertText", json!({ "text": text }))
                    .await?;
                Ok(json!(null))
            }

            BrowserAction::Fill { selector, value } => {
                self.client
                    .eval(&format!(
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
                    ))
                    .await?;
                Ok(json!(null))
            }

            BrowserAction::Submit { selector } => {
                let sel = selector.as_deref().unwrap_or("form");
                self.client
                    .eval(&format!("document.querySelector({})?.submit()", quote(sel)))
                    .await?;
                Ok(json!(null))
            }

            BrowserAction::PressKey { key } => {
                let (code, vk) = key_info(&key);
                self.client
                    .send_command(
                        "Input.dispatchKeyEvent",
                        json!({
                            "type": "keyDown", "key": key, "code": code,
                            "windowsVirtualKeyCode": vk,
                        }),
                    )
                    .await?;
                self.client
                    .send_command(
                        "Input.dispatchKeyEvent",
                        json!({
                            "type": "keyUp", "key": key, "code": code,
                        }),
                    )
                    .await?;
                Ok(json!(null))
            }

            BrowserAction::GetTitle => Ok(json!(self.client.eval("document.title").await?)),
            BrowserAction::GetUrl => Ok(json!(self.client.eval("window.location.href").await?)),
            BrowserAction::GetText => Ok(json!(self.client.eval("document.body.innerText").await?)),

            BrowserAction::GetContent { selector } => {
                let expr = match selector {
                    Some(sel) => format!("document.querySelector({})?.innerHTML", quote(&sel)),
                    None => "document.documentElement.outerHTML".into(),
                };
                Ok(json!(self.client.eval(&expr).await?))
            }

            BrowserAction::GetLinks => {
                let ev = self
                    .client
                    .evaluate(
                        "Array.from(document.querySelectorAll('a'))\
                     .map(a => ({ href: a.href, text: a.innerText.trim() }))",
                    )
                    .await?;
                Ok(ev.result.value.unwrap_or(json!([])))
            }

            BrowserAction::GetAttributes { selector } => {
                let ev = self
                    .client
                    .evaluate(&format!(
                        "(sel => {{ \
                        let el = document.querySelector(sel); \
                        if (!el) return null; \
                        let attrs = {{}}; \
                        for (let a of el.attributes) attrs[a.name] = a.value; \
                        return attrs; \
                    }})({})",
                        quote(&selector)
                    ))
                    .await?;
                Ok(ev.result.value.unwrap_or(json!(null)))
            }

            BrowserAction::Exists { selector } => {
                let ev = self
                    .client
                    .evaluate(&format!("!!document.querySelector({})", quote(&selector)))
                    .await?;
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

            BrowserAction::WaitForSelector {
                selector,
                timeout_ms,
            } => {
                let deadline =
                    std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
                let expr = format!("!!document.querySelector({})", quote(&selector));
                loop {
                    let ev = self.client.evaluate(&expr).await?;
                    if ev.result.value == Some(json!(true)) {
                        return Ok(json!(true));
                    }
                    if std::time::Instant::now() >= deadline {
                        return Err(CdpError::Timeout);
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(SELECTOR_POLL_MS)).await;
                }
            }

            BrowserAction::Scroll { x, y } => {
                self.client
                    .eval(&format!("window.scrollTo({x}, {y})"))
                    .await?;
                Ok(json!(null))
            }

            BrowserAction::SetViewport {
                width,
                height,
                mobile,
            } => {
                self.client.set_viewport(width, height, mobile).await?;
                Ok(json!(null))
            }

            BrowserAction::GetMetrics => {
                self.client
                    .send_command("Performance.getMetrics", json!({}))
                    .await
            }
        }
    }
}
