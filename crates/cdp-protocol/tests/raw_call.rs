//! What `CdpClient::call` and `CdpClient::call_raw` promise, checked against a
//! mock CDP endpoint rather than a browser.
//!
//! These run in a plain `cargo test`: no Chrome, no network, no fixed ports.
//! The browser-facing half of the same contract lives in `integration.rs`.

mod mock_cdp;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cdp_driver::{CdpClient, CdpError};
use serde::Deserialize;
use serde_json::{json, Value};

use mock_cdp::{Command, Reply, METHOD_NOT_FOUND};

/// Long enough that a loopback round trip is never the reason a test fails.
const PATIENT: Duration = Duration::from_secs(5);

/// Short enough that the timeout test finishes quickly, long enough that a busy
/// machine does not trip it by accident.
const IMPATIENT: Duration = Duration::from_millis(250);

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

/// A client whose endpoint always answers with `result`, and the log of every
/// command that reached it.
async fn client_recording(
    result: Value,
) -> (CdpClient, mock_cdp::MockCdp, Arc<Mutex<Vec<Command>>>) {
    let seen: Arc<Mutex<Vec<Command>>> = Arc::new(Mutex::new(Vec::new()));
    let recorder = seen.clone();
    let (client, server) = client_answering(move |command| {
        recorder
            .lock()
            .expect("record the command")
            .push(command.clone());
        Reply::Result(result.clone())
    })
    .await;
    (client, server, seen)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatedTarget {
    target_id: String,
}

#[tokio::test]
async fn call_deserializes_the_result_object() {
    let (client, _server) =
        client_answering(|_| Reply::Result(json!({ "targetId": "ABC123" }))).await;

    let created: CreatedTarget = client
        .call("Target.createTarget", json!({ "url": "about:blank" }))
        .await
        .expect("call should succeed");

    assert_eq!(created.target_id, "ABC123");
}

#[tokio::test]
async fn call_deserializes_the_result_not_the_envelope() {
    // The reply on the wire is {"id", "result": {...}}. Deserializing the whole
    // envelope into CreatedTarget would fail, so success here is the assertion:
    // `call` unwrapped `result` before handing it to serde.
    let (client, _server) =
        client_answering(|_| Reply::Result(json!({ "targetId": "only-in-result" }))).await;

    let created: CreatedTarget = client
        .call("Target.createTarget", json!({}))
        .await
        .expect("call should unwrap the result field");

    assert_eq!(created.target_id, "only-in-result");
}

#[tokio::test]
async fn call_sends_the_method_and_params_verbatim() {
    let (client, _server, seen) = client_recording(json!({})).await;

    let _: Value = client
        .call(
            "Storage.clearDataForOrigin",
            json!({ "origin": "https://example.com", "storageTypes": "all" }),
        )
        .await
        .expect("call should succeed");

    let log = seen.lock().expect("read the recorded commands");
    assert_eq!(log.len(), 1, "exactly one command should have been sent");
    assert_eq!(log[0].method, "Storage.clearDataForOrigin");
    assert_eq!(log[0].params["origin"], "https://example.com");
    assert_eq!(log[0].params["storageTypes"], "all");
}

#[tokio::test]
async fn concurrent_calls_get_distinct_ids_and_their_own_answers() {
    // Each command is answered with its own id echoed back, so a client that
    // mismatched replies to callers would surface as a wrong value here.
    let (client, _server) = client_answering(|command| {
        Reply::Result(json!({ "seen": command.id, "method": command.method }))
    })
    .await;

    let (first, second) = tokio::join!(
        client.call_raw("Runtime.enable", json!({})),
        client.call_raw("Network.enable", json!({})),
    );
    let first = first.expect("first call should succeed");
    let second = second.expect("second call should succeed");

    assert_eq!(first["method"], "Runtime.enable");
    assert_eq!(second["method"], "Network.enable");
    assert_ne!(
        first["seen"], second["seen"],
        "concurrent commands must not share an id"
    );
}

#[tokio::test]
async fn call_raw_returns_the_result_untouched() {
    let payload = json!({ "nested": { "list": [1, 2, 3] }, "flag": true });
    let expected = payload.clone();
    let (client, _server) = client_answering(move |_| Reply::Result(payload.clone())).await;

    let got = client
        .call_raw("Anything.works", json!({}))
        .await
        .expect("call_raw should succeed");

    assert_eq!(got, expected);
}

#[tokio::test]
async fn a_command_with_no_result_yields_null() {
    // Chrome answers commands like Page.enable with {"id":N,"result":{}}, but a
    // bare {"id":N} is legal too. The client turns the absent field into Null
    // rather than erroring.
    let (client, _server) = client_answering(|_| Reply::Result(Value::Null)).await;

    let got = client
        .call_raw("Page.enable", json!({}))
        .await
        .expect("a result-less command should still succeed");

    assert_eq!(got, Value::Null);
}

#[tokio::test]
async fn a_protocol_error_becomes_cdp_error_protocol() {
    let (client, _server) = client_answering(|_| Reply::Error {
        code: METHOD_NOT_FOUND,
        message: "'Nonsense.command' wasn't found".to_string(),
    })
    .await;

    let failure = client
        .call_raw("Nonsense.command", json!({}))
        .await
        .expect_err("an error reply must not look like success");

    match failure {
        CdpError::Protocol(message) => {
            assert!(
                message.contains("wasn't found"),
                "the browser's message should survive: {message}"
            );
        }
        other => panic!("expected CdpError::Protocol, got {other:?}"),
    }
}

#[tokio::test]
async fn a_result_of_the_wrong_shape_becomes_cdp_error_json() {
    let (client, _server) = client_answering(|_| Reply::Result(json!({ "unexpected": 1 }))).await;

    let failure = client
        .call::<_, CreatedTarget>("Target.createTarget", json!({}))
        .await
        .expect_err("a result missing targetId cannot deserialize");

    assert!(
        matches!(failure, CdpError::Json(_)),
        "expected CdpError::Json, got {failure:?}"
    );
}

#[tokio::test]
async fn an_unanswered_command_times_out() {
    let (client, _server) = client_answering(|_| Reply::Silence).await;
    client.set_command_timeout(IMPATIENT);

    let failure = client
        .call_raw("Page.navigate", json!({ "url": "about:blank" }))
        .await
        .expect_err("a command the browser never answers must time out");

    assert!(
        matches!(failure, CdpError::Timeout),
        "expected CdpError::Timeout, got {failure:?}"
    );
}

#[tokio::test]
async fn a_client_recovers_after_one_command_times_out() {
    // A timed-out command drops its pending entry. The next command must still
    // find its own reply rather than inheriting the abandoned one.
    let (client, _server) = client_answering(|command| match command.method.as_str() {
        "Slow.command" => Reply::Silence,
        _ => Reply::Result(json!({ "ok": true })),
    })
    .await;

    client.set_command_timeout(IMPATIENT);
    let timed_out = client.call_raw("Slow.command", json!({})).await;
    assert!(matches!(timed_out, Err(CdpError::Timeout)));

    client.set_command_timeout(PATIENT);
    let after = client
        .call_raw("Fast.command", json!({}))
        .await
        .expect("the client should still be usable");

    assert_eq!(after["ok"], true);
}

#[tokio::test]
async fn a_generated_constant_carries_the_method_name_to_the_wire() {
    use cdp_driver::methods::{storage, target};

    // The constants are only useful if they hold exactly the string CDP expects,
    // so check the value that actually reaches the server, not the constant.
    let (client, _server, seen) = client_recording(json!({ "targetId": "from-a-constant" })).await;

    let created: CreatedTarget = client
        .call(target::CREATE_TARGET, json!({ "url": "about:blank" }))
        .await
        .expect("a call keyed by a generated constant should succeed");
    assert_eq!(created.target_id, "from-a-constant");

    let _: Value = client
        .call(storage::CLEAR_DATA_FOR_ORIGIN, json!({ "origin": "x" }))
        .await
        .expect("a second domain should work the same way");

    let log = seen.lock().expect("read the recorded commands");
    assert_eq!(log[0].method, "Target.createTarget");
    assert_eq!(log[1].method, "Storage.clearDataForOrigin");
}

#[tokio::test]
async fn a_command_in_flight_when_the_socket_drops_fails_at_once() {
    // Without this the caller would wait out the whole timeout for a reply that
    // can never arrive, so the client fails everything still pending instead.
    let (client, _server) = client_answering(|_| Reply::Disconnect).await;
    client.set_command_timeout(PATIENT);

    let failure = tokio::time::timeout(
        Duration::from_secs(2),
        client.call_raw("Page.navigate", json!({ "url": "about:blank" })),
    )
    .await
    .expect("the call must resolve well inside the command timeout")
    .expect_err("a dropped socket cannot produce a result");

    match failure {
        CdpError::Protocol(message) => assert!(
            message.contains("connection closed"),
            "expected a connection-closed message, got {message}"
        ),
        other => panic!("expected CdpError::Protocol, got {other:?}"),
    }
}
