#![deny(clippy::all)]

//! Node/Deno/Bun bindings for the `cdp-protocol` crate.
//!
//! Three classes are exported:
//! - [`CdpClient`]  low-level CDP client (1:1 with the Rust `CdpClient`).
//! - [`BrowserAgent`]  high-level action runner (navigate/click/fill/...).
//! - [`Cluster`]  fixed-size pool of agents for concurrent work.

use std::sync::Arc;

use napi::bindgen_prelude::{Buffer, Error, Result};
use napi_derive::napi;
use serde_json::Value;
use tokio::sync::{Mutex, Semaphore};

use cdp_protocol::{
    BrowserAgent as CoreAgent, CdpClient as CoreClient, CdpError, Config as CoreConfig,
};

fn to_napi(e: CdpError) -> Error {
    Error::from_reason(e.to_string())
}

fn json_err(e: serde_json::Error) -> Error {
    Error::from_reason(e.to_string())
}

// ---------------------------------------------------------------------------
// Shared value objects
// ---------------------------------------------------------------------------

/// Connection / viewport configuration.
#[napi(object)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub viewport_width: i32,
    pub viewport_height: i32,
}

impl Default for Config {
    fn default() -> Self {
        let c = CoreConfig::default();
        Config {
            host: c.host,
            port: c.port,
            viewport_width: c.viewport_width,
            viewport_height: c.viewport_height,
        }
    }
}

impl From<Config> for CoreConfig {
    fn from(c: Config) -> Self {
        let mut base = CoreConfig::default();
        base.host = c.host;
        base.port = c.port;
        base.viewport_width = c.viewport_width;
        base.viewport_height = c.viewport_height;
        base
    }
}

/// Result of one browser action.
#[napi(object)]
pub struct ActionResult {
    pub success: bool,
    /// Action output as a JSON value (`null` when the action returns nothing).
    pub value: Option<Value>,
    pub error: Option<String>,
}

/// Result of one clustered task.
#[napi(object)]
pub struct TaskResult {
    pub success: bool,
    pub results: Vec<ActionResult>,
    pub elapsed_ms: f64,
    pub attempts: u32,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// CdpClient  low-level
// ---------------------------------------------------------------------------

/// Chrome DevTools Protocol client.
///
/// Connects to a Chrome/Chromium already listening with
/// `--remote-debugging-port`. It does not spawn the browser.
#[napi]
pub struct CdpClient {
    inner: Arc<CoreClient>,
}

#[napi]
impl CdpClient {
    /// Connect to a target WebSocket debugger URL.
    #[napi(factory)]
    pub async fn connect(ws_url: String) -> Result<CdpClient> {
        let inner = CoreClient::connect(&ws_url).await.map_err(to_napi)?;
        Ok(CdpClient { inner: Arc::new(inner) })
    }

    /// Discover the first `page` target on `host:port` and connect to it.
    #[napi(factory)]
    pub async fn connect_to_page(host: String, port: u16) -> Result<CdpClient> {
        let inner = CoreClient::connect_to_page(&host, port)
            .await
            .map_err(to_napi)?;
        Ok(CdpClient { inner: Arc::new(inner) })
    }

    /// `GET /json/version`  browser + protocol version.
    #[napi]
    pub async fn get_version(host: String, port: u16) -> Result<Value> {
        let v = CoreClient::get_version(&host, port).await.map_err(to_napi)?;
        serde_json::to_value(v).map_err(json_err)
    }

    /// `GET /json/list`  all inspectable targets.
    #[napi]
    pub async fn list_targets(host: String, port: u16) -> Result<Value> {
        let t = CoreClient::list_targets(&host, port).await.map_err(to_napi)?;
        serde_json::to_value(t).map_err(json_err)
    }

    /// `PUT /json/new`  open a new tab, optionally at `url`.
    #[napi]
    pub async fn create_tab(host: String, port: u16, url: Option<String>) -> Result<Value> {
        let t = CoreClient::create_tab(&host, port, url.as_deref())
            .await
            .map_err(to_napi)?;
        serde_json::to_value(t).map_err(json_err)
    }

