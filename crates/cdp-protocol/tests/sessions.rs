//! What a flat CDP session promises on the wire, checked against a mock endpoint
//! rather than a browser: which commands carry a `sessionId`, and which target an
//! event is attributed to.
//!
//! These run in a plain `cargo test`: no Chrome, no network, no fixed ports. The
//! browser-facing half of the same contract lives in `integration.rs`.

mod mock_cdp;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cdp_driver::{CdpClient, CdpError};
use serde_json::{json, Value};

use mock_cdp::{Command, Reply};

/// Long enough that a loopback round trip is never the reason a test fails.
const PATIENT: Duration = Duration::from_secs(5);

/// Short enough that a test asserting nothing arrives finishes quickly, long
/// enough that a busy machine does not trip it by accident.
const IMPATIENT: Duration = Duration::from_millis(250);

/// The session id the mock endpoint hands out for an attach, and tags its events
/// with. Chrome's are 32 hex characters; any opaque string exercises the same code.
const A_SESSION: &str = "5E5510N0000000000000000000000001";

/// A second session, so a test can tell one apart from the other.
const ANOTHER_SESSION: &str = "5E5510N0000000000000000000000002";

/// A client wired to a mock endpoint that answers every command the same way.
///
/// The `MockCdp` is returned alongside the client because dropping it stops the
/// server; a test that discards it would be talking to nothing.
async fn client_answering<F>(respond: F) -> (CdpClient, mock_cdp::MockCdp)
where
    F: Fn(&Command) -> Reply + Send + Sync + 'static,
{
    let server = mock_cdp::start(respond).await;
    let client = CdpClient::connect(&server.ws_url)
        .await
        .expect("connect to the mock endpoint");
    client.set_command_timeout(PATIENT);
    (client, server)
}

/// A client whose endpoint answers every attach with `session_id` and everything
/// else with an empty result, and the log of every command that reached it.
async fn client_attaching(
    session_id: &'static str,
) -> (CdpClient, mock_cdp::MockCdp, Arc<Mutex<Vec<Command>>>) {
    let seen: Arc<Mutex<Vec<Command>>> = Arc::new(Mutex::new(Vec::new()));
    let recorder = seen.clone();
    let (client, server) = client_answering(move |command| {
        recorder
            .lock()
            .expect("record the command")
            .push(command.clone());
        match command.method.as_str() {
            "Target.attachToTarget" => Reply::Result(json!({ "sessionId": session_id })),
            _ => Reply::Result(json!({})),
        }
    })
    .await;
    (client, server, seen)
}

/// An endpoint that answers `Page.enable` by raising `frames` as unsolicited
/// events, the way Chrome raises a target's events once a domain is enabled.
async fn client_raising(frames: Vec<Value>) -> (CdpClient, mock_cdp::MockCdp) {
    client_answering(move |command| match command.method.as_str() {
        "Page.enable" => Reply::ResultThenEvents {
            result: json!({}),
            events: frames.clone(),
        },
        _ => Reply::Result(json!({})),
    })
    .await
}

/// One event frame as Chrome sends it for an attached target.
fn an_event_from(session_id: &str) -> Value {
    json!({
        "method": "Page.loadEventFired",
        "params": { "timestamp": 1234.5 },
        "sessionId": session_id,
    })
}

/// One event frame as Chrome sends it for the target the client connected to.
fn an_event_from_the_connected_target() -> Value {
    json!({
        "method": "Page.loadEventFired",
        "params": { "timestamp": 6789.0 },
    })
}

#[tokio::test]
async fn attaching_asks_for_a_flat_session_and_keeps_the_id() {
    let (client, _server, seen) = client_attaching(A_SESSION).await;

    let session = client
        .attach_to_target("TARGET-1")
        .await
        .expect("attach should succeed");

    assert_eq!(session.id(), A_SESSION);

    let log = seen.lock().expect("read the recorded commands");
    assert_eq!(log[0].method, "Target.attachToTarget");
    assert_eq!(log[0].params["targetId"], "TARGET-1");
    assert_eq!(
        log[0].params["flatten"], true,
        "without flatten the session would have to be tunnelled through the \
         deprecated Target.sendMessageToTarget"
    );
    assert_eq!(
        log[0].session_id, None,
        "the attach itself is a browser-level command, not a session one"
    );
}

#[tokio::test]
async fn a_command_in_a_session_carries_its_session_id_on_the_wire() {
    let (client, _server, seen) = client_attaching(A_SESSION).await;
    let session = client
        .attach_to_target("TARGET-1")
        .await
        .expect("attach should succeed");

    let _ = session
        .call_raw("Page.navigate", json!({ "url": "about:blank" }))
        .await
        .expect("a session command should succeed");

    let log = seen.lock().expect("read the recorded commands");
    let navigate = log.last().expect("the navigate should have arrived");
    assert_eq!(navigate.method, "Page.navigate");
    assert_eq!(navigate.session_id.as_deref(), Some(A_SESSION));
    assert_eq!(
        navigate.params["url"], "about:blank",
        "the session id belongs beside the params, never inside them"
    );
    assert!(
        navigate.params.get("sessionId").is_none(),
        "the session id must not be smuggled into the params"
    );
}

#[tokio::test]
async fn a_command_outside_a_session_carries_no_session_id() {
    let (client, _server, seen) = client_attaching(A_SESSION).await;

    let _ = client
        .call_raw("Page.navigate", json!({ "url": "about:blank" }))
        .await
        .expect("a plain command should succeed");

    let log = seen.lock().expect("read the recorded commands");
    assert_eq!(
        log[0].session_id, None,
        "a client with no session must send the envelope it always sent"
    );
}

