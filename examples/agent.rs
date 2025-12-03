//! AI Agent browser control example
//!
//! This shows how an AI agent can control the browser using high-level actions.
//! Actions can be specified as JSON, making it easy to integrate with LLMs.
//!
//! Run Chrome with: google-chrome --remote-debugging-port=9222
//! Then: cargo run --example agent

use cdp_protocol::{ActionBuilder, ActionResult, BrowserAction, BrowserAgent, Result};
use serde_json::json;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    println!("=== AI Agent Browser Control Example ===\n");

    // Connect agent to browser
    println!("Connecting to browser...");
    let agent = BrowserAgent::connect("localhost", 9222).await?;
    println!("Connected!\n");

    // Example 1: Execute actions programmatically
    println!("--- Example 1: Programmatic Actions ---");

    let result = agent
        .execute(BrowserAction::Navigate {
            url: "https://example.com".to_string(),
        })
        .await;
    print_result("Navigate", &result);

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let result = agent.execute(BrowserAction::GetTitle).await;
    print_result("GetTitle", &result);

    let result = agent.execute(BrowserAction::GetUrl).await;
    print_result("GetUrl", &result);

    let result = agent.execute(BrowserAction::GetText).await;
    if let ActionResult::Success { data: Some(d), .. } = &result {
        let text = d["text"].as_str().unwrap_or("");
        println!("  Text preview: {}...", &text[..text.len().min(100)]);
    }

    let result = agent.execute(BrowserAction::GetLinks).await;
    print_result("GetLinks", &result);

    // Example 2: Execute actions from JSON (AI-friendly)
    println!("\n--- Example 2: JSON Actions (AI Integration) ---");

    let json_actions = vec![
        r#"{"action": "navigate", "url": "https://www.rust-lang.org"}"#,
        r#"{"action": "wait", "ms": 2000}"#,
        r#"{"action": "get_title"}"#,
        r#"{"action": "screenshot", "path": "rust_lang.png"}"#,
        r#"{"action": "evaluate", "expression": "document.querySelectorAll('a').length"}"#,
    ];

    for json in json_actions {
        println!("\nAction JSON: {}", json);
        let result = agent.execute_json(json).await;
        print_result("Result", &result);
    }

    // Example 3: Action chaining with builder
    println!("\n--- Example 3: Action Builder Chain ---");

    let actions = ActionBuilder::new()
        .navigate("https://www.google.com")
        .wait(1500)
        .screenshot(Some("google.png"))
        .build();

    println!("Executing {} actions...", actions.len());
    let results = agent.execute_many(actions).await;
    for (i, result) in results.iter().enumerate() {
        print_result(&format!("Action {}", i + 1), result);
    }

    // Example 4: Search interaction
    println!("\n--- Example 4: Search Interaction ---");

    let search_actions = vec![
        BrowserAction::Navigate {
            url: "https://duckduckgo.com".to_string(),
        },
        BrowserAction::Wait { ms: 1500 },
        BrowserAction::Fill {
            selector: "input[name='q']".to_string(),
            value: "Rust programming language".to_string(),
        },
        BrowserAction::PressKey {
            key: "Enter".to_string(),
        },
        BrowserAction::Wait { ms: 2000 },
        BrowserAction::Screenshot {
            path: Some("search_results.png".to_string()),
        },
        BrowserAction::GetTitle,
    ];

    for action in search_actions {
        let result = agent.execute(action).await;
        if !result.is_success() {
            println!("Action failed: {:?}", result);
            break;
        }
    }
    println!("Search completed! Check search_results.png");

    // Example 5: Element inspection
    println!("\n--- Example 5: Element Inspection ---");

    let result = agent
        .execute(BrowserAction::Exists {
            selector: "body".to_string(),
        })
        .await;
    print_result("Body exists", &result);

    let result = agent
        .execute(BrowserAction::GetAttributes {
            selector: "html".to_string(),
        })
        .await;
    print_result("HTML attributes", &result);

    // Example 6: Custom JavaScript
    println!("\n--- Example 6: Custom JavaScript ---");

    let result = agent
        .execute(BrowserAction::Evaluate {
            expression: r#"
                (() => {
                    const info = {
                        viewport: {
                            width: window.innerWidth,
                            height: window.innerHeight
                        },
                        userAgent: navigator.userAgent,
                        language: navigator.language,
                        cookiesEnabled: navigator.cookieEnabled,
                        platform: navigator.platform
                    };
                    return info;
                })()
            "#
            .to_string(),
        })
        .await;
    print_result("Browser info", &result);

    println!("\n=== Demo Complete ===");
    println!("\nGenerated files:");
    println!("  - rust_lang.png");
    println!("  - google.png");
    println!("  - search_results.png");

    Ok(())
}

fn print_result(name: &str, result: &ActionResult) {
    match result {
        ActionResult::Success { data, message } => {
            print!("  ✓ {}", name);
            if let Some(msg) = message {
                print!(": {}", msg);
            }
            if let Some(d) = data {
                let json = serde_json::to_string(d).unwrap_or_default();
                if json.len() < 100 {
                    print!(" -> {}", json);
                }
            }
            println!();
        }
        ActionResult::Error { message } => {
            println!("  ✗ {}: {}", name, message);
        }
    }
}