    /// Enable a CDP domain, e.g. `"Page"`, `"Runtime"`, `"DOM"`, `"Network"`.
    #[napi]
    pub async fn enable_domain(&self, domain: String) -> Result<()> {
        self.inner.enable_domain(&domain).await.map_err(to_napi)
    }

    /// Navigate to `url`; returns the CDP frameId.
    #[napi]
    pub async fn navigate(&self, url: String) -> Result<String> {
        let inner = self.inner.clone();
        Ok(inner.navigate(&url).await.map_err(to_napi)?.frame_id)
    }

    /// Navigate and resolve once `Page.loadEventFired` arrives (needs `Page` enabled).
    #[napi]
    pub async fn navigate_and_wait(&self, url: String, timeout_ms: i64) -> Result<String> {
        let inner = self.inner.clone();
        let r = inner
            .navigate_and_wait(&url, timeout_ms as u64)
            .await
            .map_err(to_napi)?;
        Ok(r.frame_id)
    }

    /// Evaluate a JS expression, returning the value coerced to a string.
    #[napi]
    pub async fn eval(&self, expression: String) -> Result<String> {
        let inner = self.inner.clone();
        inner.eval(&expression).await.map_err(to_napi)
    }

    /// Evaluate a JS expression, returning the full result as a JSON value.
    #[napi]
    pub async fn evaluate(&self, expression: String) -> Result<Value> {
        let inner = self.inner.clone();
        let r = inner.evaluate(&expression).await.map_err(to_napi)?;
        Ok(r.result.value.unwrap_or(Value::Null))
    }

    /// Wait for a CDP event `method`, returning its params (needs the domain enabled).
    #[napi]
    pub async fn wait_for_event(&self, method: String, timeout_ms: i64) -> Result<Value> {
        let inner = self.inner.clone();
        inner
            .wait_for_event(&method, timeout_ms as u64)
            .await
            .map_err(to_napi)
    }

    /// `DOM.querySelector` from the document root; returns the matched nodeId (0 if none).
    #[napi]
    pub async fn query_selector(&self, selector: String) -> Result<i64> {
        let inner = self.inner.clone();
        let doc = inner.get_document().await.map_err(to_napi)?;
        inner
            .query_selector(doc.node_id, &selector)
            .await
            .map_err(to_napi)
    }

    /// `DOM.getOuterHTML` for a nodeId.
    #[napi]
    pub async fn get_outer_html(&self, node_id: i64) -> Result<String> {
        let inner = self.inner.clone();
        inner.get_outer_html(node_id).await.map_err(to_napi)
    }

    /// PNG screenshot of the current viewport.
    #[napi]
    pub async fn screenshot(&self) -> Result<Buffer> {
        let inner = self.inner.clone();
        Ok(inner.screenshot().await.map_err(to_napi)?.into())
    }

    /// Write a viewport PNG screenshot to `path`.
    #[napi]
    pub async fn screenshot_to_file(&self, path: String) -> Result<()> {
        let inner = self.inner.clone();
        inner.screenshot_to_file(&path).await.map_err(to_napi)
    }

    /// Full-page PNG screenshot.
    #[napi]
    pub async fn full_page_screenshot(&self) -> Result<Buffer> {
        let inner = self.inner.clone();
        Ok(inner.full_page_screenshot().await.map_err(to_napi)?.into())
    }

    /// Write a full-page PNG screenshot to `path`.
    #[napi]
    pub async fn full_page_screenshot_to_file(&self, path: String) -> Result<()> {
        let inner = self.inner.clone();
        inner
            .full_page_screenshot_to_file(&path)
            .await
            .map_err(to_napi)
    }

    /// Override device metrics (viewport).
    #[napi]
    pub async fn set_viewport(&self, width: i32, height: i32, mobile: bool) -> Result<()> {
        let inner = self.inner.clone();
        inner.set_viewport(width, height, mobile).await.map_err(to_napi)
    }

    /// `Network.getCookies`.
    #[napi]
    pub async fn get_cookies(&self) -> Result<Value> {
        let inner = self.inner.clone();
        let cookies = inner.get_cookies().await.map_err(to_napi)?;
        serde_json::to_value(cookies).map_err(json_err)
    }

