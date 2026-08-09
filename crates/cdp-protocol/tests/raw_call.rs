//! What the client promises about what comes back over the wire, checked against
//! a mock CDP endpoint rather than a browser: `call` and `call_raw`, plus the
//! typed wrappers that have to judge a result rather than just return it.
//!
//! These run in a plain `cargo test`: no Chrome, no network, no fixed ports.
//! The browser-facing half of the same contract lives in `integration.rs`.

mod mock_cdp;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cdp_driver::{BrowserAction, BrowserAgent, CdpClient, CdpError};
use serde::Deserialize;
use serde_json::{json, Value};

use mock_cdp::{Command, Reply, INVALID_PARAMS, METHOD_NOT_FOUND};

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

/// What Chrome 151 answers `Page.navigate` with for a host that does not resolve:
/// a frame id and a loader id, exactly as a successful navigation has, and an
/// `errorText` that is the only sign the page never loaded.
fn a_navigation_chrome_refused() -> Value {
    json!({
        "errorText": "net::ERR_NAME_NOT_RESOLVED",
        "frameId": "46AB4AD75BF7C3B9",
        "isDownload": false,
        "loaderId": "C1861D0F0F0F",
    })
}

/// What Chrome answers when the navigation did start.
fn a_navigation_that_started() -> Value {
    json!({
        "frameId": "46AB4AD75BF7C3B9",
        "isDownload": false,
        "loaderId": "C1861D0F0F0F",
    })
}

