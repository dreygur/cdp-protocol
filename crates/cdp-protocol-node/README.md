# cdp-protocol (Node / Deno / Bun)

napi-rs bindings for the [`cdp-protocol`](https://crates.io/crates/cdp-protocol)
Rust crate: a Chrome DevTools Protocol client. WebSocket transport and the tokio
runtime run in native Rust; JS gets Promises and TypeScript types.

Connects to a Chrome/Chromium already listening on a debug port. It does not
spawn the browser:

```bash
google-chrome --headless=new --remote-debugging-port=9222 --no-first-run
```

## Install

```bash
npm i cdp-protocol      # Node / Bun
```

```ts
import { CdpClient } from 'npm:cdp-protocol'   // Deno
```

Rust snake_case maps to JS camelCase automatically (`connect_to_page` →
`connectToPage`). Native addon per platform is resolved at runtime; all three
runtimes load the same `.node` via Node-API.

## Three classes

### `CdpClient`  low-level, 1:1 with the CDP domains

```js
import { CdpClient } from 'cdp-protocol'

const client = await CdpClient.connectToPage('127.0.0.1', 9222)
await client.enableDomain('Page')
await client.enableDomain('Runtime')

await client.navigateAndWait('https://example.com', 10_000)
const title = await client.eval('document.title')
const png = await client.screenshot()          // Buffer
const cookies = await client.getCookies()       // JSON
await client.close()
```

Statics: `connect(wsUrl)`, `connectToPage(host, port)`, `getVersion`,
`listTargets`, `createTab`. Instance: `enableDomain`, `navigate`,
`navigateAndWait`, `eval`, `evaluate`, `waitForEvent`, `querySelector`,
`getOuterHtml`, `screenshot(ToFile)`, `fullPageScreenshot(ToFile)`,
`setViewport`, `getCookies`, `close`.

### `BrowserAgent`  high-level actions (auto-enables Page/Runtime/DOM/Network)

```js
import { BrowserAgent } from 'cdp-protocol'

const agent = await BrowserAgent.connect('127.0.0.1', 9222)
await agent.navigate('https://example.com')
console.log((await agent.getTitle()).value)

const results = await agent.executeMany([
  { action: 'fill', selector: '#q', value: 'hello' },
  { action: 'press_key', key: 'Enter' },
  { action: 'wait_for_selector', selector: '.result', timeout_ms: 5000 },
  { action: 'get_links' },
])
await agent.close()
```

Every action returns `{ success, value, error }`. Actions accept an object or a
JSON string (`execute`, `executeJson`, `executeMany`). Action names:

`navigate` · `back` · `forward` · `reload` · `click` · `type` · `fill` ·
`submit` · `press_key` · `get_title` · `get_url` · `get_text` · `get_content` ·
`get_links` · `get_attributes` · `exists` · `screenshot` · `evaluate` · `wait` ·
`wait_for_selector` · `scroll` · `set_viewport` · `get_metrics`

### `Cluster`  fixed-size pool, one tab per worker

```js
import { Cluster } from 'cdp-protocol'

const cluster = await Cluster.create({
  host: '127.0.0.1', port: 9222, concurrency: 4, retries: 1,
})

const tasks = await Promise.all(
  urls.map((url) =>
    cluster.execute([
      { action: 'navigate', url },
      { action: 'get_title' },
    ]),
  ),
)
// each: { success, results: ActionResult[], elapsedMs, attempts, error }
await cluster.close()
```

A worker is checked out per `execute` call, capped by `concurrency`; a failed
batch retries up to `retries` times.

## Examples

Mirror the Rust `examples/`. Need Chrome on `:9222` and a built addon.

```bash
npm run example:basic        # low-level CdpClient
npm run example:agent        # BrowserAgent: object + JSON actions, batches
npm run example:cluster      # worker pool with retries
npm run example:industrial   # fresh tab per page, bounded concurrency
```

## Build from source

```bash
npm install
npm run build          # release; emits index.js, index.d.ts, *.node
node test.mjs          # smoke test (needs Chrome on :9222)
```

`napi build` generates `index.js` (loader picking the right prebuilt `.node`)
and `index.d.ts` (typings derived from the `#[napi]` attributes).

## Load test

`npm run load-test` stresses the native async boundary (the shared-`&self` +
`Arc`-clone pattern) against a real Chrome. Local run (linux-x64, headless):

| Scenario | Result |
| --- | --- |
| 3000 concurrent `eval()` on one shared client | 0 mismatches, ~48k ops/s |
| Cluster (12 workers, 600 tasks), per-task title check | 600/600 ok, 0 cross-talk |
| 200 concurrent screenshots (Buffer marshaling) | 200/200 ok, 0 empty |
| Memory | RSS stable ~52→74 MB, no leak |

Confirms the pending-request map is race-free, workers don't cross-talk, and
`Buffer` returns are thread-safe under load.

## Publishing

CI (`.github/workflows/node-bindings.yml`) builds one `.node` per platform, then
on a `node-v*` tag runs `napi prepublish` + `npm publish`. Per-platform binaries
ship as `@scope`-style optional dependencies of the meta package.