    /// Close the current tab.
    #[napi]
    pub async fn close(&self) -> Result<()> {
        let inner = self.inner.clone();
        inner.close().await.map_err(to_napi)
    }
}

// ---------------------------------------------------------------------------
// BrowserAgent  high-level
// ---------------------------------------------------------------------------

/// High-level action runner. Enables `Page`, `Runtime`, `DOM`, `Network` on connect.
///
/// Actions are plain objects, e.g. `{ action: 'navigate', url: 'https://...' }`.
/// See the README for the full action list.
#[napi]
pub struct BrowserAgent {
    inner: Arc<CoreAgent>,
}

#[napi]
impl BrowserAgent {
    /// Connect to the first `page` target on `host:port` and enable the core domains.
    #[napi(factory)]
    pub async fn connect(host: String, port: u16) -> Result<BrowserAgent> {
        let inner = CoreAgent::connect(&host, port).await.map_err(to_napi)?;
        Ok(BrowserAgent { inner: Arc::new(inner) })
    }

    /// Connect using a [`Config`] (also applies the viewport).
    #[napi(factory)]
    pub async fn connect_with_config(config: Config) -> Result<BrowserAgent> {
        let core: CoreConfig = config.into();
        let inner = CoreAgent::connect_with_config(&core)
            .await
            .map_err(to_napi)?;
        Ok(BrowserAgent { inner: Arc::new(inner) })
    }

    /// Run one action given as an object, e.g. `{ action: 'navigate', url }`.
    #[napi]
    pub async fn execute(&self, action: Value) -> Result<ActionResult> {
        let inner = self.inner.clone();
        let json = serde_json::to_string(&action).map_err(json_err)?;
        Ok(core_result(inner.execute_json(&json).await))
    }

    /// Run a JSON string action (`{ "action": "navigate", "url": "..." }`).
    #[napi]
    pub async fn execute_json(&self, json: String) -> Result<ActionResult> {
        let inner = self.inner.clone();
        Ok(core_result(inner.execute_json(&json).await))
    }

    /// Run an array of action objects sequentially.
    #[napi]
    pub async fn execute_many(&self, actions: Vec<Value>) -> Result<Vec<ActionResult>> {
        let inner = self.inner.clone();
        let mut out = Vec::with_capacity(actions.len());
        for a in actions {
            let json = serde_json::to_string(&a).map_err(json_err)?;
            out.push(core_result(inner.execute_json(&json).await));
        }
        Ok(out)
    }

    /// Convenience: navigate.
    #[napi]
    pub async fn navigate(&self, url: String) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "navigate", "url": url })).await
    }

    /// Convenience: click a selector.
    #[napi]
    pub async fn click(&self, selector: String) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "click", "selector": selector })).await
    }

    /// Convenience: fill an input.
    #[napi]
    pub async fn fill(&self, selector: String, value: String) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "fill", "selector": selector, "value": value })).await
    }

    /// Convenience: press a key.
    #[napi]
    pub async fn press_key(&self, key: String) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "press_key", "key": key })).await
    }

    /// Convenience: page title.
    #[napi]
    pub async fn get_title(&self) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "get_title" })).await
    }

    /// Convenience: visible text.
    #[napi]
    pub async fn get_text(&self) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "get_text" })).await
    }

    /// Convenience: `[{ href, text }]` for every anchor.
    #[napi]
    pub async fn get_links(&self) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "get_links" })).await
    }

    /// Convenience: does a selector match?
    #[napi]
    pub async fn exists(&self, selector: String) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "exists", "selector": selector })).await
    }

    /// Convenience: wait until a selector appears or timeout.
    #[napi]
    pub async fn wait_for_selector(&self, selector: String, timeout_ms: i64) -> Result<ActionResult> {
        self.run(serde_json::json!({
            "action": "wait_for_selector",
            "selector": selector,
            "timeout_ms": timeout_ms,
        }))
        .await
    }

    /// Convenience: full-page screenshot to `path` (returns byte count if omitted).
    #[napi]
    pub async fn screenshot(&self, path: Option<String>) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "screenshot", "path": path })).await
    }

    /// Convenience: evaluate a JS expression, return its JSON value.
    #[napi]
    pub async fn evaluate(&self, expression: String) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "evaluate", "expression": expression })).await
    }

    /// Close the underlying tab.
    #[napi]
    pub async fn close(&self) -> Result<()> {
        let inner = self.inner.clone();
        inner.close().await.map_err(to_napi)
    }

    async fn run(&self, action: Value) -> Result<ActionResult> {
        let inner = self.inner.clone();
        let json = serde_json::to_string(&action).map_err(json_err)?;
        Ok(core_result(inner.execute_json(&json).await))
    }
}

