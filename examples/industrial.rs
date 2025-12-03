//! Industrial scraping - parallel page processing
//!
//! Opens multiple tabs concurrently, navigates, and captures screenshots.

use cdp_protocol::{CdpClient, Result};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;

const NUM_PAGES: usize = 50; // Number of concurrent pages
const MAX_CONCURRENT: usize = 10; // Max concurrent connections

// Sites to scrape (will cycle through these)
const URLS: &[&str] = &[
    "https://www.rust-lang.org",
    "https://www.google.com",
    "https://github.com",
    "https://stackoverflow.com",
    "https://news.ycombinator.com",
    "https://www.wikipedia.org",
    "https://www.reddit.com",
    "https://docs.rs",
    "https://crates.io",
    "https://www.mozilla.org",
];

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    println!("=== Industrial Scraping Demo ===");
    println!("Pages to process: {}", NUM_PAGES);
    println!("Max concurrent: {}", MAX_CONCURRENT);

    // Create output directory
    std::fs::create_dir_all("screenshots").ok();

    let start = Instant::now();

    // Semaphore to limit concurrency
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));

    // Spawn all tasks
    let mut handles = Vec::with_capacity(NUM_PAGES);

    for i in 0..NUM_PAGES {
        let url = URLS[i % URLS.len()].to_string();
        let sem = semaphore.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();
            process_page(i, &url).await
        });

        handles.push(handle);
    }

    // Collect results
    let mut success = 0;
    let mut failed = 0;

    for (i, handle) in handles.into_iter().enumerate() {
        match handle.await {
            Ok(Ok((title, elapsed))) => {
                println!("[{:3}] ✓ {} ({:.1}s)", i, title, elapsed);
                success += 1;
            }
            Ok(Err(e)) => {
                println!("[{:3}] ✗ Error: {}", i, e);
                failed += 1;
            }
            Err(e) => {
                println!("[{:3}] ✗ Task panic: {}", i, e);
                failed += 1;
            }
        }
    }

    let total_time = start.elapsed();

    println!("\n=== Results ===");
    println!("Total time: {:.2}s", total_time.as_secs_f64());
    println!("Success: {}", success);
    println!("Failed: {}", failed);
    println!("Avg per page: {:.2}s", total_time.as_secs_f64() / NUM_PAGES as f64);
    println!("Pages/second: {:.2}", NUM_PAGES as f64 / total_time.as_secs_f64());
    println!("\nScreenshots saved to ./screenshots/");

    Ok(())
}

async fn process_page(id: usize, url: &str) -> Result<(String, f64)> {
    let start = Instant::now();

    // Create new tab
    let target = CdpClient::create_tab("localhost", 9222, Some(url)).await?;

    let ws_url = target
        .web_socket_debugger_url
        .ok_or_else(|| cdp_protocol::CdpError::InvalidUrl("No WS URL".into()))?;

    // Connect to the new tab
    let client = CdpClient::connect(&ws_url).await?;

    // Enable domains
    client.enable_domain("Page").await?;
    client.enable_domain("Runtime").await?;

    // Wait for page load
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    // Get title
    let title: String = client.eval("document.title").await.unwrap_or_else(|_| "Unknown".to_string());

    // Take screenshot
    let screenshot_path = format!("screenshots/page_{:03}.png", id);
    client.screenshot_to_file(&screenshot_path).await?;

    // Close the tab (send Target.closeTarget via browser endpoint)
    // For now we just let it be - tabs will close when Chrome restarts

    let elapsed = start.elapsed().as_secs_f64();
    Ok((title, elapsed))
}
