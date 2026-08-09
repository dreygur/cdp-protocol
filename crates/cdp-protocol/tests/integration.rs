//! Integration tests that drive a real browser over CDP.
//!
//! They are `#[ignore]` by default so `cargo test` stays hermetic. Run them
//! against a Chrome started with `--remote-debugging-port=9222`:
//!
//! ```text
//! cargo test -p cdp-driver -- --ignored
//! ```
//!
//! CI launches headless Chrome first; see `.github/workflows/integration.yml`.
//! Override the endpoint with `CDP_HOST` / `CDP_PORT`.

use cdp_driver::{BrowserAction, BrowserAgent, CdpClient, CdpError};
use serde::Deserialize;
use serde_json::json;

/// CDP's own code for a method the browser does not recognise. Chrome sends it
/// for any unknown method, so it is stable enough to assert on.
const METHOD_NOT_FOUND: i64 = -32601;

/// A host no DNS will ever resolve, per RFC 6761's reserved `.invalid` TLD.
const UNRESOLVABLE_URL: &str = "http://nonexistent.invalid.tld.example";

fn host() -> String {
    std::env::var("CDP_HOST").unwrap_or_else(|_| "localhost".to_string())
}

fn port() -> u16 {
    std::env::var("CDP_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(9222)
}

#[tokio::test]
#[ignore = "requires a running Chrome on the debugging port"]
async fn navigate_eval_and_query() {
    let client = CdpClient::connect_to_page(&host(), port())
        .await
        .expect("connect to page target");
    for domain in ["Page", "Runtime", "DOM"] {
        client.enable_domain(domain).await.expect("enable domain");
    }

    client
        .navigate_and_wait("data:text/html,<title>hi</title><h1>ok</h1>", 10_000)
        .await
        .expect("navigate");

    let title = client.eval("document.title").await.expect("eval title");
    assert_eq!(title, "hi");

    let doc = client.get_document().await.expect("get document");
    let found = client
        .query_selector(doc.node_id, "h1")
        .await
        .expect("query_selector");
    assert!(found.is_some(), "expected to find the <h1>");

    let missing = client
        .query_selector(doc.node_id, "does-not-exist")
        .await
        .expect("query_selector");
    assert!(missing.is_none(), "expected None for a missing selector");
}

#[tokio::test]
#[ignore = "requires a running Chrome on the debugging port"]
async fn agent_navigate_and_screenshot() {
    let agent = BrowserAgent::connect(&host(), port())
        .await
        .expect("connect agent");

    let nav = agent
        .execute(BrowserAction::Navigate {
            url: "data:text/html,<h1>x</h1>".to_string(),
        })
        .await;
    assert!(nav.is_success(), "navigate failed: {nav}");

    let shot = agent
        .execute(BrowserAction::Screenshot { path: None })
        .await;
    assert!(shot.is_success(), "screenshot failed: {shot}");
}

/// The shape `Browser.getVersion` answers with, of which we need one field.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserVersionResult {
    product: String,
}

#[tokio::test]
#[ignore = "requires a running Chrome on the debugging port"]
async fn raw_call_reaches_a_domain_with_no_wrapper() {
    // Browser and Target have no typed wrappers in this crate, so this only
    // works through the raw escape hatch. It is the half `raw_call.rs` cannot
    // check: that real Chrome accepts what we put on the wire.
    let client = CdpClient::connect_to_page(&host(), port())
        .await
        .expect("connect to page target");

    let version: BrowserVersionResult = client
        .call("Browser.getVersion", json!({}))
        .await
        .expect("Browser.getVersion");
    assert!(
        version.product.contains('/'),
        "expected a product like Chrome/120.0.0.0, got {:?}",
        version.product
    );

    let targets = client
        .call_raw("Target.getTargets", json!({}))
        .await
        .expect("Target.getTargets");
    assert!(
        targets["targetInfos"].is_array(),
        "expected targetInfos to be an array, got {targets}"
    );
}

#[tokio::test]
#[ignore = "requires a running Chrome on the debugging port"]
async fn raw_call_surfaces_a_real_protocol_error() {
    // Chrome's own rejection of an unknown method, not a mock server's.
    let client = CdpClient::connect_to_page(&host(), port())
        .await
        .expect("connect to page target");

    let failure = client
        .call_raw("Nonsense.command", json!({}))
        .await
        .expect_err("Chrome must reject an unknown method");

    match failure {
        CdpError::Browser { code, message, .. } => {
            assert_eq!(code, METHOD_NOT_FOUND, "Chrome's own code for {message}");
        }
        other => panic!("expected CdpError::Browser, got {other:?}"),
    }
}

#[tokio::test]
#[ignore = "requires a running Chrome on the debugging port"]
async fn a_navigation_to_a_host_that_does_not_resolve_fails() {
    // Chrome answers this with a frame id, a loader id and an errorText, which is
    // the whole reason navigate has to read the result rather than return it.
    let client = CdpClient::connect_to_page(&host(), port())
        .await
        .expect("connect to page target");
    client.enable_domain("Page").await.expect("enable Page");

    let failure = client
        .navigate(UNRESOLVABLE_URL)
        .await
        .expect_err("a page that cannot resolve must not report success");

    assert!(
        failure.to_string().contains("net::ERR_"),
        "expected Chrome's network error text, got {failure}"
    );
}

