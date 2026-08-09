//! Sending a CDP command as a type, and reading an event back as one.
//!
//! [`CdpClient::call`](crate::CdpClient::call) takes a method name and a
//! `Serialize` beside a result type chosen by the caller, so nothing stops the
//! three from disagreeing. The traits here tie them together: a generated
//! parameters struct in [`crate::protocol`] knows its own method name and its
//! own result type, and an event payload knows the event it belongs to.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::CdpClient;
use crate::error::Result;
use crate::session::{CdpSession, SessionEvent};

/// The result of a command the protocol declares no return fields for.
///
/// Chrome answers such a command with an empty object, and older builds
/// sometimes with nothing at all, so this accepts whatever arrives rather than
/// turning a successful command into a decoding error.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Value")]
pub struct NoReturns {}

/// A CDP command, carrying its own method name and result type.
///
/// Implemented by every `*Params` struct in [`crate::protocol`].
pub trait Command: Serialize {
    /// The protocol's name for the command, e.g. `"Page.navigate"`.
    const METHOD: &'static str;

    /// What the browser answers with, or [`NoReturns`] when it answers with
    /// nothing.
    type Returns: DeserializeOwned;
}

/// A CDP event, carrying the name it arrives under.
///
/// Implemented by every `*Event` struct in [`crate::protocol`].
pub trait Event: DeserializeOwned {
    /// The protocol's name for the event, e.g. `"Page.loadEventFired"`.
    const METHOD: &'static str;
}

impl From<Value> for NoReturns {
    fn from(_: Value) -> Self {
        Self {}
    }
}

/// Decode a raw `(method, params)` pair as `E`.
///
/// `None` means the pair is some other event, which is the common case when
/// walking a subscription; `Some(Err(_))` means it was this event and its
/// payload did not fit.
///
/// ```no_run
/// # use cdp_driver::CdpClient;
/// use cdp_driver::protocol::page::LoadEventFiredEvent;
/// use cdp_driver::typed;
///
/// # async fn run(client: &CdpClient) -> cdp_driver::Result<()> {
/// let mut events = client.subscribe_events();
/// while let Ok((method, params)) = events.recv().await {
///     if let Some(loaded) = typed::decode::<LoadEventFiredEvent>(&method, &params) {
///         println!("loaded at {}", loaded?.timestamp);
///         break;
///     }
/// }
/// # Ok(())
/// # }
/// ```
pub fn decode<E: Event>(method: &str, params: &Value) -> Option<Result<E>> {
    if method != E::METHOD {
        return None;
    }
    Some(E::deserialize(params).map_err(Into::into))
}

impl SessionEvent {
    /// Decode this event as `E`, or `None` when it is a different event.
    ///
    /// Which session raised it is left to the caller to check, since an event
    /// from a session and the same event from the connected target decode
    /// identically.
    pub fn decode<E: Event>(&self) -> Option<Result<E>> {
        decode(&self.method, &self.params)
    }
}

impl CdpSession<'_> {
    /// Send a typed command in this session.
    ///
    /// The counterpart of [`CdpClient::send`], differing only in that the
    /// command is addressed to this session rather than to the connected target.
    pub async fn send<C: Command>(&self, params: C) -> Result<C::Returns> {
        self.call(C::METHOD, params).await
    }

    /// Wait for the next `E` raised by this session, up to `timeout_ms`
    /// milliseconds. Events from other sessions are ignored.
    pub async fn wait_for<E: Event>(&self, timeout_ms: u64) -> Result<E> {
        let params = self.wait_for_event(E::METHOD, timeout_ms).await?;
        Ok(serde_json::from_value(params)?)
    }
}

impl CdpClient {
    /// Send a typed command and get back its declared result.
    ///
    /// The method name and the result type both come from the parameters, so a
    /// command cannot be paired with the wrong result.
    ///
    /// ```no_run
    /// # use cdp_driver::CdpClient;
    /// use cdp_driver::protocol::page::NavigateParams;
    ///
    /// # async fn run(client: &CdpClient) -> cdp_driver::Result<()> {
    /// let navigated = client
    ///     .send(NavigateParams {
    ///         url: "https://example.com".to_string(),
    ///         ..Default::default()
    ///     })
    ///     .await?;
    /// println!("{}", navigated.frame_id);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send<C: Command>(&self, params: C) -> Result<C::Returns> {
        self.call(C::METHOD, params).await
    }

    /// Wait for the next `E` on the connected target, up to `timeout_ms`
    /// milliseconds.
    ///
    /// The event's domain has to be enabled first, as with
    /// [`wait_for_event`](Self::wait_for_event).
    pub async fn wait_for<E: Event>(&self, timeout_ms: u64) -> Result<E> {
        let params = self.wait_for_event(E::METHOD, timeout_ms).await?;
        Ok(serde_json::from_value(params)?)
    }
}
