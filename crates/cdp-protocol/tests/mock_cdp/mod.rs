//! A mock CDP endpoint: one WebSocket server that speaks the JSON envelope
//! Chrome speaks, and answers whatever a test tells it to answer.
//!
//! It exists so the transport in [`cdp_driver::CdpClient`] can be tested without a
//! browser. The client cannot tell it apart from Chrome; it only ever sees
//! `{"id","method","params"}` going out and `{"id","result"}` or `{"id","error"}`
//! coming back.

// Every test binary including this module uses only the part of the endpoint its
// own tests need, so items no single binary reaches are expected here.
#![allow(dead_code)]

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

/// CDP's own code for a command whose parameters the browser rejects.
pub const INVALID_PARAMS: i64 = -32602;

/// One command as it arrived from the client, already unwrapped from its envelope.
#[derive(Debug, Clone)]
pub struct Command {
    pub id: u64,
    pub method: String,
    pub params: Value,
    /// The session the command was addressed to, absent when it was addressed to
    /// the target the client connected to.
    pub session_id: Option<String>,
}

/// What the server should do about a [`Command`].
pub enum Reply {
    /// Answer with `{"id", "result": <value>}`.
    Result(Value),
    /// Answer with `{"id", "error": {"code", "message"}}`, carrying `"data"` too
    /// when the test supplies the detail string some CDP errors come with.
    Error {
        code: i64,
        message: String,
        data: Option<String>,
    },
    /// Answer with `{"id"}` merged into a frame the test builds itself, for shapes
    /// the other variants cannot express (an error frame missing its code, say).
    Frame(Value),
    /// Answer with `{"id", "result"}` and then push frames the client never asked
    /// for, as Chrome does when a command's side effect raises events. Each frame
    /// is sent verbatim, so a test decides for itself whether one is tagged with a
    /// `sessionId`.
    ResultThenEvents { result: Value, events: Vec<Value> },
    /// Answer with nothing at all, leaving the client to time out.
    Silence,
    /// Drop the socket without answering, as a browser that exits mid-command does.
    Disconnect,
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
        session_id: value
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_string),
    })
}

/// Address a reply back to the command it answers. Chrome echoes the `sessionId`
/// of a command addressed to a session, so the mock does too: a client that keyed
/// its pending replies by session rather than by id alone would pass tests the
/// browser would fail it on otherwise.
fn answering(command: &Command, body: Value) -> Value {
    let mut frame = body;
    frame["id"] = json!(command.id);
    if let Some(session_id) = &command.session_id {
        frame["sessionId"] = json!(session_id);
    }
    frame
}

/// Render a reply into the frames Chrome would send, in order. An empty list
/// means send nothing, which covers both staying silent and hanging up.
fn render_frames(command: &Command, reply: &Reply) -> Vec<String> {
    let (body, events) = match reply {
        Reply::Result(result) => (json!({ "result": result }), Vec::new()),
        Reply::Error {
            code,
            message,
            data,
        } => {
            let mut error = json!({ "code": code, "message": message });
            if let Some(detail) = data {
                error["data"] = json!(detail);
            }
            (json!({ "error": error }), Vec::new())
        }
        Reply::Frame(frame) => (frame.clone(), Vec::new()),
        Reply::ResultThenEvents { result, events } => (json!({ "result": result }), events.clone()),
        Reply::Silence | Reply::Disconnect => return Vec::new(),
    };

    let mut frames = vec![answering(command, body).to_string()];
    frames.extend(events.iter().map(Value::to_string));
    frames
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
        let reply = respond(&command);
        if matches!(reply, Reply::Disconnect) {
            return;
        }
        for frame in render_frames(&command, &reply) {
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
