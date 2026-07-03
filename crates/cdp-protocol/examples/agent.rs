use cdp_driver::{ActionBuilder, BrowserAction, BrowserAgent, Config, Result};

#[path = "common/logging.rs"]
mod logging;

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = logging::init();

    let cfg = Config::default();
    std::fs::create_dir_all(&cfg.screenshots_dir).ok();

    let agent = BrowserAgent::connect_with_config(&cfg).await?;

    // --- Programmatic actions ---
    println!("=== Programmatic actions ===");

    let r = agent
        .execute(BrowserAction::Navigate {
            url: "https://example.com".to_string(),
        })
        .await;
    println!("Navigate: {}", r);

    let r = agent.execute(BrowserAction::Wait { ms: 1500 }).await;
    println!("Wait: {}", r);

    let r = agent.execute(BrowserAction::GetTitle).await;
    println!("Title: {}", r);

    let r = agent.execute(BrowserAction::GetUrl).await;
    println!("URL: {}", r);

    // --- JSON dispatch (LLM tool calls) ---
    println!("\n=== JSON dispatch ===");

    let r = agent
        .execute_json(r#"{"action":"navigate","url":"https://www.rust-lang.org"}"#)
        .await;
    println!("Navigate: {}", r);

    let r = agent.execute_json(r#"{"action":"wait","ms":2000}"#).await;
    println!("Wait: {}", r);

    let screenshot_path = format!("{}/rust-lang.png", cfg.screenshots_dir);
    let r = agent
        .execute_json(&format!(
            r#"{{"action":"screenshot","path":"{screenshot_path}"}}"#
        ))
        .await;
    println!("Screenshot: {}", r);

    let r = agent.execute_json(r#"{"action":"get_title"}"#).await;
    println!("Title: {}", r);

    let r = agent
        .execute_json(r#"{"action":"evaluate","expression":"navigator.userAgent"}"#)
        .await;
    println!("UserAgent: {}", r);

    // --- ActionBuilder (fluent chaining) ---
    println!("\n=== ActionBuilder ===");

    let search_screenshot = format!("{}/google-search.png", cfg.screenshots_dir);
    let actions = ActionBuilder::new()
        .navigate("https://www.google.com")
        .wait(1500)
        .fill("textarea[name='q'],input[name='q']", "Rust programming")
        .press_key("Enter")
        .wait(2000)
        .screenshot(Some(&search_screenshot))
        .get_title()
        .build();

    let results = agent.execute_many(actions).await;
    for (i, r) in results.iter().enumerate() {
        println!("[{i}] {r}");
    }

    // --- Data extraction ---
    println!("\n=== Data extraction ===");

    agent
        .execute(BrowserAction::Navigate {
            url: "https://example.com".to_string(),
        })
        .await;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    let r = agent
        .execute(BrowserAction::Evaluate {
            expression: r#"
                (() => ({
                    viewport:  { width: window.innerWidth, height: window.innerHeight },
                    userAgent: navigator.userAgent,
                    language:  navigator.language,
                    cookies:   navigator.cookieEnabled,
                    platform:  navigator.platform,
                }))()
            "#
            .to_string(),
        })
        .await;
    println!("Page info: {}", r);

    let r = agent.execute(BrowserAction::GetLinks).await;
    println!("Links: {}", r);

    Ok(())
}
