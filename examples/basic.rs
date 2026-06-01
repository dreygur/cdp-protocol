use cdp_protocol::{CdpClient, Config, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = Config::default();
    std::fs::create_dir_all(&cfg.screenshots_dir).ok();

    // Discovery
    let version = CdpClient::get_version(&cfg.host, cfg.port).await?;
    println!("Browser: {}", version.browser);
    println!("Protocol: {}", version.protocol_version);

    let targets = CdpClient::list_targets(&cfg.host, cfg.port).await?;
    println!("\nTargets ({}):", targets.len());
    for target in &targets {
        println!("  - {} [{}]: {}", target.target_type, target.id, target.title);
    }

    // Connect to first page target
    let client = CdpClient::connect_to_page(&cfg.host, cfg.port).await?;

    // Enable domains
    client.enable_domain("Page").await?;
    client.enable_domain("Runtime").await?;
    client.enable_domain("DOM").await?;
    client.enable_domain("Network").await?;

    // Set viewport
    client.set_viewport(cfg.viewport_width, cfg.viewport_height, false).await?;

    // Navigate
    let nav = client.navigate("https://example.com").await?;
    println!("\nNavigated — frameId: {}", nav.frame_id);

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // JavaScript evaluation
    let title: String = client.eval("document.title").await?;
    println!("Title: {}", title);

    let result = client.evaluate("1 + 2 * 3").await?;
    println!("Math: {:?}", result.result.value);

    let complex = client
        .evaluate("(() => ({ width: window.innerWidth, height: window.innerHeight }))()")
        .await?;
    println!("Viewport: {:?}", complex.result.value);

    // DOM operations
    let doc = client.get_document().await?;
    println!("\nRoot node: {} (children: {:?})", doc.node_name, doc.child_node_count);

    let h1_id = client.query_selector(doc.node_id, "h1").await?;
    if h1_id > 0 {
        let html = client.get_outer_html(h1_id).await?;
        println!("H1: {}", html);
    }

    // Screenshot
    let path = format!("{}/example.png", cfg.screenshots_dir);
    client.full_page_screenshot_to_file(&path).await?;
    println!("\nScreenshot saved: {path}");

    // Cookies
    let cookies = client.get_cookies().await?;
    println!("Cookies: {}", cookies.len());

    Ok(())
}
