# cdp-driver

Chrome DevTools Protocol (CDP) client. WebSocket-based browser automation for AI
agents, web scraping, and testing. Rust core, with first-class Node/Deno/Bun
bindings.

## Workspace

```
crates/
├── cdp-protocol/        # core Rust crate  → crates.io
└── cdp-protocol-node/   # napi-rs bindings → npm (Node/Deno/Bun)
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

```bash
cargo run -p cdp-driver --example basic
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
