//! Basic CDP client usage example
//!
//! Run Chrome with: google-chrome --remote-debugging-port=9222
//! Then: cargo run --example basic

use cdp_protocol::{CdpClient, Result};
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    println!("=== CDP Protocol Basic Example ===\n");

    // Get browser version
    println!("Fetching browser info...");
    let version = CdpClient::get_version("localhost", 9222).await?;
    println!("Browser: {}", version.browser);
    println!("Protocol: {}", version.protocol_version);

    // List targets
    println!("\nAvailable targets:");
    let targets = CdpClient::list_targets("localhost", 9222).await?;
    for target in &targets {
        println!("  - {} [{}]: {}", target.target_type, target.id, target.title);
    }

    // Connect to first page
    println!("\nConnecting to browser...");
    let client = CdpClient::connect_to_page("localhost", 9222).await?;

    // Enable domains
    client.enable_domain("Page").await?;
    client.enable_domain("Runtime").await?;
    client.enable_domain("DOM").await?;
    println!("Domains enabled");

    // Navigate
    println!("\nNavigating to example.com...");
    let nav = client.navigate("https://example.com").await?;
    println!("Frame ID: {}", nav.frame_id);

    // Wait for load
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Get page title
    let title: String = client.eval("document.title").await?;
    println!("Page title: {}", title);

    // Get URL
    let url: String = client.eval("window.location.href").await?;
    println!("Current URL: {}", url);

    // Execute JavaScript
    println!("\nExecuting JavaScript...");
    let result = client.evaluate("1 + 2 * 3").await?;
    println!("1 + 2 * 3 = {:?}", result.result.value);

    // Get document info
    let doc = client.get_document().await?;
    println!("\nDocument root: {} (nodeId: {})", doc.node_name, doc.node_id);

    // Query selector
    let h1_id = client.query_selector(doc.node_id, "h1").await?;
    if h1_id > 0 {
        let html = client.get_outer_html(h1_id).await?;
        println!("H1 element: {}", html);
    }

    // Take screenshot
    println!("\nCapturing screenshot...");
    client.screenshot_to_file("example_screenshot.png").await?;
    println!("Screenshot saved to example_screenshot.png");

    // Get cookies
    let cookies = client.get_cookies().await?;
    println!("\nCookies: {} found", cookies.len());

    // Get page text
    let text: String = client.eval("document.body.innerText.substring(0, 200)").await?;
    println!("\nPage text preview:\n{}", text);

    println!("\n=== Done ===");
    Ok(())
}
