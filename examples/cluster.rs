use cdp_protocol::{cluster::{Cluster, ClusterConfig}, Config, Result};

#[path = "common/logging.rs"]
mod logging;

const URLS: &[&str] = &[
    "https://slishee.com",
    "https://www.rust-lang.org",
    "https://www.google.com",
    "https://github.com",
    "https://stackoverflow.com",
    "https://news.ycombinator.com",
    "https://www.wikipedia.org",
    "https://docs.rs",
    "https://crates.io",
    "https://www.mozilla.org",
];

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = logging::init();

    let cfg = Config::default();
    std::fs::create_dir_all(&cfg.screenshots_dir).ok();

    let config = ClusterConfig {
        concurrency: 3,
        retries: 1,
        monitor: true,
        ..ClusterConfig::from(cfg.clone())
    };

    println!("Starting cluster ({} workers)...", config.concurrency);
    let cluster = Cluster::new(config).await?;

    let shots_dir = cfg.screenshots_dir.clone();
    let results = cluster.run(URLS.iter().copied().enumerate(), move |client, (i, url)| {
        let shots_dir = shots_dir.clone();
        async move {
            client.navigate_and_wait(url, 15_000).await?;
            let title = client.eval("document.title").await?;
            client.full_page_screenshot_to_file(
                &format!("{shots_dir}/cluster_{i:03}.png")
            ).await?;
            Ok::<_, cdp_protocol::CdpError>(title)
        }
    }).await;

    let success = results.iter().filter(|r| r.is_ok()).count();
    let failed  = results.len() - success;

    for (i, r) in results.iter().enumerate() {
        match &r.result {
            Ok(title) => println!("[{i:2}] {title} ({:.1}s)", r.elapsed.as_secs_f64()),
            Err(e)    => println!("[{i:2}] error: {e}"),
        }
    }

    println!("\nDone. {} ok, {} failed.", success, failed);

    cluster.close().await;
    Ok(())
}
