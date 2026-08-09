//! Low-level CDP client: one method per DevTools Protocol command.
//!
//! [`CdpClient`] owns a single WebSocket session to one debuggable target (tab).
//! Domain-specific commands live in sibling modules ([`crate::page`], [`crate::network`])
//! as `impl CdpClient` blocks.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use futures_util::{Sink, Stream};
use futures_util::{SinkExt, StreamExt};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::{Error as WsError, Message};
use tracing::{debug, warn};

use crate::error::{CdpError, Result};
use crate::types::*;

/// Default per-command timeout, in milliseconds. Override with
/// [`CdpClient::set_command_timeout`]. A value of `0` disables the timeout.
const DEFAULT_COMMAND_TIMEOUT_MS: u64 = 30_000;

/// How many events may queue for a subscriber before the oldest are dropped.
const EVENT_BACKLOG: usize = 256;

/// What every in-flight command is told when the socket goes away.
const CONNECTION_CLOSED: &str = "connection closed";

/// Stands in for the `code` of an error frame that carries none. CDP never uses
/// zero itself, so it cannot be mistaken for a code Chrome actually reported.
const UNREPORTED_ERROR_CODE: i64 = 0;

/// What an error frame with no message of its own is called.
const UNDESCRIBED_ERROR: &str = "protocol error";

type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>;

type Events = broadcast::Sender<(String, Value)>;

/// Decode the base64 payload a screenshot command answers with.
fn png_bytes_from(result: &Value) -> Result<Vec<u8>> {
    let data = result["data"]
        .as_str()
        .ok_or_else(|| CdpError::Protocol("screenshot response has no data".into()))?;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| CdpError::Protocol(e.to_string()))
}

/// Judge a `Page.navigate` result. Chrome answers a navigation it could not
/// perform with a frame id, a loader id and an `errorText`, so a result that
/// looks complete still has to be read as a failure.
fn navigation_outcome(url: &str, navigation: NavigationResult) -> Result<NavigationResult> {
    match &navigation.error_text {
        Some(reason) => Err(CdpError::Protocol(format!(
            "navigation to {url} failed: {reason}"
        ))),
        None => Ok(navigation),
    }
}

/// What to say about an expression that threw, given Chrome's `exceptionDetails`.
///
/// A thrown `Error` arrives as an object whose `description` is its message
/// followed by a stack trace, so only the first line is kept. Anything else JS
/// can throw (a string, a number) arrives as a plain `value` instead, and if
/// Chrome sends neither, its own summary `text` is all there is.
fn thrown_message(details: &Value) -> String {
    let exception = &details["exception"];
    if let Some(description) = exception["description"].as_str() {
        return description
            .lines()
            .next()
            .unwrap_or(description)
            .to_string();
    }
    match &exception["value"] {
        Value::Null => details["text"].as_str().unwrap_or("uncaught").to_string(),
        Value::String(thrown) => thrown.clone(),
        thrown => thrown.to_string(),
    }
}

/// Read the error object Chrome answers a rejected command with. Only `message`
/// is reliably present, so a frame carrying less still has to yield an error a
/// caller can match on.
fn browser_error_in(error: &Value) -> CdpError {
    CdpError::Browser {
        code: error["code"].as_i64().unwrap_or(UNREPORTED_ERROR_CODE),
        message: error["message"]
            .as_str()
            .unwrap_or(UNDESCRIBED_ERROR)
            .to_string(),
        data: match &error["data"] {
            Value::Null => None,
            Value::String(detail) => Some(detail.clone()),
            detail => Some(detail.to_string()),
        },
    }
}

/// The reply a command frame carries: its result, or the error it reports.
fn reply_in(frame: &Value, id: u64) -> Result<Value> {
    let Some(error) = frame.get("error") else {
        debug!(id, "recv");
        return Ok(frame.get("result").cloned().unwrap_or(Value::Null));
    };
    let rejection = browser_error_in(error);
    warn!(id, %rejection, "protocol error");
    Err(rejection)
}

/// Hand a reply to whoever is waiting on `id`, if anyone still is. Nobody is
/// waiting once a command has timed out, and that is not an error.
async fn deliver(pending: &PendingMap, id: u64, reply: Result<Value>) {
    if let Some(waiting) = pending.lock().await.remove(&id) {
        let _ = waiting.send(reply);
    }
}