#[tokio::test]
async fn an_event_from_a_session_is_attributable_to_it() {
    let (client, _server) = client_raising(vec![an_event_from(A_SESSION)]).await;
    let mut events = client.subscribe_session_events();

    let _ = client
        .call_raw("Page.enable", json!({}))
        .await
        .expect("enable should succeed");

    let event = tokio::time::timeout(PATIENT, events.recv())
        .await
        .expect("the event should arrive well inside the timeout")
        .expect("the event channel should still be open");

    assert_eq!(event.method, "Page.loadEventFired");
    assert_eq!(event.session_id.as_deref(), Some(A_SESSION));
    assert_eq!(event.params["timestamp"], 1234.5);
}

#[tokio::test]
async fn an_event_from_the_connected_target_is_marked_as_belonging_to_no_session() {
    let (client, _server) = client_raising(vec![an_event_from_the_connected_target()]).await;
    let mut events = client.subscribe_session_events();

    let _ = client
        .call_raw("Page.enable", json!({}))
        .await
        .expect("enable should succeed");

    let event = tokio::time::timeout(PATIENT, events.recv())
        .await
        .expect("the event should arrive well inside the timeout")
        .expect("the event channel should still be open");

    assert_eq!(event.session_id, None);
}

#[tokio::test]
async fn an_untagged_event_still_reaches_the_existing_subscriber() {
    // subscribe_events is what agent, page and the node bindings are built on, so
    // an event for the connected target must reach it exactly as it always did.
    let (client, _server) = client_raising(vec![an_event_from_the_connected_target()]).await;
    let mut events = client.subscribe_events();

    let _ = client
        .call_raw("Page.enable", json!({}))
        .await
        .expect("enable should succeed");

    let (method, params) = tokio::time::timeout(PATIENT, events.recv())
        .await
        .expect("the event should arrive well inside the timeout")
        .expect("the event channel should still be open");

    assert_eq!(method, "Page.loadEventFired");
    assert_eq!(params["timestamp"], 6789.0);
}

#[tokio::test]
async fn a_session_event_stays_out_of_the_untagged_subscriber() {
    // A (method, params) pair cannot say which target it came from, so letting an
    // attached tab's load event through would wake waiters like navigate_and_wait
    // for a navigation that never happened in the tab they are watching.
    let (client, _server) = client_raising(vec![an_event_from(A_SESSION)]).await;
    let mut events = client.subscribe_events();

    let _ = client
        .call_raw("Page.enable", json!({}))
        .await
        .expect("enable should succeed");

    let arrived = tokio::time::timeout(IMPATIENT, events.recv()).await;
    assert!(
        arrived.is_err(),
        "a session's event must not be mistaken for the connected target's: {arrived:?}"
    );
}

#[tokio::test]
async fn waiting_on_a_session_ignores_an_event_from_another_target() {
    let (client, _server) = client_raising(vec![an_event_from(ANOTHER_SESSION)]).await;
    let session = client.session(A_SESSION);

    let (waited, _enabled) = tokio::join!(
        session.wait_for_event("Page.loadEventFired", IMPATIENT.as_millis() as u64),
        client.call_raw("Page.enable", json!({})),
    );

    assert!(
        matches!(waited, Err(CdpError::Timeout)),
        "an event from another target is not the one this session waited for: {waited:?}"
    );
}

#[tokio::test]
async fn waiting_on_a_session_returns_its_own_event() {
    let (client, _server) = client_raising(vec![
        an_event_from(ANOTHER_SESSION),
        an_event_from(A_SESSION),
    ])
    .await;
    let session = client.session(A_SESSION);

    let (waited, _enabled) = tokio::join!(
        session.wait_for_event("Page.loadEventFired", 5_000),
        client.call_raw("Page.enable", json!({})),
    );

    let params = waited.expect("the session's own event should end the wait");
    assert_eq!(params["timestamp"], 1234.5);
}

#[tokio::test]
async fn commands_on_two_sessions_each_get_their_own_reply() {
    // Ids stay unique across the whole connection, so a reply is matched by id
    // alone. Echoing the session back is what a client keyed by session would
    // wrongly rely on, and mismatching would surface as a swapped answer here.
    let (client, _server) = client_answering(|command| {
        Reply::Result(json!({
            "session": command.session_id,
            "method": command.method,
        }))
    })
    .await;

    let first = client.session(A_SESSION);
    let second = client.session(ANOTHER_SESSION);
    let (first, second) = tokio::join!(
        first.call_raw("Runtime.evaluate", json!({ "expression": "1" })),
        second.call_raw("Page.getFrameTree", json!({})),
    );

    let first = first.expect("the first session's command should succeed");
    let second = second.expect("the second session's command should succeed");

    assert_eq!(first["session"], A_SESSION);
    assert_eq!(first["method"], "Runtime.evaluate");
    assert_eq!(second["session"], ANOTHER_SESSION);
    assert_eq!(second["method"], "Page.getFrameTree");
}

#[tokio::test]
async fn detaching_ends_the_session_and_leaves_the_connection_working() {
    let (client, _server, seen) = client_attaching(A_SESSION).await;
    let session = client
        .attach_to_target("TARGET-1")
        .await
        .expect("attach should succeed");

    session.detach().await.expect("detach should succeed");

    let after = client
        .call_raw("Browser.getVersion", json!({}))
        .await
        .expect("detaching one session must not end the connection");
    assert_eq!(after, json!({}));

    let log = seen.lock().expect("read the recorded commands");
    let detach = &log[1];
    assert_eq!(detach.method, "Target.detachFromTarget");
    assert_eq!(
        detach.params["sessionId"], A_SESSION,
        "detach names the session in its params, since it is addressed to the browser"
    );
    assert_eq!(
        detach.session_id, None,
        "a command sent into the session it is ending would have nowhere to land"
    );
}
