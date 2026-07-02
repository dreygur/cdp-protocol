# cdp-protocol

Chrome DevTools Protocol (CDP) client in Rust. WebSocket-based browser automation for AI agents, web scraping, and testing.

## Quick Start

Start Chrome with remote debugging:

```bash
google-chrome --remote-debugging-port=9222
# or headless
google-chrome --remote-debugging-port=9222 --headless=new
```

Add to `Cargo.toml`:

```toml
[dependencies]
cdp-protocol = { git = "https://github.com/dreygur/cdp-protocol" }

# optional: synchronous blocking API
cdp-protocol = { git = "https://github.com/dreygur/cdp-protocol", features = ["blocking"] }
```

## Usage

### Async (default)

```rust
use cdp_protocol::{BrowserAgent, BrowserAction, Config};

#[tokio::main]
async fn main() -> cdp_protocol::Result<()> {
    let cfg = Config::default();
    std::fs::create_dir_all(&cfg.screenshots_dir).ok();

    let agent = BrowserAgent::connect_with_config(&cfg).await?;

    agent.execute(BrowserAction::Navigate {
        url: "https://example.com".to_string(),
    }).await;

    agent.execute(BrowserAction::Screenshot {
        path: Some("screenshots/example.png".to_string()),
    }).await;

    Ok(())
}
```

### Blocking (feature = "blocking")

No async runtime needed — each client owns its own tokio runtime internally.

```rust
use cdp_protocol::blocking::BrowserAgent;
use cdp_protocol::{BrowserAction, Config};

fn main() -> cdp_protocol::Result<()> {
    let cfg = Config::default();
    std::fs::create_dir_all(&cfg.screenshots_dir).ok();

    let agent = BrowserAgent::connect_with_config(&cfg)?;

    agent.execute(BrowserAction::Navigate {
        url: "https://example.com".to_string(),
    });

    agent.execute(BrowserAction::Screenshot {
        path: Some("screenshots/example.png".to_string()),
    });

    Ok(())
}
```

### JSON actions (LLM tool calls)

```rust
agent.execute_json(r#"{"action": "navigate", "url": "https://example.com"}"#).await;
agent.execute_json(r#"{"action": "click", "selector": "button.submit"}"#).await;
agent.execute_json(r#"{"action": "fill", "selector": "#email", "value": "user@example.com"}"#).await;
agent.execute_json(r#"{"action": "screenshot", "path": "screenshots/result.png"}"#).await;
```

### Action builder

```rust
use cdp_protocol::ActionBuilder;

let actions = ActionBuilder::new()
    .navigate("https://example.com")
    .wait(1500)
    .fill("input[name='q']", "rust programming")
    .press_key("Enter")
    .wait(2000)
    .screenshot(Some("screenshots/result.png"))
    .build();

let results = agent.execute_many(actions).await;
```

### Low-level client

```rust
use cdp_protocol::{CdpClient, Config};

#[tokio::main]
async fn main() -> cdp_protocol::Result<()> {
    let cfg = Config::default();

    let client = CdpClient::connect_to_page(&cfg.host, cfg.port).await?;
    client.enable_domain("Page").await?;
    client.enable_domain("Runtime").await?;
    client.set_viewport(cfg.viewport_width, cfg.viewport_height, false).await?;

    client.navigate_and_wait("https://example.com", 10_000).await?;

    let title = client.eval("document.title").await?;
    println!("{title}");

    client.full_page_screenshot_to_file("screenshots/page.png").await?;

    Ok(())
}
```

### Events

```rust
let mut rx = client.subscribe_events();

client.navigate("https://example.com").await?;

while let Ok((method, params)) = rx.recv().await {
    println!("{method}: {params}");
}

// wait for a specific event with timeout
let metrics = client.wait_for_event("Performance.metrics", 5_000).await?;
```

### Console capture

```rust
let mut console = agent.capture_console();

agent.execute(BrowserAction::Navigate {
    url: "https://example.com".to_string(),
}).await;

while let Ok(msg) = console.recv().await {
    println!("[{}] {}", msg.level, msg.text);
}
```

### Network interception

```rust
client.enable_domain("Network").await?;
client.intercept_requests(&["*.api.example.com/*"]).await?;

let mut rx = client.subscribe_events();
client.navigate_and_wait("https://example.com", 10_000).await?;

while let Ok((method, params)) = rx.recv().await {
    if method == "Fetch.requestPaused" {
        let request_id = params["requestId"].as_str().unwrap();
        client.continue_request(request_id).await?;
    }
}
```

### PDF export

```rust
client.navigate_and_wait("https://example.com", 10_000).await?;
client.print_to_pdf("output.pdf").await?;
```

### Emulation

```rust
client.set_user_agent("Mozilla/5.0 (compatible; MyBot/1.0)").await?;
client.set_geolocation(37.7749, -122.4194, 10.0).await?;
client.set_offline(true).await?;
```

### Cluster (puppeteer-cluster style)

