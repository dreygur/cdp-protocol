use cdp_protocol::{CdpClient, CdpError, Config, Result};

#[path = "common/logging.rs"]
mod logging;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

const MAX_CONCURRENT: usize = 5;

const URLS: &[&str] = &[
    // --- replaced errored sites, slishee first ---
    "https://slishee.com",
    "https://www.ebay.com",    // was amazon (bot-blocked)
    "https://bsky.app",        // was x.com (bot-blocked)
    "https://arstechnica.com", // was techcrunch (bot-blocked)
    "https://sendgrid.com",    // was twilio (0-width)
    "https://www.sqlite.org",  // was postgresql (0-width)
    "https://mariadb.org",     // was mysql (0-width)
    "https://opensearch.org",  // was elastic (0-width)
    "https://helm.sh",         // was kubernetes (0-width)
    "https://podman.io",       // was docker (0-width)
    "https://www.kernel.org",  // was linuxfoundation (0-width)
    // --- confirmed working ---
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
    "https://www.apple.com",
    "https://www.microsoft.com",
    "https://www.netflix.com",
    "https://www.linkedin.com",
    "https://www.youtube.com",
    "https://www.instagram.com",
    "https://www.facebook.com",
    "https://www.nytimes.com",
    "https://www.bbc.com",
    "https://www.cnn.com",
    "https://www.theguardian.com",
    "https://www.wired.com",
    "https://medium.com",
    "https://dev.to",
    "https://hashnode.com",
    "https://lobste.rs",
    "https://www.npmjs.com",
    "https://pypi.org",
    "https://hub.docker.com",
    "https://www.cloudflare.com",
    "https://www.digitalocean.com",
    "https://vercel.com",
    "https://www.netlify.com",
    "https://stripe.com",
    "https://www.mongodb.com",
    "https://redis.io",
    "https://prometheus.io",
    "https://grafana.com",
    "https://www.gnome.org",
    // --- new 50 ---
    "https://www.kde.org",
    "https://neovim.io",
    "https://code.visualstudio.com",
    "https://www.jetbrains.com",
    "https://www.python.org",
    "https://go.dev",
    "https://www.scala-lang.org",
    "https://www.haskell.org",
    "https://elixir-lang.org",
    "https://www.php.net",
    "https://www.ruby-lang.org",
    "https://nodejs.org",
    "https://deno.com",
    "https://bun.sh",
    "https://svelte.dev",
    "https://vuejs.org",
    "https://react.dev",
    "https://angular.dev",
    "https://nextjs.org",
    "https://astro.build",
    "https://tailwindcss.com",
    "https://supabase.com",
    "https://neon.tech",
    "https://www.nginx.com",
    "https://traefik.io",
    "https://about.gitlab.com",
    "https://bitbucket.org",
    "https://circleci.com",
    "https://www.jenkins.io",
    "https://www.ansible.com",
    "https://aws.amazon.com",
    "https://azure.microsoft.com",
    "https://cloud.google.com",
    "https://www.ibm.com",
    "https://www.oracle.com",
    "https://www.salesforce.com",
    "https://www.shopify.com",
    "https://www.dropbox.com",
    "https://slack.com",
    "https://zoom.us",
    "https://www.notion.so",
    "https://www.figma.com",
    "https://linear.app",
    "https://www.postman.com",
    "https://insomnia.rest",
    "https://about.sourcegraph.com",
    "https://www.sonarsource.com",
    "https://www.vim.org",
    "https://www.gitkraken.com",
    "https://planetscale.com",
];

const NUM_PAGES: usize = URLS.len();

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = logging::init();

    let cfg = Arc::new(Config::default());
    std::fs::create_dir_all(&cfg.screenshots_dir).ok();

    println!("=== Industrial Scraping Demo ===");
    println!("Pages to process: {NUM_PAGES}");
    println!("Max concurrent:   {MAX_CONCURRENT}\n");

    let start = Instant::now();
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));
    let mut set = JoinSet::new();

    for (i, &url) in URLS.iter().enumerate() {
        let url = url.to_string();
        let sem = semaphore.clone();
        let cfg = cfg.clone();

        set.spawn(async move {
            let _permit = sem.acquire().await.expect("semaphore closed");
            (i, process_page(i, &url, &cfg).await)
        });
    }

    let mut success = 0usize;
    let mut failed = 0usize;

    while let Some(res) = set.join_next().await {
        match res {
            Ok((i, Ok((title, elapsed)))) => {
                println!("[{i:3}] ✓ {title} ({elapsed:.1}s)");
                success += 1;
            }
            Ok((i, Err(e))) => {
                println!("[{i:3}] ✗ Error: {e}");
                failed += 1;
            }
            Err(e) => {
                println!("[???] ✗ Panic: {e}");
                failed += 1;
            }
        }
    }

    let total = start.elapsed();
    println!(
        "\nTotal: {:.2}s | Success: {} | Failed: {} | {:.2} pages/sec",
        total.as_secs_f64(),
        success,
        failed,
        NUM_PAGES as f64 / total.as_secs_f64()
    );

    Ok(())
}

async fn process_page(id: usize, url: &str, cfg: &Config) -> Result<(String, f64)> {
    let start = Instant::now();

    let target = CdpClient::create_tab(&cfg.host, cfg.port, None).await?;
    let ws_url = target
        .web_socket_debugger_url
        .ok_or_else(|| CdpError::InvalidUrl(format!("no WS URL for tab {id}")))?;

    let client = CdpClient::connect(&ws_url).await?;
    client.enable_domain("Page").await?;
    client.enable_domain("Runtime").await?;

    client
        .set_viewport(cfg.viewport_width, cfg.viewport_height, false)
        .await?;
    client.navigate_and_wait(url, 15_000).await?;

    let title = client
        .eval("document.title")
        .await
        .unwrap_or_else(|_| "Unknown".into());

    let path = format!("{}/page_{id:03}.png", cfg.screenshots_dir);
    client.full_page_screenshot_to_file(&path).await?;

    client.close().await?;

    Ok((title, start.elapsed().as_secs_f64()))
}
