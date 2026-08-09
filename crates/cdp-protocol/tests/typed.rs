//! What the generated protocol types promise: that a real CDP payload decodes
//! into them, that a value the vendored schema never listed does not break the
//! payload it arrived in, and that a typed command reaches the wire as the
//! method and parameters the protocol declares.
//!
//! These run in a plain `cargo test`: no Chrome, no network, no fixed ports.

mod mock_cdp;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cdp_driver::protocol::dom::{GetDocumentParams, GetDocumentReturns, PseudoType};
use cdp_driver::protocol::page::{self, NavigateParams};
use cdp_driver::protocol::runtime::{ConsoleApiCalledEvent, ConsoleApiCalledType};
use cdp_driver::protocol::target::{CreateTargetParams, CreateTargetReturns};
use cdp_driver::typed::{self, NoReturns};
use cdp_driver::{CdpClient, Command as _, Event as _, SessionEvent};
use serde_json::{json, Value};

use mock_cdp::{Command, Reply};

/// Long enough that a loopback round trip is never the reason a test fails.
const PATIENT: Duration = Duration::from_secs(5);

/// The session id the mock endpoint hands out for an attach.
const A_SESSION: &str = "5E5510N0000000000000000000000001";

/// A `DOM.getDocument` result with a document that contains an element that
/// contains a nested document, which is the shape that needs boxing to exist at
/// all, and a pseudo element carrying an enum the schema does list.
fn a_document() -> Value {
    json!({
        "root": {
            "nodeId": 1,
            "backendNodeId": 11,
            "nodeType": 9,
            "nodeName": "#document",
            "localName": "",
            "nodeValue": "",
            "documentURL": "https://example.com/",
            "childNodeCount": 1,
            "children": [{
                "nodeId": 2,
                "parentId": 1,
                "backendNodeId": 12,
                "nodeType": 1,
                "nodeName": "IFRAME",
                "localName": "iframe",
                "nodeValue": "",
                "attributes": ["src", "/inner.html"],
                "pseudoType": "first-line",
                "contentDocument": {
                    "nodeId": 3,
                    "backendNodeId": 13,
                    "nodeType": 9,
                    "nodeName": "#document",
                    "localName": "",
                    "nodeValue": "",
                    "documentURL": "https://example.com/inner.html"
                }
            }]
        }
    })
}

/// A client wired to a mock endpoint that answers every command the same way,
/// and the log of every command that reached it.
///
/// The `MockCdp` is returned alongside the client because dropping it stops the
/// server; a test that discards it would be talking to nothing.
async fn client_recording(
    result: Value,
) -> (CdpClient, mock_cdp::MockCdp, Arc<Mutex<Vec<Command>>>) {
    let seen: Arc<Mutex<Vec<Command>>> = Arc::new(Mutex::new(Vec::new()));
    let recorder = seen.clone();
    let server = mock_cdp::start(move |command| {
        recorder
            .lock()
            .expect("record the command")
            .push(command.clone());
        Reply::Result(result.clone())
    })
    .await;
    let client = CdpClient::connect(&server.ws_url)
        .await
        .expect("connect to the mock endpoint");
    client.set_command_timeout(PATIENT);
    (client, server, seen)
}

#[test]
fn a_recursive_payload_decodes_all_the_way_down() {
    let document: GetDocumentReturns =
        serde_json::from_value(a_document()).expect("decode a document");

    assert_eq!(document.root.node_name, "#document");
    let children = document.root.children.expect("the document has a child");
    assert_eq!(children.len(), 1);

    let iframe = &children[0];
    assert_eq!(iframe.node_name, "IFRAME");
    assert_eq!(
        iframe.attributes.as_deref(),
        Some(&["src".to_string(), "/inner.html".to_string()][..])
    );

    let inner = iframe
        .content_document
        .as_ref()
        .expect("the iframe has a document");
    assert_eq!(inner.node_id, 3);
    assert_eq!(
        inner.document_url.as_deref(),
        Some("https://example.com/inner.html")
    );
}

#[test]
fn an_optional_the_payload_omits_decodes_as_none() {
    let document: GetDocumentReturns =
        serde_json::from_value(a_document()).expect("decode a document");

    // The schema calls both optional and this payload carries neither.
    assert_eq!(document.root.parent_id, None);
    assert_eq!(document.root.frame_id, None);
}

#[test]
fn an_enum_value_the_schema_lists_decodes_to_its_variant() {
    let document: GetDocumentReturns =
        serde_json::from_value(a_document()).expect("decode a document");
    let children = document.root.children.expect("the document has a child");

    assert_eq!(children[0].pseudo_type, Some(PseudoType::FirstLine));
}