/// Publish an unsolicited frame to event subscribers.
fn publish(events: &Events, frame: &Value) {
    let Some(method) = frame.get("method").and_then(Value::as_str) else {
        return;
    };
    debug!(%method, "event");
    let params = frame.get("params").cloned().unwrap_or(Value::Null);
    let _ = events.send((method.to_owned(), params));
}

/// Route one text frame. A frame carrying an `id` answers a command; anything
/// else is an event. Frames that are not JSON at all are dropped.
async fn route(text: &str, pending: &PendingMap, events: &Events) {
    let Ok(frame) = serde_json::from_str::<Value>(text) else {
        return;
    };
    match frame.get("id").and_then(Value::as_u64) {
        Some(id) => deliver(pending, id, reply_in(&frame, id)).await,
        None => publish(events, &frame),
    }
}

/// Fail every command still awaiting a reply.
async fn abandon_pending(pending: &PendingMap) {
    for (_, waiting) in pending.lock().await.drain() {
        let _ = waiting.send(Err(CdpError::Protocol(CONNECTION_CLOSED.into())));
    }
}

/// Forward queued commands to the socket until the client is dropped or the
/// socket refuses them.
async fn write_outgoing<S>(mut sink: S, mut outgoing: mpsc::UnboundedReceiver<Message>)
where
    S: Sink<Message> + Unpin,
{
    while let Some(message) = outgoing.recv().await {
        if sink.send(message).await.is_err() {
            break;
        }
    }
}