fn core_result(r: cdp_protocol::ActionResult) -> ActionResult {
    ActionResult {
        success: r.success,
        value: r.value,
        error: r.error,
    }
}

// ---------------------------------------------------------------------------
// Cluster  agent pool
// ---------------------------------------------------------------------------

/// Options for [`Cluster.create`].
#[napi(object)]
pub struct ClusterOptions {
    pub host: String,
    pub port: u16,
    /// Number of tabs / concurrent workers.
    pub concurrency: u32,
    /// Retries per task on failure (default 0).
    pub retries: Option<u32>,
    pub viewport_width: Option<i32>,
    pub viewport_height: Option<i32>,
}

struct Worker {
    agent: CoreAgent,
}

/// Fixed-size pool of [`BrowserAgent`] workers, one tab each.
#[napi]
pub struct Cluster {
    workers: Arc<Mutex<Vec<Arc<Worker>>>>,
    sem: Arc<Semaphore>,
    retries: u32,
}

#[napi]
impl Cluster {
    /// Open `concurrency` tabs and wrap each as a worker agent.
    #[napi(factory)]
    pub async fn create(opts: ClusterOptions) -> Result<Cluster> {
        let width = opts.viewport_width.unwrap_or(1920);
        let height = opts.viewport_height.unwrap_or(1200);
        let mut workers = Vec::with_capacity(opts.concurrency as usize);

        for i in 0..opts.concurrency {
            let target = CoreClient::create_tab(&opts.host, opts.port, None)
                .await
                .map_err(to_napi)?;
            let ws = target.web_socket_debugger_url.ok_or_else(|| {
                Error::from_reason(format!("worker {i}: target has no debugger URL"))
            })?;
            let client = CoreClient::connect(&ws).await.map_err(to_napi)?;
            for d in ["Page", "Runtime", "DOM", "Network"] {
                client.enable_domain(d).await.map_err(to_napi)?;
            }
            client.set_viewport(width, height, false).await.map_err(to_napi)?;
            workers.push(Arc::new(Worker {
                agent: CoreAgent::from_client(client),
            }));
        }

        Ok(Cluster {
            workers: Arc::new(Mutex::new(workers)),
            sem: Arc::new(Semaphore::new(opts.concurrency as usize)),
            retries: opts.retries.unwrap_or(0),
        })
    }

    /// Run one action batch on a free worker, with retries.
    #[napi]
    pub async fn execute(&self, actions: Vec<Value>) -> Result<TaskResult> {
        let jsons: std::result::Result<Vec<String>, _> =
            actions.iter().map(serde_json::to_string).collect();
        let jsons = jsons.map_err(json_err)?;

        let _permit = self.sem.acquire().await.expect("semaphore closed");
        let worker = self.workers.lock().await.pop().expect("worker missing");

        let start = std::time::Instant::now();
        let mut attempts = 0u32;
        let mut results;
        loop {
            attempts += 1;
            results = Vec::with_capacity(jsons.len());
            let mut ok = true;
            for j in &jsons {
                let r = core_result(worker.agent.execute_json(j).await);
                if !r.success {
                    ok = false;
                }
                let stop = !r.success;
                results.push(r);
                if stop {
                    break;
                }
            }
            if ok || attempts > self.retries {
                let success = ok;
                self.workers.lock().await.push(worker);
                return Ok(TaskResult {
                    success,
                    error: if success {
                        None
                    } else {
                        results.last().and_then(|r| r.error.clone())
                    },
                    results,
                    elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
                    attempts,
                });
            }
        }
    }

    /// Close every worker tab.
    #[napi]
    pub async fn close(&self) -> Result<()> {
        let workers = self.workers.lock().await;
        for w in workers.iter() {
            let _ = w.agent.close().await;
        }
        Ok(())
    }
}