/// What `Runtime.evaluate` answers for an expression that threw an `Error`. The
/// `description` carries the message with the stack trace appended to it.
fn an_evaluation_that_threw() -> Value {
    json!({
        "result": {
            "type": "object",
            "subtype": "error",
            "className": "Error",
            "description": "Error: boom\n    at <anonymous>:1:7",
        },
        "exceptionDetails": {
            "exceptionId": 1,
            "text": "Uncaught",
            "lineNumber": 0,
            "columnNumber": 6,
            "exception": {
                "type": "object",
                "subtype": "error",
                "className": "Error",
                "description": "Error: boom\n    at <anonymous>:1:7",
            },
        },
    })
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
async fn a_rejected_command_becomes_a_browser_error_carrying_its_code() {
    // The code is the point: a caller that wants to know "unknown method" should
    // read -32601 rather than pattern-match on English.
    let (client, _server) = client_answering(|_| Reply::Error {
        code: METHOD_NOT_FOUND,
        message: "'Nonsense.command' wasn't found".to_string(),
        data: None,
    })
    .await;

    let failure = client
        .call_raw("Nonsense.command", json!({}))
        .await
        .expect_err("an error reply must not look like success");

    match failure {
        CdpError::Browser {
            code,
            message,
            data,
        } => {
            assert_eq!(code, METHOD_NOT_FOUND);
            assert!(
                message.contains("wasn't found"),
                "the browser's message should survive: {message}"
            );
            assert!(data.is_none(), "no data was sent, so none should appear");
        }
        other => panic!("expected CdpError::Browser, got {other:?}"),
    }
}

#[tokio::test]
async fn a_rejected_command_keeps_the_data_the_browser_attached() {
    let (client, _server) = client_answering(|_| Reply::Error {
        code: INVALID_PARAMS,
        message: "Invalid parameters".to_string(),
        data: Some("url: string value expected".to_string()),
    })
    .await;

    let failure = client
        .call_raw("Page.navigate", json!({ "url": 7 }))
        .await
        .expect_err("bad parameters must not look like success");

    match failure {
        CdpError::Browser { code, data, .. } => {
            assert_eq!(code, INVALID_PARAMS);
            assert_eq!(data.as_deref(), Some("url: string value expected"));
        }
        other => panic!("expected CdpError::Browser, got {other:?}"),
    }
}

#[tokio::test]
async fn a_browser_error_reads_as_one_sentence_with_its_code_and_data() {
    // Display is what reaches logs and the Node bindings, so it is part of the
    // contract, not a debugging convenience.
    let (client, _server) = client_answering(|_| Reply::Error {
        code: INVALID_PARAMS,
        message: "Invalid parameters".to_string(),
        data: Some("url: string value expected".to_string()),
    })
    .await;

    let failure = client
        .call_raw("Page.navigate", json!({}))
        .await
        .expect_err("bad parameters must not look like success");

    assert_eq!(
        failure.to_string(),
        "Browser error -32602: Invalid parameters (url: string value expected)"
    );
}

#[tokio::test]
async fn an_error_frame_with_no_code_is_still_a_browser_error() {
    // Nothing in CDP promises every field; an error frame stripped to its message
    // must still fail the command rather than deserialize into a result.
    let (client, _server) =
        client_answering(|_| Reply::Frame(json!({ "error": { "message": "something broke" } })))
            .await;

    let failure = client
        .call_raw("Page.navigate", json!({}))
        .await
        .expect_err("an error frame must not look like success");

    match failure {
        CdpError::Browser { code, message, .. } => {
            assert_eq!(code, 0, "an absent code should not be invented");
            assert_eq!(message, "something broke");
        }
        other => panic!("expected CdpError::Browser, got {other:?}"),
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

#[tokio::test]
async fn a_navigation_chrome_refused_is_an_error_not_a_frame_id() {
    // Chrome reports a DNS failure inside a result that otherwise looks like a
    // successful navigation, so returning Ok here loses the failure entirely.
    let (client, _server) =
        client_answering(|_| Reply::Result(a_navigation_chrome_refused())).await;

    let failure = client
        .navigate("http://nonexistent.invalid.tld.example")
        .await
        .expect_err("a navigation that never loaded must not report success");

    match failure {
        CdpError::Protocol(message) => assert!(
            message.contains("net::ERR_NAME_NOT_RESOLVED"),
            "the reason Chrome gave should survive: {message}"
        ),
        other => panic!("expected CdpError::Protocol, got {other:?}"),
    }
}

#[tokio::test]
async fn a_navigation_that_started_returns_the_frame_and_loader_ids() {
    let (client, _server) = client_answering(|_| Reply::Result(a_navigation_that_started())).await;

    let navigation = client
        .navigate("https://example.com")
        .await
        .expect("a navigation with no errorText should succeed");

    assert_eq!(navigation.frame_id, "46AB4AD75BF7C3B9");
    assert_eq!(navigation.loader_id.as_deref(), Some("C1861D0F0F0F"));
    assert!(navigation.error_text.is_none());
    assert!(!navigation.is_download);
}

#[tokio::test]
async fn a_navigation_that_became_a_download_says_so() {
    let (client, _server) = client_answering(|_| {
        Reply::Result(json!({ "frameId": "F1", "loaderId": "L1", "isDownload": true }))
    })
    .await;

    let navigation = client
        .navigate("https://example.com/report.pdf")
        .await
        .expect("a download is not a navigation failure");

    assert!(
        navigation.is_download,
        "isDownload must reach the caller, since no page load follows one"
    );
}

#[tokio::test]
async fn navigate_and_wait_fails_a_refused_navigation_without_waiting_for_a_load_event() {
    // A refused navigation fires no load event, so waiting for one would burn the
    // whole timeout before reporting a failure already known at the first reply.
    let (client, _server) =
        client_answering(|_| Reply::Result(a_navigation_chrome_refused())).await;

    let failure = tokio::time::timeout(
        Duration::from_secs(2),
        client.navigate_and_wait("http://nonexistent.invalid.tld.example", 60_000),
    )
    .await
    .expect("the failure should be reported at once, not after the wait")
    .expect_err("a navigation that never loaded must not report success");

    assert!(
        failure.to_string().contains("net::ERR_NAME_NOT_RESOLVED"),
        "the reason Chrome gave should survive: {failure}"
    );
}

#[tokio::test]
async fn an_agent_reports_a_refused_navigation_as_a_failed_action() {
    let (client, _server) =
        client_answering(|_| Reply::Result(a_navigation_chrome_refused())).await;
    let agent = BrowserAgent::from_client(client);

    let outcome = agent
        .execute(BrowserAction::Navigate {
            url: "http://nonexistent.invalid.tld.example".to_string(),
        })
        .await;

    assert!(
        !outcome.is_success(),
        "a navigation that never loaded must not be a successful action: {outcome}"
    );
    assert!(
        outcome
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("net::ERR_NAME_NOT_RESOLVED"),
        "the reason Chrome gave should reach the action result: {outcome}"
    );
}

#[tokio::test]
async fn eval_reports_a_thrown_error_instead_of_an_empty_string() {
    // An expression that throws has no value, and "" is a value an expression can
    // legitimately produce, so silence here is indistinguishable from success.
    let (client, _server) = client_answering(|_| Reply::Result(an_evaluation_that_threw())).await;

    let failure = client
        .eval("throw new Error('boom')")
        .await
        .expect_err("an expression that threw must not report success");

    match failure {
        CdpError::Protocol(message) => {
            assert!(
                message.contains("Error: boom"),
                "the thrown message should survive: {message}"
            );
            assert!(
                !message.contains("at <anonymous>"),
                "the stack trace belongs in evaluate, not in the message: {message}"
            );
        }
        other => panic!("expected CdpError::Protocol, got {other:?}"),
    }
}

#[tokio::test]
async fn eval_reports_a_thrown_value_that_is_not_an_error() {
    // JS can throw anything; a thrown string arrives as a plain value with no
    // description to read the message out of.
    let (client, _server) = client_answering(|_| {
        Reply::Result(json!({
            "result": { "type": "string", "value": "just a string" },
            "exceptionDetails": {
                "exceptionId": 2,
                "text": "Uncaught",
                "exception": { "type": "string", "value": "just a string" },
            },
        }))
    })
    .await;

    let failure = client
        .eval("throw 'just a string'")
        .await
        .expect_err("throwing a string is still throwing");

    assert!(
        failure.to_string().contains("just a string"),
        "the thrown value should survive: {failure}"
    );
}

#[tokio::test]
async fn eval_still_returns_an_empty_string_when_that_is_the_answer() {
    let (client, _server) =
        client_answering(|_| Reply::Result(json!({ "result": { "type": "string", "value": "" } })))
            .await;

    let value = client
        .eval("''")
        .await
        .expect("an expression that produced a value did not throw");

    assert_eq!(value, "");
}

#[tokio::test]
async fn evaluate_still_hands_back_exception_details_rather_than_failing() {
    // eval turns a thrown expression into an error; evaluate is the escape hatch
    // for callers that want the exception as data.
    let (client, _server) = client_answering(|_| Reply::Result(an_evaluation_that_threw())).await;

    let outcome = client
        .evaluate("throw new Error('boom')")
        .await
        .expect("evaluate reports an exception through its result");

    let details = outcome
        .exception_details
        .expect("exceptionDetails should be preserved");
    assert_eq!(details["text"], "Uncaught");
}

#[tokio::test]
async fn an_agent_action_whose_script_throws_reports_failure() {
    let (client, _server) = client_answering(|_| Reply::Result(an_evaluation_that_threw())).await;
    let agent = BrowserAgent::from_client(client);

    let outcome = agent
        .execute(BrowserAction::Click {
            selector: Some("#go".to_string()),
            x: None,
            y: None,
        })
        .await;

    assert!(
        !outcome.is_success(),
        "a click whose script threw must not be a successful action: {outcome}"
    );
    assert!(
        outcome
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("Error: boom"),
        "the thrown message should reach the action result: {outcome}"
    );
}
