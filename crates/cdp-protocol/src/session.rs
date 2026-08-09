//! Flat CDP sessions: addressing commands and attributing events to a target
//! other than the one the connection was opened to.
//!
//! Attaching to a target yields a session id. A command carrying that id beside
//! its `id` runs in that target, and an event raised there comes back carrying it
//! too. Everything rides the one WebSocket [`CdpClient`] already owns, so an
//! out-of-process iframe, a service worker, or a second tab is reachable without
//! a second connection.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::broadcast;

use crate::client::CdpClient;
use crate::error::{CdpError, Result};
use crate::methods::target::{ATTACH_TO_TARGET, DETACH_FROM_TARGET};

/// Ask for sessions multiplexed onto the connection we already have. The other
/// arrangement, tunnelling each frame through `Target.sendMessageToTarget`, is
/// deprecated in CDP and is deliberately not implemented here.
const FLAT_SESSIONS: bool = true;

/// One CDP event, and which target raised it.
#[derive(Debug, Clone)]
pub struct SessionEvent {
    /// The session the event came from, or `None` for the target the client is
    /// connected to directly.
    pub session_id: Option<String>,
    /// The CDP event name, e.g. `"Page.loadEventFired"`.
    pub method: String,
    /// The event's parameters, or [`Value::Null`] for an event that carries none.
    pub params: Value,
}

/// What `Target.attachToTarget` answers with.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Attached {
    session_id: String,
}

/// A handle for driving one attached target over the client's connection.
///
/// Borrowed from the client on purpose: a session cannot outlive the connection
/// it is multiplexed onto. To hold one past that borrow, keep the session id and
/// rebuild the handle with [`CdpClient::session`].
pub struct CdpSession<'a> {
    client: &'a CdpClient,
    session_id: String,
}

impl<'a> CdpSession<'a> {
    /// The session id CDP assigned when the target was attached.
    pub fn id(&self) -> &str {
        &self.session_id
    }

    /// Send any CDP command in this session and return its raw `result` object.
    ///
    /// The counterpart of [`CdpClient::call_raw`], differing only in that the
    /// command is addressed to this session rather than to the connected target.
    pub async fn call_raw(&self, method: &str, params: Value) -> Result<Value> {
        self.client
            .send_command_on(Some(&self.session_id), method, params)
            .await
    }

    /// Send any CDP command in this session and deserialize its result into `R`.
    ///
    /// ```no_run
    /// # use cdp_driver::CdpClient;
    /// # use serde_json::{json, Value};
    /// # async fn run(client: &CdpClient) -> cdp_driver::Result<()> {
    /// let session = client.attach_to_target("A1B2C3").await?;
    /// let _: Value = session
    ///     .call("Page.navigate", json!({ "url": "https://example.com" }))
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

    /// Enable a CDP domain in this session, required before that target's events
    /// for the domain start flowing.
    pub async fn enable_domain(&self, domain: &str) -> Result<()> {
        self.call_raw(&format!("{domain}.enable"), json!({}))
            .await?;
        Ok(())
    }

    /// Wait for the next event named `method` raised by this session, up to
    /// `timeout_ms` milliseconds. Events from other sessions and from the connected
    /// target are ignored.
    pub async fn wait_for_event(&self, method: &str, timeout_ms: u64) -> Result<Value> {
        let mut rx = self.client.subscribe_session_events();
        let session_id = self.session_id.clone();
        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), async move {
            loop {
                match rx.recv().await {
                    Ok(event)
                        if event.method == method
                            && event.session_id.as_deref() == Some(session_id.as_str()) =>
                    {
                        return Ok(event.params)
                    }
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => return Err(CdpError::Protocol("event channel closed".into())),
                }
            }
        })
        .await
        .map_err(|_| CdpError::Timeout)?
    }

    /// Detach from the target, ending this session.
    ///
    /// The connection itself is untouched: the client and any other session on it
    /// keep working. Consumes the handle, since its session id is spent.
    pub async fn detach(self) -> Result<()> {
        self.client
            .call_raw(DETACH_FROM_TARGET, json!({ "sessionId": self.session_id }))
            .await?;
        Ok(())
    }
}

impl CdpClient {
    /// Address commands to a session id obtained elsewhere, e.g. from a
    /// `Target.attachedToTarget` event raised by `setAutoAttach`.
    ///
    /// Nothing is sent, and the id is not checked; commands simply fail with the
    /// browser's own error if no such session exists.
    pub fn session(&self, session_id: impl Into<String>) -> CdpSession<'_> {
        CdpSession {
            client: self,
            session_id: session_id.into(),
        }
    }

    /// Attach to `target_id` and return a handle for driving it.
    ///
    /// The session shares this client's connection, its message ids, and its
    /// command timeout. Find target ids with `Target.getTargets`, or create one
    /// with `Target.createTarget`, through [`call`](CdpClient::call).
    ///
    /// ```no_run
    /// # use cdp_driver::CdpClient;
    /// # use serde_json::{json, Value};
    /// # async fn run(client: &CdpClient) -> cdp_driver::Result<()> {
    /// let session = client.attach_to_target("A1B2C3").await?;
    /// let _: Value = session.call("Runtime.enable", json!({})).await?;
    /// session.detach().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn attach_to_target(&self, target_id: &str) -> Result<CdpSession<'_>> {
        let attached: Attached = self
            .call(
                ATTACH_TO_TARGET,
                json!({ "targetId": target_id, "flatten": FLAT_SESSIONS }),
            )
            .await?;
        Ok(self.session(attached.session_id))
    }
}