Pre-creates a pool of browser tabs and distributes tasks across them with retries. Workers are reused between tasks — no create/close overhead per task.

```rust
use cdp_protocol::cluster::{Cluster, ClusterConfig};
use cdp_protocol::Config;

let cluster = Cluster::new(ClusterConfig {
    concurrency: 5,
    retries: 2,
    monitor: true,
    ..ClusterConfig::from(Config::default())
}).await?;

let results = cluster.run(urls, |client, url| async move {
    client.navigate_and_wait(&url, 15_000).await?;
    let title = client.eval("document.title").await?;
    client.full_page_screenshot_to_file(&format!("screenshots/{}.png", url)).await?;
    Ok(title)
}).await;

cluster.close().await;
```

`ClusterConfig` fields:

| Field | Default | Description |
|-------|---------|-------------|
| `concurrency` | `5` | number of worker tabs |
| `retries` | `2` | retries per task before failure |
| `monitor` | `false` | print per-task timing |

## Config

`Config::default()` sets:

| Field | Default |
|-------|---------|
| `host` | `localhost` |
| `port` | `9222` |
| `viewport_width` | `1920` |
| `viewport_height` | `1200` |
| `screenshots_dir` | `screenshots` |

## Actions

| Action | Parameters |
|--------|-----------|
| `navigate` | `url` |
| `go_back` / `go_forward` / `reload` | - |
| `click` | `selector` or `x, y` |
| `type` | `text`, `selector?` |
| `fill` | `selector`, `value` |
| `submit` | `selector?` |
| `press_key` | `key` |
| `get_title` / `get_url` / `get_text` | - |
| `get_content` | `selector?` |
| `get_links` / `get_attributes` / `exists` | `selector` |
| `screenshot` | `path?` |
| `evaluate` | `expression` |
| `wait` | `ms` |
| `wait_for_selector` | `selector`, `timeout_ms?` |
| `scroll` | `x`, `y` |
| `set_viewport` | `width`, `height`, `mobile?` |
| `get_metrics` | - |

## Debug Logging

```bash
# debug CDP send/recv/events
RUST_LOG=cdp_protocol=debug cargo run --example basic

# synchronous log output (easier to correlate with code flow)
RUST_LOG=cdp_protocol=debug RUST_LOG_SYNC=1 cargo run --example basic

# everything including tokio/reqwest internals
RUST_LOG=debug cargo run --example basic
```

## Examples

```bash
cargo run -p cdp-protocol --example basic        # low-level CdpClient
cargo run -p cdp-protocol --example agent        # BrowserAgent + ActionBuilder
cargo run -p cdp-protocol --example industrial   # 100 pages in parallel with JoinSet
cargo run -p cdp-protocol --example cluster      # worker pool with retries
```

## Node / Deno / Bun

The same engine ships as an npm package via [napi-rs](https://napi.rs) bindings
in [`crates/cdp-protocol-node`](../cdp-protocol-node). Native Rust does the CDP
work; JS gets Promises and TypeScript types. Works in Node, Bun, and Deno
(`npm:` specifier).

```js
import { BrowserAgent, Cluster } from 'cdp-protocol'

const agent = await BrowserAgent.connect('127.0.0.1', 9222)
await agent.navigate('https://example.com')
console.log((await agent.getTitle()).value)
await agent.close()
```

`CdpClient` (low-level), `BrowserAgent` (actions), and `Cluster` (worker pool)
are exposed to JS. `CdpClient` mirrors the Rust client/network/page surface;
`Cluster` is a purpose-built batch pool (not a port of the generic Rust
`Cluster`). See [`crates/cdp-protocol-node`](../cdp-protocol-node/README.md).

## Project Structure

```
src/
├── lib.rs          # public exports
├── client.rs       # CDP WebSocket client, event system
├── agent.rs        # high-level agent, BrowserAction enum, ActionBuilder
├── cluster.rs      # worker pool (Cluster, ClusterConfig)
├── blocking.rs     # synchronous wrappers (feature = "blocking")
├── network.rs      # network methods (cookies, headers, interception)
├── page.rs         # page/emulation/DOM methods (PDF, user agent, geolocation)
├── config.rs       # Config struct
├── types.rs        # protocol types
└── error.rs        # CdpError

examples/
├── basic.rs        # low-level usage
├── agent.rs        # agent + JSON dispatch + builder
├── industrial.rs   # parallel scraping with JoinSet
├── cluster.rs      # worker pool with retries
└── common/
    └── logging.rs  # shared tracing init
```

## Resources

- [Blog: CDP vs WebDriver deep dive](https://dev.to/dreygur/browser-automation-protocols-cdp-vs-webdriver-deep-dive-5bmn)
- [CDP Protocol Viewer](https://chromedevtools.github.io/devtools-protocol/)
- [W3C WebDriver Spec](https://www.w3.org/TR/webdriver2/)
- [Puppeteer Docs](https://pptr.dev/)
- [Playwright Docs](https://playwright.dev/)
- [Selenium BiDi](https://www.selenium.dev/documentation/webdriver/bidi/)
