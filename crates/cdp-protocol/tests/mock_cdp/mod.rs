//! A mock CDP endpoint: one WebSocket server that speaks the JSON envelope
//! Chrome speaks, and answers whatever a test tells it to answer.
//!
//! It exists so the transport in [`cdp_driver::CdpClient`] can be tested without a
//! browser. The client cannot tell it apart from Chrome; it only ever sees
//! `{"id","method","params"}` going out and `{"id","result"}` or `{"id","error"}`
//! coming back.

use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

/// Bind only to loopback; these servers are for one test process, never a network.
const LOOPBACK: &str = "127.0.0.1";

/// Port 0 asks the OS for any free port, so tests never collide on a fixed one.
const ANY_PORT: u16 = 0;

/// CDP's own code for a method the browser does not recognise.
pub const METHOD_NOT_FOUND: i64 = -32601;

/// One command as it arrived from the client, already unwrapped from its envelope.
#[derive(Debug, Clone)]
pub struct Command {
    pub id: u64,
    pub method: String,
    pub params: Value,
}

/// What the server should do about a [`Command`].
pub enum Reply {
    /// Answer with `{"id", "result": <value>}`.
    Result(Value),
    /// Answer with `{"id", "error": {"code", "message"}}`.
    Error { code: i64, message: String },
    /// Answer with nothing at all, leaving the client to time out.
    Silence,
}

/// The test's decision procedure: given a command, produce a reply.
type Responder = Arc<dyn Fn(&Command) -> Reply + Send + Sync>;

/// A running mock endpoint. Dropping it stops the server.
pub struct MockCdp {
    /// Hand this to `CdpClient::connect`.
    pub ws_url: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for MockCdp {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Unwrap one incoming frame. Returns `None` for frames that are not commands,
/// which the client should never send but which we refuse to panic over.
fn parse_command(text: &str) -> Option<Command> {
    let value: Value = serde_json::from_str(text).ok()?;
    Some(Command {
        id: value.get("id")?.as_u64()?,
        method: value.get("method")?.as_str()?.to_string(),
        params: value.get("params").cloned().unwrap_or(Value::Null),
    })
}

/// Render a reply into the frame Chrome would send, or `None` for [`Reply::Silence`].
fn render_reply(id: u64, reply: Reply) -> Option<String> {
    let envelope = match reply {
        Reply::Result(result) => json!({ "id": id, "result": result }),
        Reply::Error { code, message } => {
            json!({ "id": id, "error": { "code": code, "message": message } })
        }
        Reply::Silence => return None,
    };
    Some(envelope.to_string())
}

/// Serve a single accepted socket until the client goes away.
async fn serve_connection(stream: tokio::net::TcpStream, respond: Responder) {
    let Ok(ws) = accept_async(stream).await else {
        return;
    };
    let (mut sink, mut source) = ws.split();

    while let Some(Ok(message)) = source.next().await {
        let Message::Text(text) = message else {
            continue;
        };
        let Some(command) = parse_command(&text) else {
            continue;
        };
        if let Some(frame) = render_reply(command.id, respond(&command)) {
            if sink.send(Message::Text(frame.into())).await.is_err() {
                return;
            }
        }
    }
}

/// Accept connections forever, giving each one the same responder.
async fn serve_forever(listener: TcpListener, respond: Responder) {
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(serve_connection(stream, respond.clone()));
    }
}

/// Start a mock endpoint that answers every command via `respond`.
///
/// The listener is bound before returning, so the returned `ws_url` is ready to
/// connect to with no sleeping or retrying.
pub async fn start<F>(respond: F) -> MockCdp
where
    F: Fn(&Command) -> Reply + Send + Sync + 'static,
{
    let listener = TcpListener::bind((LOOPBACK, ANY_PORT))
        .await
        .expect("bind a loopback port");
    let port = listener.local_addr().expect("read the bound port").port();
    let task = tokio::spawn(serve_forever(listener, Arc::new(respond)));

    MockCdp {
        ws_url: format!("ws://{LOOPBACK}:{port}"),
        task,
    }
}
