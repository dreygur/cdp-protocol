# cdp-driver

Chrome DevTools Protocol (CDP) client in Rust. WebSocket-based browser automation for AI agents, web scraping, and testing.

Covers the whole protocol: all 664 commands, 233 events and 608 types are
generated from the published schema, alongside a small hand-written API for the
things you reach for constantly.

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
cdp-driver = "0.4"

# optional: synchronous blocking API
cdp-driver = { version = "0.4", features = ["blocking"] }
```

## Usage

### Async (default)

```rust
use cdp_driver::{BrowserAgent, BrowserAction, Config};

#[tokio::main]
async fn main() -> cdp_driver::Result<()> {
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

No async runtime needed - each client owns its own tokio runtime internally.

```rust
use cdp_driver::blocking::BrowserAgent;
use cdp_driver::{BrowserAction, Config};

fn main() -> cdp_driver::Result<()> {
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
use cdp_driver::ActionBuilder;

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
use cdp_driver::{CdpClient, Config};

#[tokio::main]
async fn main() -> cdp_driver::Result<()> {
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

### Typed commands and events

Every command is a struct that knows its own method name and result type, so the
three cannot disagree. Generated from the protocol schema into
`cdp_driver::protocol`, one module per domain.

```rust
use cdp_driver::protocol::page::{EnableParams, LoadEventFiredEvent, NavigateParams};
use cdp_driver::protocol::runtime::EvaluateParams;

client.send(EnableParams::default()).await?;

let navigated = client.send(NavigateParams {
    url: "https://example.com".to_string(),
    ..Default::default()
}).await?;
assert!(navigated.error_text.is_none());

let loaded = client.wait_for::<LoadEventFiredEvent>(10_000).await?;
println!("loaded at {}", loaded.timestamp);

let evaluated = client.send(EvaluateParams {
    expression: "document.title".to_string(),
    return_by_value: Some(true),
    ..Default::default()
}).await?;
```

Schema enums keep an `Unrecognized(String)` variant, because Chrome ships values
ahead of the published protocol and a payload should not fail to decode just
because it is newer than this crate.

### Any command by name

`call` and `call_raw` send anything, for exploring a response or reaching a
command with no typed struct to hand. `cdp_driver::methods` holds a constant per
command so a typo is a compile error rather than a runtime one.

```rust
use cdp_driver::methods::target;
use cdp_driver::protocol::target::CreateTargetReturns;
use serde_json::json;

// deserialized into a type you name
let created: CreateTargetReturns = client
    .call(target::CREATE_TARGET, json!({ "url": "about:blank" }))
    .await?;

// or raw, to index into while you are still learning a response
let targets = client.call_raw(target::GET_TARGETS, json!({})).await?;
```

### Sessions

Attaching reaches a target other than the one you connected to (another tab, an
out-of-process iframe, a worker) over the same socket, using CDP's flat session
mode.

```rust
let session = client.attach_to_target(&target_id).await?;

session.send(EnableParams::default()).await?;
session.send(NavigateParams {
    url: "https://example.com".to_string(),
    ..Default::default()
}).await?;

session.detach().await?;
```

`subscribe_session_events()` yields `SessionEvent { session_id, method, params }`,
where `session_id` is `None` for the target you connected to directly. The older
`subscribe_events()` keeps its exact meaning and only ever sees those untagged
events, so an attached tab's activity cannot wake a waiter watching another tab.

### Errors

`CdpError::Browser { code, message, data }` is what Chrome rejected, carrying
CDP's own numeric code (`-32601` for an unknown method, `-32602` for bad
parameters). Branch on `code` rather than on the wording of `message`.
`CdpError::Protocol(String)` is reserved for what this crate itself could not
make sense of.

Two calls report failure rather than hiding it: `navigate` fails when Chrome
answers with an `errorText` such as `net::ERR_NAME_NOT_RESOLVED`, and `eval`
fails when the expression throws. Use `evaluate` when you want an exception back
as data instead of as an error.

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

Pre-creates a pool of browser tabs and distributes tasks across them with retries. Workers are reused between tasks - no create/close overhead per task.

```rust
use cdp_driver::cluster::{Cluster, ClusterConfig};
use cdp_driver::Config;

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
RUST_LOG=cdp_driver=debug cargo run --example basic

# synchronous log output (easier to correlate with code flow)
RUST_LOG=cdp_driver=debug RUST_LOG_SYNC=1 cargo run --example basic

# everything including tokio/reqwest internals
RUST_LOG=debug cargo run --example basic
```

## Examples

```bash
cargo run -p cdp-driver --example basic        # low-level CdpClient
cargo run -p cdp-driver --example agent        # BrowserAgent + ActionBuilder
cargo run -p cdp-driver --example industrial   # 100 pages in parallel with JoinSet
cargo run -p cdp-driver --example cluster      # worker pool with retries
cargo run -p cdp-driver --example raw          # call/call_raw into unwrapped domains
```

## Node / Deno / Bun

The same engine ships as an npm package via [napi-rs](https://napi.rs) bindings
in [`crates/cdp-protocol-node`](../cdp-protocol-node). Native Rust does the CDP
work; JS gets Promises and TypeScript types. Works in Node, Bun, and Deno
(`npm:` specifier).

```js
import { BrowserAgent, Cluster } from 'cdp-driver'

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
├── lib.rs            # public exports
├── client.rs         # the session: socket, command channel, frame routing, events
├── session.rs        # CdpSession, attaching to other targets
├── typed.rs          # Command and Event traits, send/wait_for/decode
├── protocol/         # generated: 664 commands, 233 events, 608 types
├── methods/          # generated: a name constant per command
├── discovery.rs      # Chrome's HTTP /json endpoints
├── page.rs           # navigation, emulation, PDF, geolocation
├── network.rs        # cookies, headers, interception
├── dom.rs            # document tree
├── runtime.rs        # JavaScript evaluation
├── screenshot.rs     # PNG capture
├── agent.rs          # BrowserAgent: runs actions, reports outcomes
├── action.rs         # BrowserAction, the action vocabulary
├── action_parse.rs   # BrowserAction from LLM tool-call JSON
├── action_builder.rs # ActionBuilder
├── keys.rs           # keyboard names for Input.dispatchKeyEvent
├── cluster.rs        # worker pool (Cluster, ClusterConfig)
├── blocking.rs       # synchronous wrappers (feature = "blocking")
├── config.rs         # Config struct
├── types.rs          # hand-written types for the curated API
└── error.rs          # CdpError

examples/
├── basic.rs        # low-level usage
├── agent.rs        # agent + JSON dispatch + builder
├── industrial.rs   # parallel scraping with JoinSet
├── cluster.rs      # worker pool with retries
├── raw.rs          # call/call_raw into domains with no wrapper
└── common/
    └── logging.rs  # shared tracing init
```

Generated code comes from the schemas vendored in `protocol/` at the repository
root. `just generate-methods` regenerates it; `just update-protocol` refreshes
the schemas from upstream first.

## Resources

- [Blog: CDP vs WebDriver deep dive](https://dev.to/dreygur/browser-automation-protocols-cdp-vs-webdriver-deep-dive-5bmn)
- [CDP Protocol Viewer](https://chromedevtools.github.io/devtools-protocol/)
- [W3C WebDriver Spec](https://www.w3.org/TR/webdriver2/)
- [Puppeteer Docs](https://pptr.dev/)
- [Playwright Docs](https://playwright.dev/)
- [Selenium BiDi](https://www.selenium.dev/documentation/webdriver/bidi/)
