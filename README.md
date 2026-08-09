# cdp-driver

Chrome DevTools Protocol (CDP) client. WebSocket-based browser automation for AI
agents, web scraping, and testing. Rust core, with first-class Node/Deno/Bun
bindings.

Complete protocol coverage: all 664 commands, 233 events and 608 types are
generated from the published schema as typed structs, with sessions for reaching
other tabs, iframes and workers over one connection.

## Workspace

```
crates/
├── cdp-protocol/        # core Rust crate  → crates.io
└── cdp-protocol-node/   # napi-rs bindings → npm (Node/Deno/Bun)
protocol/                # vendored CDP schemas the generator reads
xtask/                   # the generator
```

- **[crates/cdp-protocol](crates/cdp-protocol)** the engine: `CdpClient`,
  `BrowserAgent`, `Cluster`. See its README for the full Rust API.
- **[crates/cdp-protocol-node](crates/cdp-protocol-node)** native addon exposing
  the same three classes to JavaScript, Promises + TypeScript types.

## Quick start

Start Chrome with remote debugging:

```bash
google-chrome --headless=new --remote-debugging-port=9222 --no-first-run
```

Rust:

```rust
use cdp_driver::CdpClient;
use cdp_driver::protocol::page::{EnableParams, LoadEventFiredEvent, NavigateParams};

let client = CdpClient::connect_to_page("localhost", 9222).await?;
client.send(EnableParams::default()).await?;

client.send(NavigateParams {
    url: "https://example.com".to_string(),
    ..Default::default()
}).await?;
client.wait_for::<LoadEventFiredEvent>(10_000).await?;
```

```bash
cargo run -p cdp-driver --example basic   # or: agent, cluster, industrial, raw
```

Node / Deno / Bun:

```js
import { BrowserAgent } from 'cdp-driver'

const agent = await BrowserAgent.connect('127.0.0.1', 9222)
await agent.navigate('https://example.com')
console.log((await agent.getTitle()).value)
await agent.close()
```

## Resources

- [Blog: CDP vs WebDriver deep dive](https://dev.to/dreygur/browser-automation-protocols-cdp-vs-webdriver-deep-dive-5bmn)
- [CDP Protocol Viewer](https://chromedevtools.github.io/devtools-protocol/)
- [W3C WebDriver Spec](https://www.w3.org/TR/webdriver2/)
- [Puppeteer Docs](https://pptr.dev/)
- [Playwright Docs](https://playwright.dev/)
