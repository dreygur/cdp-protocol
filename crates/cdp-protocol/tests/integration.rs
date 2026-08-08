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

    assert!(
        matches!(failure, CdpError::Protocol(_)),
        "expected CdpError::Protocol, got {failure:?}"
    );
}