#[tokio::test]
#[ignore = "requires a running Chrome on the debugging port"]
async fn navigate_and_wait_fails_a_navigation_that_never_loads() {
    let client = CdpClient::connect_to_page(&host(), port())
        .await
        .expect("connect to page target");
    client.enable_domain("Page").await.expect("enable Page");

    let failure = client
        .navigate_and_wait(UNRESOLVABLE_URL, 10_000)
        .await
        .expect_err("a page that cannot resolve must not report success");

    assert!(
        !matches!(failure, CdpError::Timeout),
        "the failure should be Chrome's reason, not the wait expiring: {failure}"
    );
    assert!(
        failure.to_string().contains("net::ERR_"),
        "expected Chrome's network error text, got {failure}"
    );
}

#[tokio::test]
#[ignore = "requires a running Chrome on the debugging port"]
async fn an_agent_navigation_to_a_host_that_does_not_resolve_reports_failure() {
    let agent = BrowserAgent::connect(&host(), port())
        .await
        .expect("connect agent");

    let outcome = agent
        .execute(BrowserAction::Navigate {
            url: UNRESOLVABLE_URL.to_string(),
        })
        .await;

    assert!(
        !outcome.is_success(),
        "a navigation that never loaded must not be a successful action: {outcome}"
    );
}

#[tokio::test]
#[ignore = "requires a running Chrome on the debugging port"]
async fn eval_of_an_expression_that_throws_fails() {
    // The exception shape here is Chrome's, not a mock's: it is what decides
    // whether the message a caller sees is useful.
    let client = CdpClient::connect_to_page(&host(), port())
        .await
        .expect("connect to page target");
    client
        .enable_domain("Runtime")
        .await
        .expect("enable Runtime");

    let failure = client
        .eval("throw new Error('boom')")
        .await
        .expect_err("an expression that threw must not report success");
    assert!(
        failure.to_string().contains("boom"),
        "expected the thrown message, got {failure}"
    );

    let empty = client
        .eval("''")
        .await
        .expect("an expression that produced a value did not throw");
    assert_eq!(empty, "", "an empty result is still a result");
}

/// The shape `Target.createTarget` answers with, of which we need one field.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreatedTarget {
    target_id: String,
}

/// What the tab opened for the session navigates to. Its title is how a later
/// assertion tells the two tabs apart.
const SECOND_TAB: &str = "data:text/html,<title>second</title><h1>second</h1>";

/// What the tab the client connected to shows for the whole test.
const FIRST_TAB: &str = "data:text/html,<title>first</title><h1>first</h1>";

#[tokio::test]
#[ignore = "requires a running Chrome on the debugging port"]
async fn a_session_drives_a_second_tab_and_leaves_the_connected_one_alone() {
    // The point of a session: one socket, two tabs, and commands that land in the
    // tab they were addressed to. Nothing here is reachable through call_raw
    // alone, because the method names are ordinary and the envelope is not.
    let client = CdpClient::connect_to_page(&host(), port())
        .await
        .expect("connect to page target");
    for domain in ["Page", "Runtime"] {
        client.enable_domain(domain).await.expect("enable domain");
    }
    client
        .navigate_and_wait(FIRST_TAB, 10_000)
        .await
        .expect("navigate the connected tab");

    let created: CreatedTarget = client
        .call("Target.createTarget", json!({ "url": "about:blank" }))
        .await
        .expect("Target.createTarget");

    let session = client
        .attach_to_target(&created.target_id)
        .await
        .expect("attach to the second tab");
    for domain in ["Page", "Runtime"] {
        session
            .enable_domain(domain)
            .await
            .expect("enable domain in the session");
    }

    let (loaded, navigated) = tokio::join!(
        session.wait_for_event("Page.loadEventFired", 10_000),
        session.call_raw("Page.navigate", json!({ "url": SECOND_TAB })),
    );
    navigated.expect("navigate inside the session");
    loaded.expect("the second tab should raise its own load event");

    let evaluated = session
        .call_raw(
            "Runtime.evaluate",
            json!({ "expression": "document.title", "returnByValue": true }),
        )
        .await
        .expect("evaluate inside the session");
    assert_eq!(
        evaluated["result"]["value"], "second",
        "the session's commands should have landed in the tab it attached to"
    );

    let untouched = client.eval("document.title").await.expect("eval title");
    assert_eq!(
        untouched, "first",
        "driving a session must not disturb the tab the client connected to"
    );

    session.detach().await.expect("detach from the second tab");

    let _ = client
        .call_raw(
            "Target.closeTarget",
            json!({ "targetId": created.target_id }),
        )
        .await
        .expect("the connection must outlive the session it carried");
}