#[test]
fn an_enum_value_the_vendored_schema_never_listed_still_decodes() {
    // Chrome ships new enum values ahead of the published protocol. Decoding
    // has to survive one, or every payload carrying it is lost.
    let event: ConsoleApiCalledEvent = serde_json::from_value(json!({
        "type": "someKindInventedAfterThisSchemaWasVendored",
        "args": [],
        "executionContextId": 1,
        "timestamp": 1234.5
    }))
    .expect("decode an event carrying an unlisted enum value");

    assert_eq!(
        event.r#type,
        ConsoleApiCalledType::Unrecognized(
            "someKindInventedAfterThisSchemaWasVendored".to_string()
        )
    );
    assert_eq!(event.stack_trace, None);
}

#[test]
fn an_unlisted_enum_value_goes_back_out_as_it_arrived() {
    let value = ConsoleApiCalledType::Unrecognized("brandNew".to_string());
    assert_eq!(
        serde_json::to_value(&value).expect("serialize"),
        json!("brandNew")
    );
    assert_eq!(
        serde_json::to_value(ConsoleApiCalledType::Warning).expect("serialize"),
        json!("warning")
    );
}

#[test]
fn an_optional_left_unset_is_not_sent_at_all() {
    // CDP rejects a parameter it never declared, and an explicit null is such a
    // parameter, so an absent optional has to be absent on the wire.
    let params = NavigateParams {
        url: "https://example.com".to_string(),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(params).expect("serialize"),
        json!({ "url": "https://example.com" })
    );
}

#[test]
fn a_command_knows_its_own_method_name() {
    assert_eq!(NavigateParams::METHOD, "Page.navigate");
    assert_eq!(CreateTargetParams::METHOD, "Target.createTarget");
    assert_eq!(ConsoleApiCalledEvent::METHOD, "Runtime.consoleAPICalled");
}

#[test]
fn decoding_an_event_answers_none_for_a_different_event() {
    let params = json!({ "type": "log", "args": [], "executionContextId": 1, "timestamp": 1.0 });

    assert!(typed::decode::<ConsoleApiCalledEvent>("Page.loadEventFired", &params).is_none());
    let decoded = typed::decode::<ConsoleApiCalledEvent>("Runtime.consoleAPICalled", &params)
        .expect("the method matches")
        .expect("the payload fits");
    assert_eq!(decoded.r#type, ConsoleApiCalledType::Log);
}

#[test]
fn a_session_event_decodes_into_its_payload() {
    let event = SessionEvent {
        session_id: Some(A_SESSION.to_string()),
        method: "Page.loadEventFired".to_string(),
        params: json!({ "timestamp": 42.5 }),
    };

    let loaded = event
        .decode::<page::LoadEventFiredEvent>()
        .expect("the method matches")
        .expect("the payload fits");
    assert_eq!(loaded.timestamp, 42.5);
    assert!(event.decode::<ConsoleApiCalledEvent>().is_none());
}

#[tokio::test]
async fn a_typed_command_sends_the_method_the_protocol_declares() {
    let (client, _server, seen) = client_recording(json!({ "targetId": "T1" })).await;

    let created: CreateTargetReturns = client
        .send(CreateTargetParams {
            url: "about:blank".to_string(),
            ..Default::default()
        })
        .await
        .expect("the command succeeds");

    assert_eq!(created.target_id, "T1");
    let sent = seen.lock().expect("read the log");
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].method, "Target.createTarget");
    // Every other parameter is optional and unset, so nothing else goes out.
    assert_eq!(sent[0].params, json!({ "url": "about:blank" }));
}

#[tokio::test]
async fn a_command_the_protocol_gives_no_result_still_succeeds() {
    let (client, _server, seen) = client_recording(json!({})).await;

    let answered: NoReturns = client
        .send(page::EnableParams::default())
        .await
        .expect("the command succeeds");

    assert_eq!(answered, NoReturns::default());
    assert_eq!(seen.lock().expect("read the log")[0].method, "Page.enable");
}

#[tokio::test]
async fn a_typed_command_can_be_addressed_to_a_session() {
    let (client, _server, seen) = client_recording(json!({ "root": a_document()["root"] })).await;

    let session = client.session(A_SESSION);
    let document: GetDocumentReturns = session
        .send(GetDocumentParams {
            depth: Some(1),
            ..Default::default()
        })
        .await
        .expect("the command succeeds");

    assert_eq!(document.root.node_id, 1);
    let sent = seen.lock().expect("read the log");
    assert_eq!(sent[0].method, "DOM.getDocument");
    assert_eq!(sent[0].session_id.as_deref(), Some(A_SESSION));
    assert_eq!(sent[0].params, json!({ "depth": 1 }));
}
