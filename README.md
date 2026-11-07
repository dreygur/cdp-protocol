<p align="center">
  <img src="assets/banner.svg" alt="Rust CDP client: Chrome DevTools Protocol, typed end to end" width="880">
</p>

<h1 align="center">Rust CDP client</h1>

<p align="center">
  <a href="https://crates.io/crates/cdp-driver"><img src="https://img.shields.io/crates/v/cdp-driver.svg?logo=rust" alt="crates.io"></a>
  <a href="https://docs.rs/cdp-driver"><img src="https://img.shields.io/docsrs/cdp-driver?logo=docs.rs" alt="docs.rs"></a>
  <a href="https://www.npmjs.com/package/cdp-driver"><img src="https://img.shields.io/npm/v/cdp-driver.svg?logo=npm" alt="npm"></a>
  <a href="https://github.com/dreygur/cdp-protocol/actions/workflows/ci.yml"><img src="https://github.com/dreygur/cdp-protocol/actions/workflows/ci.yml/badge.svg" alt="ci"></a>
  <img src="https://img.shields.io/badge/MSRV-1.88-blue?logo=rust" alt="MSRV 1.88">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green.svg" alt="MIT license"></a>
</p>

Chrome DevTools Protocol (CDP) client for Rust, published as `cdp-driver`.
WebSocket browser automation for AI agents, web scraping and testing, with
Node/Deno/Bun bindings over the same core.

All 677 commands, 232 events and 611 types are generated from the published
schema as typed structs. Sessions reach other tabs, iframes and workers over one
connection.

```toml
[dependencies]
cdp-driver = "0.4"
```

## How it talks to Chrome

One WebSocket per client. Every command gets an id; the read loop hands each
reply to whoever is waiting on that id and fans events out to every subscriber.
Commands for an attached tab carry its `sessionId`, so one socket drives many
targets.

```mermaid
flowchart LR
    subgraph app["your code"]
        A["BrowserAgent"]
        K["Cluster"]
        C["CdpClient"]
        S["CdpSession"]
        A --> C
        K --> C
        S --> C
    end
    C == "one WebSocket" ==> B
    subgraph chrome["Chrome"]
        B(("browser"))
        B --- T1["connected tab"]
        B --- T2["second tab"]
        B --- T3["iframe / worker"]
    end
```

A command and the event it triggers, as they move through the client:

```mermaid
sequenceDiagram
    participant Code as your code
    participant Client as CdpClient
    participant Read as read loop
    participant Chrome

    Code->>Client: send(NavigateParams)
    Note over Client: pending[7] = oneshot
    Client->>Chrome: { id: 7, method: "Page.navigate", params }
    Chrome-->>Read: { id: 7, result }
    Read-->>Client: resolve pending[7]
    Client-->>Code: NavigateReturns
    Chrome-->>Read: { method: "Page.loadEventFired" }
    Read-->>Code: broadcast to wait_for::<LoadEventFiredEvent>

    Code->>Client: session.call_raw(..)
    Client->>Chrome: { id: 8, sessionId: "AB12", method, params }
```

`Cluster` is a pool of clients that runs your async tasks across several pages
at once.

## Where the types come from

Nothing in `src/protocol/` or `src/methods/` is written by hand. The generator
reads Chrome's own schema, so a new Chrome release is one command away.

```mermaid
flowchart LR
    U["ChromeDevTools/<br/>devtools-protocol"] -- "just update-protocol" --> J["protocol/<br/>browser_protocol.json<br/>js_protocol.json"]
    J --> X["xtask<br/>(the generator)"]
    X --> T["src/protocol/<br/>typed params, returns, events"]
    X --> M["src/methods/<br/>method name constants"]
    T --> D["client.send(NavigateParams { .. })"]
    M --> K["client.call(methods::page::NAVIGATE, ..)"]
```

## Workspace

```
crates/
├── cdp-protocol/        # core Rust crate  → crates.io (cdp-driver)
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
