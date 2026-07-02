//! Unit tests that need no browser: config, builders, serde, errors.

use cdp_protocol::cluster::ClusterConfig;
use cdp_protocol::{ActionBuilder, BrowserAction, CdpError, Config};
use cdp_protocol::{NavigationResult, Target};

#[test]
fn config_defaults() {
    let c = Config::default();
    assert_eq!(c.host, "localhost");
    assert_eq!(c.port, 9222);
    assert_eq!(c.viewport_width, 1920);
    assert_eq!(c.viewport_height, 1200);
}

#[test]
fn cluster_config_from_config() {
    let cc = ClusterConfig::from(Config::default());
    assert_eq!(cc.concurrency, 5);
    assert_eq!(cc.retries, 2);
    assert_eq!(cc.port, 9222);
    assert!(!cc.monitor);
}

#[test]
fn action_builder_chains_in_order() {
    let actions = ActionBuilder::new()
        .navigate("https://example.com")
        .wait(500)
        .click("#go")
        .get_title()
        .build();

    assert_eq!(actions.len(), 4);
    // First and last variants are what we chained.
    assert!(matches!(actions[0], BrowserAction::Navigate { .. }));
    assert!(matches!(actions[3], BrowserAction::GetTitle));
}

#[test]
fn browser_action_json_roundtrip() {
    let action = BrowserAction::Fill {
        selector: "#q".into(),
        value: "hi".into(),
    };
    let s = serde_json::to_string(&action).unwrap();
    let back: BrowserAction = serde_json::from_str(&s).unwrap();
    assert!(matches!(back, BrowserAction::Fill { selector, value } if selector == "#q" && value == "hi"));
}

#[test]
fn target_deserializes_cdp_shape() {
    let json = r#"{
        "id": "abc",
        "type": "page",
        "title": "Example",
        "url": "https://example.com",
        "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/page/abc"
    }"#;
    let t: Target = serde_json::from_str(json).unwrap();
    assert_eq!(t.target_type, "page");
    assert!(t.web_socket_debugger_url.is_some());
}

#[test]
fn navigation_result_serialize_roundtrip() {
    let nav = NavigationResult { frame_id: "F1".into(), loader_id: None };
    let v = serde_json::to_value(&nav).unwrap();
    assert_eq!(v["frameId"], "F1");
    let back: NavigationResult = serde_json::from_value(v).unwrap();
    assert_eq!(back.frame_id, "F1");
}

#[test]
fn error_display_is_stable() {
    assert_eq!(CdpError::Timeout.to_string(), "Operation timed out");
    assert_eq!(CdpError::NoTarget.to_string(), "No page target available");
    assert_eq!(
        CdpError::Protocol("boom".into()).to_string(),
        "Protocol error: boom"
    );
}
