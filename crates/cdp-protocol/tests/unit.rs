//! Unit tests that need no browser: config, builders, serde, errors.

use cdp_driver::cluster::ClusterConfig;
use cdp_driver::{ActionBuilder, BrowserAction, CdpError, Config};
use cdp_driver::{NavigationResult, Target};

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
    assert!(
        matches!(back, BrowserAction::Fill { selector, value } if selector == "#q" && value == "hi")
    );
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
    let nav = NavigationResult {
        frame_id: "F1".into(),
        loader_id: None,
        error_text: None,
        is_download: false,
    };
    let v = serde_json::to_value(&nav).unwrap();
    assert_eq!(v["frameId"], "F1");
    let back: NavigationResult = serde_json::from_value(v).unwrap();
    assert_eq!(back.frame_id, "F1");
}

#[test]
fn a_navigation_result_keeps_the_error_text_chrome_sent() {
    // The exact frame Chrome answers a DNS failure with. Dropping errorText here
    // is what made a failed navigation look like a successful one.
    let json = r#"{
        "errorText": "net::ERR_NAME_NOT_RESOLVED",
        "frameId": "46AB4AD75BF7C3B9",
        "isDownload": false,
        "loaderId": "C1861D0F0F0F"
    }"#;
    let nav: NavigationResult = serde_json::from_str(json).unwrap();
    assert_eq!(
        nav.error_text.as_deref(),
        Some("net::ERR_NAME_NOT_RESOLVED")
    );
    assert!(!nav.is_download);
}

#[test]
fn a_navigation_result_without_the_download_flag_reads_as_not_a_download() {
    let json = r#"{ "frameId": "F1", "loaderId": "L1" }"#;
    let nav: NavigationResult = serde_json::from_str(json).unwrap();
    assert!(nav.error_text.is_none());
    assert!(!nav.is_download);
}

#[test]
fn error_display_is_stable() {
    assert_eq!(CdpError::Timeout.to_string(), "Operation timed out");
    assert_eq!(CdpError::NoTarget.to_string(), "No page target available");
    assert_eq!(
        CdpError::Protocol("boom".into()).to_string(),
        "Protocol error: boom"
    );
    assert_eq!(
        CdpError::Browser {
            code: -32601,
            message: "'Nonsense.command' wasn't found".into(),
            data: None,
        }
        .to_string(),
        "Browser error -32601: 'Nonsense.command' wasn't found"
    );
    assert_eq!(
        CdpError::Browser {
            code: -32602,
            message: "Invalid parameters".into(),
            data: Some("url: string value expected".into()),
        }
        .to_string(),
        "Browser error -32602: Invalid parameters (url: string value expected)"
    );
}