/// Demultiplex the socket for the client's lifetime, sending replies to their
/// callers and events to subscribers.
///
/// However the stream ends, whether closed cleanly, failed, or simply exhausted,
/// the commands still in flight are failed rather than left to time out.
async fn read_incoming<S>(mut stream: S, pending: PendingMap, events: Events)
where
    S: Stream<Item = std::result::Result<Message, WsError>> + Unpin,
{
    while let Some(message) = stream.next().await {
        match message {
            Ok(Message::Text(text)) => route(&text, &pending, &events).await,
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }
    abandon_pending(&pending).await;
}

/// A single WebSocket session to one Chrome debugging target.
///
/// Cloning is not supported; share a client across tasks with `Arc<CdpClient>`
/// (see [`Cluster`](crate::cluster::Cluster) for an example). Every command sent
/// concurrently gets its own response future, matched by CDP message id.
pub struct CdpClient {
    tx: tokio::sync::mpsc::UnboundedSender<Message>,
    pending: PendingMap,
    next_id: Arc<AtomicU64>,
    events_tx: broadcast::Sender<(String, Value)>,
    command_timeout_ms: Arc<AtomicU64>,
}

impl CdpClient {
    /// Open a CDP WebSocket session at `ws_url` (a target's `webSocketDebuggerUrl`).
    ///
    /// Spawns background tasks that pump outgoing commands and demultiplex incoming
    /// replies/events for the lifetime of the returned client.
    pub async fn connect(ws_url: &str) -> Result<Self> {
        debug!(%ws_url, "connecting");
        let (ws_stream, _) = connect_async(ws_url).await?;
        let (sink, stream) = ws_stream.split();

        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let (events_tx, _) = broadcast::channel::<(String, Value)>(EVENT_BACKLOG);
        let (tx, outgoing) = mpsc::unbounded_channel::<Message>();

        tokio::spawn(write_outgoing(sink, outgoing));
        tokio::spawn(read_incoming(stream, pending.clone(), events_tx.clone()));

        Ok(CdpClient {
            tx,
            pending,
            next_id: Arc::new(AtomicU64::new(1)),
            events_tx,
            command_timeout_ms: Arc::new(AtomicU64::new(DEFAULT_COMMAND_TIMEOUT_MS)),
        })
    }

    /// Set the per-command timeout applied to every CDP command this client sends.
    /// Passing a zero duration disables the timeout (commands wait indefinitely).
    pub fn set_command_timeout(&self, timeout: Duration) {
        let ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
        self.command_timeout_ms.store(ms, Ordering::Relaxed);
    }

    /// The current per-command timeout.
    pub fn command_timeout(&self) -> Duration {
        Duration::from_millis(self.command_timeout_ms.load(Ordering::Relaxed))
    }

    pub(crate) async fn send_command(&self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        debug!(%method, id, "send");
        let (tx, rx) = oneshot::channel();

        self.pending.lock().await.insert(id, tx);

        if let Err(e) = self.tx.send(Message::Text(
            json!({ "id": id, "method": method, "params": params })
                .to_string()
                .into(),
        )) {
            // Writer task is gone; don't leave a dangling entry in `pending`.
            self.pending.lock().await.remove(&id);
            return Err(CdpError::Protocol(e.to_string()));
        }

        let closed = || CdpError::Protocol("response channel closed".into());

        let timeout_ms = self.command_timeout_ms.load(Ordering::Relaxed);
        if timeout_ms == 0 {
            return match rx.await {
                Ok(outcome) => outcome,
                Err(_) => Err(closed()),
            };
        }

        match tokio::time::timeout(Duration::from_millis(timeout_ms), rx).await {
            Ok(Ok(outcome)) => outcome,
            Ok(Err(_)) => Err(closed()),
            Err(_) => {
                // Timed out waiting for a reply: drop the pending sender so the
                // map doesn't grow unbounded for commands the browser never answers.
                self.pending.lock().await.remove(&id);
                Err(CdpError::Timeout)
            }
        }
    }

    /// Send any CDP command and deserialize its result into `R`.
    ///
    /// The typed wrappers on this client cover only a small, curated slice of the
    /// DevTools Protocol; this is the escape hatch for everything else, including
    /// domains this crate has no wrappers for at all (`Target`, `Storage`,
    /// `Debugger`, ...). `params` must serialize to a JSON object, since that is
    /// what CDP expects; use `json!({})` for commands that take no parameters.
    ///
    /// ```no_run
    /// # use cdp_driver::CdpClient;
    /// # use serde::Deserialize;
    /// # use serde_json::json;
    /// #[derive(Deserialize)]
    /// #[serde(rename_all = "camelCase")]
    /// struct CreateTarget {
    ///     target_id: String,
    /// }
    ///
    /// # async fn run(client: &CdpClient) -> cdp_driver::Result<()> {
    /// let created: CreateTarget = client
    ///     .call("Target.createTarget", json!({ "url": "about:blank" }))
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn call<P: Serialize, R: DeserializeOwned>(
        &self,
        method: &str,
        params: P,
    ) -> Result<R> {
        let result = self.call_raw(method, serde_json::to_value(params)?).await?;
        Ok(serde_json::from_value(result)?)
    }

    /// Send any CDP command and return its raw `result` object unparsed.
    ///
    /// Prefer [`call`](Self::call) when you have a type to deserialize into. This is
    /// the lower-level form, useful for exploring a response's shape or for commands
    /// whose result you only want to index into. Commands that return no result
    /// yield [`Value::Null`].
    pub async fn call_raw(&self, method: &str, params: Value) -> Result<Value> {
        self.send_command(method, params).await
    }

    /// Subscribe to raw CDP events as `(method, params)` pairs.
    ///
    /// Only events for domains enabled via [`enable_domain`](Self::enable_domain)
    /// are emitted. Lagging receivers silently drop the oldest events rather than
    /// blocking the connection.
    pub fn subscribe_events(&self) -> broadcast::Receiver<(String, Value)> {
        self.events_tx.subscribe()
    }

    /// Wait for the next event named `method`, up to `timeout_ms` milliseconds.
    pub async fn wait_for_event(&self, method: &str, timeout_ms: u64) -> Result<Value> {
        let mut rx = self.events_tx.subscribe();
        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), async move {
            loop {
                match rx.recv().await {
                    Ok((m, params)) if m == method => return Ok(params),
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => return Err(CdpError::Protocol("event channel closed".into())),
                }
            }
        })
        .await
        .map_err(|_| CdpError::Timeout)?
    }

    /// Enable a CDP domain (e.g. `"Page"`, `"Runtime"`, `"Network"`), required before
    /// that domain's events start flowing or some of its commands work.
    pub async fn enable_domain(&self, domain: &str) -> Result<()> {
        self.send_command(&format!("{domain}.enable"), json!({}))
            .await?;
        Ok(())
    }

    /// Navigate to `url`. Returns as soon as navigation starts, without waiting for
    /// the page to finish loading; use [`navigate_and_wait`](Self::navigate_and_wait)
    /// to block until `Page.loadEventFired`.
    ///
    /// A navigation Chrome refused (an unresolvable host, a refused connection, a
    /// blocked request) is a [`CdpError::Protocol`] carrying Chrome's `errorText`,
    /// not a successful result.
    pub async fn navigate(&self, url: &str) -> Result<NavigationResult> {
        let result = self
            .send_command("Page.navigate", json!({ "url": url }))
            .await?;
        navigation_outcome(url, serde_json::from_value(result)?)
    }

    /// Navigate to `url` and wait for `Page.loadEventFired`, up to `timeout_ms`.
    /// Requires the `"Page"` domain to be enabled.
    ///
    /// Fails the same way [`navigate`](Self::navigate) does, before waiting for a
    /// load event that a refused navigation would never fire.
    pub async fn navigate_and_wait(&self, url: &str, timeout_ms: u64) -> Result<NavigationResult> {
        let mut rx = self.events_tx.subscribe();
        let nav = self.navigate(url).await?;
        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), async move {
            loop {
                match rx.recv().await {
                    Ok((m, _)) if m == "Page.loadEventFired" => return Ok(nav),
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => return Err(CdpError::Protocol("event channel closed".into())),
                }
            }
        })
        .await
        .map_err(|_| CdpError::Timeout)?
    }

    /// Evaluate a JS expression and return its result stringified.
    ///
    /// An expression that throws is a [`CdpError::Protocol`] carrying the thrown
    /// message, since a returned string cannot tell an empty result apart from a
    /// failure. Use [`evaluate`](Self::evaluate) when you want the structured
    /// result, or an exception as data rather than as an error.
    pub async fn eval(&self, expression: &str) -> Result<String> {
        let outcome = self.evaluate(expression).await?;
        if let Some(details) = &outcome.exception_details {
            return Err(CdpError::Protocol(format!(
                "evaluation threw: {}",
                thrown_message(details)
            )));
        }
        Ok(outcome
            .result
            .value
            .map(|v| match v {
                Value::String(s) => s,
                other => other.to_string(),
            })
            .unwrap_or_default())
    }

    /// Evaluate a JS expression via `Runtime.evaluate` and return the full result,
    /// including any exception details.
    pub async fn evaluate(&self, expression: &str) -> Result<EvaluateResult> {
        let result = self
            .send_command(
                "Runtime.evaluate",
                json!({ "expression": expression, "returnByValue": true }),
            )
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    /// Fetch the document's root node. Requires the `"DOM"` domain to be enabled.
    pub async fn get_document(&self) -> Result<DocumentNode> {
        let result = self
            .send_command("DOM.getDocument", json!({ "depth": 0 }))
            .await?;
        let root = result["root"].clone();
        if root.is_null() {
            return Err(CdpError::Protocol(
                "DOM.getDocument returned no root".into(),
            ));
        }
        Ok(serde_json::from_value(root)?)
    }

    /// Returns the matched `nodeId`, or `None` when nothing matches.
    /// CDP reports a missing match as `nodeId` 0; this surfaces that as `None`
    /// rather than a node id that looks valid.
    pub async fn query_selector(&self, node_id: i64, selector: &str) -> Result<Option<i64>> {
        let result = self
            .send_command(
                "DOM.querySelector",
                json!({ "nodeId": node_id, "selector": selector }),
            )
            .await?;
        Ok(match result["nodeId"].as_i64() {
            Some(id) if id > 0 => Some(id),
            _ => None,
        })
    }

    /// Serialize a node's outer HTML.
    pub async fn get_outer_html(&self, node_id: i64) -> Result<String> {
        let result = self
            .send_command("DOM.getOuterHTML", json!({ "nodeId": node_id }))
            .await?;
        Ok(result["outerHTML"].as_str().unwrap_or("").to_string())
    }

    /// Close the tab this client is attached to.
    pub async fn close(&self) -> Result<()> {
        // Ignore errors, connection drops immediately after the tab closes
        let _ = self.send_command("Page.close", json!({})).await;
        Ok(())
    }

    /// Capture a PNG screenshot of the current viewport.
    pub async fn screenshot(&self) -> Result<Vec<u8>> {
        let result = self
            .send_command(
                "Page.captureScreenshot",
                json!({ "format": "png", "fromSurface": true }),
            )
            .await?;
        png_bytes_from(&result)
    }

    /// [`screenshot`](Self::screenshot), written directly to `path`.
    pub async fn screenshot_to_file(&self, path: &str) -> Result<()> {
        tokio::fs::write(path, self.screenshot().await?).await?;
        Ok(())
    }

    /// Capture a PNG screenshot of the full page, resizing the viewport to the
    /// page's scroll size first (restoring it is the caller's responsibility).
    pub async fn full_page_screenshot(&self) -> Result<Vec<u8>> {
        let size = self
            .evaluate(
                "(() => ({ \
                w: Math.max(document.body.scrollWidth, document.documentElement.scrollWidth), \
                h: Math.max(document.body.scrollHeight, document.documentElement.scrollHeight) \
            }))()",
            )
            .await?;

        let dims = size.result.value.as_ref();
        let w = dims.and_then(|v| v["w"].as_i64()).unwrap_or(1920) as i32;
        let h = dims.and_then(|v| v["h"].as_i64()).unwrap_or(1200) as i32;
        self.set_viewport(w.max(1920), h.max(1200), false).await?;

        let result = self
            .send_command(
                "Page.captureScreenshot",
                json!({
                    "format": "png",
                    "captureBeyondViewport": true,
                    "fromSurface": true,
                }),
            )
            .await?;
        png_bytes_from(&result)
    }

    /// [`full_page_screenshot`](Self::full_page_screenshot), written directly to `path`.
    pub async fn full_page_screenshot_to_file(&self, path: &str) -> Result<()> {
        tokio::fs::write(path, self.full_page_screenshot().await?).await?;
        Ok(())
    }

    /// Override the viewport size and mobile emulation flag.
    pub async fn set_viewport(&self, width: i32, height: i32, mobile: bool) -> Result<()> {
        self.send_command(
            "Emulation.setDeviceMetricsOverride",
            json!({ "width": width, "height": height, "deviceScaleFactor": 1, "mobile": mobile }),
        )
        .await?;
        Ok(())
    }

    /// List cookies visible to the current page.
    pub async fn get_cookies(&self) -> Result<Vec<Cookie>> {
        let result = self.send_command("Network.getCookies", json!({})).await?;
        Ok(serde_json::from_value(result["cookies"].clone())?)
    }

    /// `GET /json/version` on Chrome's debugging HTTP endpoint.
    pub async fn get_version(host: &str, port: u16) -> Result<BrowserVersion> {
        let url = format!("http://{host}:{port}/json/version");
        Ok(reqwest::get(&url).await?.json().await?)
    }

    /// `GET /json/list`: enumerate debuggable targets (tabs, workers, ...).
    pub async fn list_targets(host: &str, port: u16) -> Result<Vec<Target>> {
        let url = format!("http://{host}:{port}/json/list");
        Ok(reqwest::get(&url).await?.json().await?)
    }

    /// Connect to the first target of type `"page"` reported by [`list_targets`](Self::list_targets).
    /// Returns [`CdpError::NoTarget`] if none exists.
    pub async fn connect_to_page(host: &str, port: u16) -> Result<Self> {
        let targets = Self::list_targets(host, port).await?;
        let page = targets
            .into_iter()
            .find(|t| t.target_type == "page")
            .ok_or(CdpError::NoTarget)?;
        let ws_url = page
            .web_socket_debugger_url
            .ok_or_else(|| CdpError::InvalidUrl("target has no debugger URL".into()))?;
        Self::connect(&ws_url).await
    }

    /// Open a new tab, optionally navigating it to `url` immediately. Uses `PUT
    /// /json/new` since modern Chrome rejects `GET` for this endpoint.
    pub async fn create_tab(host: &str, port: u16, url: Option<&str>) -> Result<Target> {
        // Chrome requires PUT for /json/new (GET returns 405 in modern versions)
        let endpoint = match url {
            Some(u) => format!("http://{host}:{port}/json/new?{u}"),
            None => format!("http://{host}:{port}/json/new"),
        };
        Ok(reqwest::Client::new()
            .put(&endpoint)
            .send()
            .await?
            .json()
            .await?)
    }
}
