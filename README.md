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
```

## Usage

### High-level agent

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

    client.navigate("https://example.com").await?;

    let title = client.eval("document.title").await?;
    println!("{title}");

    client.full_page_screenshot_to_file("screenshots/page.png").await?;

    Ok(())
}
```

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

## Examples

```bash
cargo run --example basic        # low-level CdpClient
cargo run --example agent        # BrowserAgent + ActionBuilder
cargo run --example industrial   # 100 pages in parallel
```

## Project Structure

```
src/
├── lib.rs          # public exports
├── client.rs       # CDP WebSocket client
├── agent.rs        # high-level agent, BrowserAction enum, ActionBuilder
├── config.rs       # Config struct
├── types.rs        # protocol types
└── error.rs        # CdpError

examples/
├── basic.rs        # low-level usage
├── agent.rs        # agent + JSON dispatch + builder
└── industrial.rs   # parallel scraping with JoinSet
```

## Resources

- [CDP Protocol Reference](https://chromedevtools.github.io/devtools-protocol/)
- [Blog: CDP vs WebDriver deep dive](https://dev.to/dreygur/browser-automation-protocols-cdp-vs-webdriver-deep-dive-5bmn)
