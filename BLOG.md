# Browser Automation Protocols: CDP vs WebDriver Deep Dive

> A technical lead's perspective on browser automation internals, protocol architectures, and when to use what.

---

## Table of Contents

1. [The Two Protocols](#the-two-protocols)
2. [Architecture Comparison](#architecture-comparison)
3. [WebDriver Protocol (W3C)](#webdriver-protocol-w3c)
4. [Chrome DevTools Protocol (CDP)](#chrome-devtools-protocol-cdp)
5. [Head-to-Head Comparison](#head-to-head-comparison)
6. [When to Use What](#when-to-use-what)
7. [Our CDP Implementation](#our-cdp-implementation)
8. [Production Examples](#production-examples)
9. [Final Thoughts](#final-thoughts)

---

## The Two Protocols

Browser automation comes down to two fundamental approaches:

**WebDriver** - W3C standardized, cross-browser, high-level abstraction over HTTP REST.

**CDP** - Chrome's native debugging protocol, WebSocket-based, low-level access to browser internals.

Both solve browser automation. Neither is universally "better." Your use case dictates the choice.

---

## Architecture Comparison

### WebDriver Architecture

```
┌──────────────┐    HTTP/REST    ┌──────────────┐    Native    ┌─────────────┐
│    Client    │ ◄────────────► │    Driver    │ ◄──────────► │   Browser   │
│  (Selenium)  │   Port 4444    │  (chromedriver│   Protocol  │   (Chrome)  │
└──────────────┘                │   geckodriver)│              └─────────────┘
                                └──────────────┘
```

**Three-tier model:**
1. Client library sends HTTP requests
2. Driver binary translates to browser-native calls
3. Browser executes and responds

**The middleman tax:** Every command pays HTTP overhead + driver process latency.

### CDP Architecture

```
┌──────────────┐    WebSocket    ┌─────────────┐
│    Client    │ ◄────────────► │   Browser   │
│   (Direct)   │   Port 9222    │   (Chrome)  │
└──────────────┘                └─────────────┘
```

**Two-tier model:**
1. Client connects directly to browser
2. Persistent WebSocket, bidirectional streaming

**No middleman.** Direct protocol access. Events pushed in real-time.

---

## WebDriver Protocol (W3C)

### Overview

WebDriver is a [W3C Recommendation](https://www.w3.org/TR/webdriver2/) since 2018. It defines a REST API for browser automation with focus on cross-browser compatibility.

### Transport

HTTP/REST with JSON payloads:

```
POST /session HTTP/1.1
Content-Type: application/json

{
  "capabilities": {
    "browserName": "chrome",
    "browserVersion": "120"
  }
}
```

### Session Lifecycle

```bash
# Create session
POST /session
→ {"sessionId": "abc123", "capabilities": {...}}

# All subsequent commands use session ID
POST /session/abc123/url
GET  /session/abc123/title
POST /session/abc123/element
DELETE /session/abc123
```

### Core Endpoints

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/session` | POST | Create new session |
| `/session/{id}` | DELETE | End session |
| `/session/{id}/url` | POST | Navigate to URL |
| `/session/{id}/url` | GET | Get current URL |
| `/session/{id}/title` | GET | Get page title |
| `/session/{id}/element` | POST | Find element |
| `/session/{id}/element/{eid}/click` | POST | Click element |
| `/session/{id}/element/{eid}/value` | POST | Send keys |
| `/session/{id}/screenshot` | GET | Capture screenshot |
| `/session/{id}/execute/sync` | POST | Execute JS |

### Element Location Strategies

```json
{
  "using": "css selector",
  "value": "button.submit"
}
```

Supported locators:
- `css selector`
- `link text`
- `partial link text`
- `tag name`
- `xpath`

### Example: Complete Flow

```bash
# 1. Create session
curl -X POST http://localhost:4444/session \
  -H "Content-Type: application/json" \
  -d '{"capabilities": {"browserName": "chrome"}}'

# Response: {"value": {"sessionId": "xyz789", ...}}

# 2. Navigate
curl -X POST http://localhost:4444/session/xyz789/url \
  -H "Content-Type: application/json" \
  -d '{"url": "https://example.com"}'

# 3. Find element
curl -X POST http://localhost:4444/session/xyz789/element \
  -H "Content-Type: application/json" \
  -d '{"using": "css selector", "value": "h1"}'

# Response: {"value": {"element-6066-...": "element-id-123"}}

# 4. Get text
curl http://localhost:4444/session/xyz789/element/element-id-123/text

# Response: {"value": "Example Domain"}

# 5. Screenshot
curl http://localhost:4444/session/xyz789/screenshot

# Response: {"value": "iVBORw0KGgo...base64..."}

# 6. Cleanup
curl -X DELETE http://localhost:4444/session/xyz789
```

### Limitations

1. **No network interception** - Can't inspect/modify HTTP traffic
2. **No console access** - Can't capture `console.log` output
3. **No performance metrics** - No access to rendering/memory data
4. **No real-time events** - Polling only, no push notifications
5. **Driver dependency** - Requires separate driver binary per browser
6. **Version coupling** - Driver version must match browser version

---

## Chrome DevTools Protocol (CDP)

### Overview

CDP is Chrome's native debugging protocol. It's what DevTools uses internally. Direct access to 61 domains covering every browser capability.

### Transport

Bidirectional WebSocket with JSON-RPC:

```
Client                                Browser
   │                                     │
   │──── {"id":1,"method":"Page.navigate", ───►
   │      "params":{"url":"..."}}        │
   │                                     │
   │◄─── {"id":1,"result":{"frameId":...}} ───
   │                                     │
   │◄─── {"method":"Page.loadEventFired", ────
   │      "params":{"timestamp":...}}    │
   │                                     │
```

Three message types:
- **Request**: Client → Browser (has `id` + `method`)
- **Response**: Browser → Client (has `id` + `result`/`error`)
- **Event**: Browser → Client (has `method` only, no `id`)

### Domain Organization

CDP organizes into domains. Each domain has methods and events.

**Core domains:**

| Domain | Methods | Events | Purpose |
|--------|---------|--------|---------|
| Page | 25+ | 15+ | Navigation, lifecycle, screenshots |
| Runtime | 20+ | 10+ | JS execution, console |
| DOM | 30+ | 10+ | Document structure |
| Network | 15+ | 20+ | HTTP traffic |
| Input | 5+ | 0 | Mouse, keyboard, touch |
| Emulation | 20+ | 0 | Device simulation |
| Target | 15+ | 5+ | Tab/window management |
| Debugger | 25+ | 10+ | JS debugging |
| Profiler | 10+ | 5+ | CPU profiling |
| HeapProfiler | 10+ | 5+ | Memory profiling |

### HTTP Discovery Endpoints

Before WebSocket, discover targets via HTTP:

```bash
# List all debuggable targets
curl http://localhost:9222/json/list
[
  {
    "id": "ABC123",
    "type": "page",
    "title": "New Tab",
    "url": "chrome://newtab/",
    "webSocketDebuggerUrl": "ws://localhost:9222/devtools/page/ABC123"
  }
]

# Browser version
curl http://localhost:9222/json/version
{
  "Browser": "Chrome/120.0.0.0",
  "Protocol-Version": "1.3",
  "webSocketDebuggerUrl": "ws://localhost:9222/devtools/browser/XYZ"
}

# Create new tab
curl http://localhost:9222/json/new?https://example.com

# Close tab
curl http://localhost:9222/json/close/ABC123
```

### Protocol Examples

**1. Navigation**

```json
// Enable Page domain first
→ {"id": 1, "method": "Page.enable"}
← {"id": 1, "result": {}}

// Navigate
→ {"id": 2, "method": "Page.navigate", "params": {"url": "https://example.com"}}
← {"id": 2, "result": {"frameId": "ABC", "loaderId": "XYZ"}}

// Events fired automatically
← {"method": "Page.frameStartedLoading", "params": {"frameId": "ABC"}}
← {"method": "Page.loadEventFired", "params": {"timestamp": 1234.56}}
← {"method": "Page.frameStoppedLoading", "params": {"frameId": "ABC"}}
```

**2. JavaScript Evaluation**

```json
→ {"id": 1, "method": "Runtime.enable"}
← {"id": 1, "result": {}}

→ {"id": 2, "method": "Runtime.evaluate", "params": {
    "expression": "document.title",
    "returnByValue": true
  }}
← {"id": 2, "result": {
    "result": {"type": "string", "value": "Example Domain"}
  }}

// Complex evaluation
→ {"id": 3, "method": "Runtime.evaluate", "params": {
    "expression": "(() => { return {width: window.innerWidth, height: window.innerHeight}; })()",
    "returnByValue": true
  }}
← {"id": 3, "result": {
    "result": {"type": "object", "value": {"width": 1920, "height": 1080}}
  }}
```

**3. DOM Operations**

```json
→ {"id": 1, "method": "DOM.enable"}
← {"id": 1, "result": {}}

→ {"id": 2, "method": "DOM.getDocument", "params": {"depth": 0}}
← {"id": 2, "result": {
    "root": {"nodeId": 1, "nodeName": "#document", "childNodeCount": 2}
  }}

→ {"id": 3, "method": "DOM.querySelector", "params": {"nodeId": 1, "selector": "h1"}}
← {"id": 3, "result": {"nodeId": 42}}

→ {"id": 4, "method": "DOM.getOuterHTML", "params": {"nodeId": 42}}
← {"id": 4, "result": {"outerHTML": "<h1>Example Domain</h1>"}}
```

**4. Network Interception**

```json
→ {"id": 1, "method": "Network.enable"}
← {"id": 1, "result": {}}

// Events stream automatically
← {"method": "Network.requestWillBeSent", "params": {
    "requestId": "req-1",
    "request": {
      "url": "https://example.com/api/data",
      "method": "GET",
      "headers": {"Accept": "application/json"}
    },
    "timestamp": 1234.56,
    "type": "XHR"
  }}

← {"method": "Network.responseReceived", "params": {
    "requestId": "req-1",
    "response": {
      "status": 200,
      "statusText": "OK",
      "headers": {"content-type": "application/json"},
      "mimeType": "application/json"
    }
  }}

← {"method": "Network.loadingFinished", "params": {
    "requestId": "req-1",
    "encodedDataLength": 1234
  }}

// Get response body
→ {"id": 2, "method": "Network.getResponseBody", "params": {"requestId": "req-1"}}
← {"id": 2, "result": {"body": "{\"data\": [...]}", "base64Encoded": false}}
```

**5. Screenshots**

```json
→ {"id": 1, "method": "Page.captureScreenshot", "params": {
    "format": "png",
    "quality": 100,
    "fromSurface": true
  }}
← {"id": 1, "result": {"data": "iVBORw0KGgoAAAANSUhEUgAAA..."}}

// Full page screenshot
→ {"id": 2, "method": "Page.captureScreenshot", "params": {
    "format": "png",
    "captureBeyondViewport": true
  }}

// Specific region
→ {"id": 3, "method": "Page.captureScreenshot", "params": {
    "format": "jpeg",
    "quality": 80,
    "clip": {"x": 0, "y": 0, "width": 800, "height": 600, "scale": 1}
  }}
```

**6. Input Simulation**

```json
// Mouse click
→ {"id": 1, "method": "Input.dispatchMouseEvent", "params": {
    "type": "mousePressed",
    "x": 100, "y": 200,
    "button": "left",
    "clickCount": 1
  }}
← {"id": 1, "result": {}}

→ {"id": 2, "method": "Input.dispatchMouseEvent", "params": {
    "type": "mouseReleased",
    "x": 100, "y": 200,
    "button": "left",
    "clickCount": 1
  }}

// Type text
→ {"id": 3, "method": "Input.insertText", "params": {"text": "Hello World"}}

// Key press
→ {"id": 4, "method": "Input.dispatchKeyEvent", "params": {
    "type": "keyDown",
    "key": "Enter",
    "code": "Enter",
    "windowsVirtualKeyCode": 13
  }}
→ {"id": 5, "method": "Input.dispatchKeyEvent", "params": {
    "type": "keyUp",
    "key": "Enter",
    "code": "Enter"
  }}
```

**7. Console Capture**

```json
→ {"id": 1, "method": "Runtime.enable"}
← {"id": 1, "result": {}}

// Console events stream automatically
← {"method": "Runtime.consoleAPICalled", "params": {
    "type": "log",
    "args": [{"type": "string", "value": "Hello from page"}],
    "timestamp": 1234567890.123
  }}

← {"method": "Runtime.consoleAPICalled", "params": {
    "type": "error",
    "args": [{"type": "string", "value": "Something went wrong"}],
    "stackTrace": {...}
  }}
```

**8. Performance Metrics**

```json
→ {"id": 1, "method": "Performance.enable"}
← {"id": 1, "result": {}}

→ {"id": 2, "method": "Performance.getMetrics"}
← {"id": 2, "result": {
    "metrics": [
      {"name": "Timestamp", "value": 1234.56},
      {"name": "Documents", "value": 1},
      {"name": "Frames", "value": 1},
      {"name": "JSEventListeners", "value": 42},
      {"name": "Nodes", "value": 150},
      {"name": "LayoutCount", "value": 3},
      {"name": "RecalcStyleCount", "value": 5},
      {"name": "JSHeapUsedSize", "value": 10485760},
      {"name": "JSHeapTotalSize", "value": 16777216}
    ]
  }}
```

---

## Head-to-Head Comparison

### Protocol Level

| Aspect | WebDriver | CDP |
|--------|-----------|-----|
| **Specification** | W3C Standard | Chrome Internal |
| **Transport** | HTTP REST | WebSocket |
| **Connection** | Request/Response | Persistent + Events |
| **Latency** | Higher (HTTP per command) | Lower (single WS) |
| **Message Format** | JSON over HTTP | JSON-RPC over WS |

### Architecture

| Aspect | WebDriver | CDP |
|--------|-----------|-----|
| **Components** | Client + Driver + Browser | Client + Browser |
| **Driver Required** | Yes (chromedriver, etc.) | No |
| **Version Coupling** | Driver ↔ Browser tight | Protocol versioned |
| **Port** | 4444 (driver) | 9222 (browser) |

### Capabilities

| Feature | WebDriver | CDP |
|---------|-----------|-----|
| Navigation | ✅ | ✅ |
| Element Interaction | ✅ | ✅ |
| JavaScript Execution | ✅ | ✅ |
| Screenshots | ✅ | ✅ |
| Cookies | ✅ | ✅ |
| **Network Interception** | ❌ | ✅ |
| **Console Access** | ❌ | ✅ |
| **Performance Metrics** | ❌ | ✅ |
| **Real-time Events** | ❌ | ✅ |
| **DOM Debugging** | ❌ | ✅ |
| **CPU Profiling** | ❌ | ✅ |
| **Memory Profiling** | ❌ | ✅ |
| **Geolocation Emulation** | Limited | ✅ |
| **Device Emulation** | Limited | ✅ |
| **Request Blocking** | ❌ | ✅ |

### Browser Support

| Browser | WebDriver | CDP |
|---------|-----------|-----|
| Chrome | ✅ | ✅ |
| Edge | ✅ | ✅ (Chromium) |
| Firefox | ✅ | Partial |
| Safari | ✅ | ❌ |
| Opera | ✅ | ✅ (Chromium) |

### Ecosystem

| Tool | WebDriver | CDP |
|------|-----------|-----|
| Selenium | Primary | Via BiDi |
| Puppeteer | ❌ | Primary |
| Playwright | Uses both | Uses both |
| Cypress | ❌ | Primary |

---

## When to Use What

### Use WebDriver When:

1. **Cross-browser testing** - Need Safari, Firefox, Chrome uniformly
2. **Existing Selenium infrastructure** - Large test suites already written
3. **Simple automation** - Basic click, type, navigate workflows
4. **Compliance requirements** - W3C standard may be mandated
5. **Team familiarity** - Team knows Selenium well

### Use CDP When:

1. **Chrome/Chromium only** - Target browser is fixed
2. **Network interception** - Mock APIs, block resources, modify requests
3. **Performance profiling** - Need rendering metrics, memory analysis
4. **Console monitoring** - Capture JS logs, errors, warnings
5. **Real-time events** - React to page events as they happen
6. **Speed critical** - Minimize automation overhead
7. **AI agents** - Need granular control for autonomous browsing
8. **Advanced debugging** - JS breakpoints, DOM inspection

### Hybrid Approach (Playwright/Selenium 4)

Modern tools use both:

```
Playwright:
  - WebDriver for cross-browser compat
  - CDP for Chrome-specific features

Selenium 4 BiDi:
  - WebDriver base protocol
  - CDP bridge for advanced features
```

---

## Our CDP Implementation

We built a production-ready Rust CDP client with two abstraction layers.

### Project Structure

```
cdp-protocol/
├── src/
│   ├── lib.rs          # Public exports
│   ├── client.rs       # Low-level CDP client (WebSocket, routing)
│   ├── agent.rs        # High-level BrowserAgent (AI-friendly)
│   ├── types.rs        # Protocol message types
│   └── error.rs        # Error handling
├── examples/
│   ├── basic.rs        # Low-level usage
│   ├── agent.rs        # High-level AI agent
│   └── industrial.rs   # Parallel scraping
└── Cargo.toml
```

### Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.21"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.11", features = ["json"] }
base64 = "0.21"
tracing = "0.1"
```

### Layer 1: CdpClient (Low-Level)

Direct protocol access with convenience wrappers.

```rust
use cdp_protocol::{CdpClient, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Discovery
    let version = CdpClient::get_version("localhost", 9222).await?;
    println!("Browser: {}", version.browser);

    let targets = CdpClient::list_targets("localhost", 9222).await?;
    for target in &targets {
        println!("  - {} [{}]: {}", target.target_type, target.id, target.title);
    }

    // Connect
    let client = CdpClient::connect_to_page("localhost", 9222).await?;

    // Enable domains
    client.enable_domain("Page").await?;
    client.enable_domain("Runtime").await?;
    client.enable_domain("DOM").await?;

    // Navigate
    let nav = client.navigate("https://example.com").await?;
    println!("Frame ID: {}", nav.frame_id);

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // JavaScript
    let title: String = client.eval("document.title").await?;
    println!("Title: {}", title);

    let result = client.evaluate("1 + 2 * 3").await?;
    println!("Math: {:?}", result.result.value);

    // DOM
    let doc = client.get_document().await?;
    let h1_id = client.query_selector(doc.node_id, "h1").await?;
    if h1_id > 0 {
        let html = client.get_outer_html(h1_id).await?;
        println!("H1: {}", html);
    }

    // Screenshot
    client.screenshot_to_file("example.png").await?;

    // Cookies
    let cookies = client.get_cookies().await?;
    println!("Cookies: {}", cookies.len());

    Ok(())
}
```

### Layer 2: BrowserAgent (High-Level)

AI-friendly interface with JSON action dispatch.

```rust
use cdp_protocol::{BrowserAgent, BrowserAction, ActionResult};

#[tokio::main]
async fn main() -> Result<()> {
    let agent = BrowserAgent::connect("localhost", 9222).await?;

    // Programmatic
    agent.execute(BrowserAction::Navigate {
        url: "https://example.com".to_string(),
    }).await;

    agent.execute(BrowserAction::GetTitle).await;

    // JSON (LLM tool calls)
    agent.execute_json(r#"{"action": "navigate", "url": "https://rust-lang.org"}"#).await;
    agent.execute_json(r#"{"action": "wait", "ms": 2000}"#).await;
    agent.execute_json(r#"{"action": "screenshot", "path": "rust.png"}"#).await;

    Ok(())
}
```

### Action Builder

Fluent API for chaining:

```rust
use cdp_protocol::ActionBuilder;

let actions = ActionBuilder::new()
    .navigate("https://www.google.com")
    .wait(1500)
    .fill("input[name='q']", "Rust programming")
    .press_key("Enter")
    .wait(2000)
    .screenshot(Some("search.png"))
    .build();

let results = agent.execute_many(actions).await;
```

### Supported Actions

```rust
pub enum BrowserAction {
    // Navigation
    Navigate { url: String },
    GoBack,
    GoForward,
    Reload,

    // Interaction
    Click { selector: Option<String>, x: Option<f64>, y: Option<f64> },
    Type { text: String, selector: Option<String> },
    Fill { selector: String, value: String },
    Submit { selector: Option<String> },
    PressKey { key: String },

    // Inspection
    GetTitle,
    GetUrl,
    GetText,
    GetContent { selector: Option<String> },
    GetLinks,
    GetAttributes { selector: String },
    Exists { selector: String },

    // Capture
    Screenshot { path: Option<String> },

    // Scripting
    Evaluate { expression: String },

    // Waiting
    Wait { ms: u64 },
    WaitForSelector { selector: String, timeout_ms: u64 },

    // Layout
    Scroll { x: f64, y: f64 },
    SetViewport { width: i32, height: i32, mobile: bool },

    // Metrics
    GetMetrics,
}
```

---

## Production Examples

### Form Automation

```rust
let search_actions = vec![
    BrowserAction::Navigate {
        url: "https://duckduckgo.com".to_string(),
    },
    BrowserAction::Wait { ms: 1500 },
    BrowserAction::Fill {
        selector: "input[name='q']".to_string(),
        value: "Rust programming language".to_string(),
    },
    BrowserAction::PressKey {
        key: "Enter".to_string(),
    },
    BrowserAction::Wait { ms: 2000 },
    BrowserAction::Screenshot {
        path: Some("search_results.png".to_string()),
    },
    BrowserAction::GetTitle,
];

for action in search_actions {
    let result = agent.execute(action).await;
    if !result.is_success() {
        println!("Failed: {:?}", result);
        break;
    }
}
```

### Data Extraction

```rust
let result = agent.execute(BrowserAction::Evaluate {
    expression: r#"
        (() => {
            return {
                viewport: {
                    width: window.innerWidth,
                    height: window.innerHeight
                },
                userAgent: navigator.userAgent,
                language: navigator.language,
                cookiesEnabled: navigator.cookieEnabled,
                platform: navigator.platform
            };
        })()
    "#.to_string(),
}).await;
```

### Industrial Scraping (50 Pages Parallel)

```rust
use cdp_protocol::{CdpClient, Result};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;

const NUM_PAGES: usize = 50;
const MAX_CONCURRENT: usize = 10;

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
    std::fs::create_dir_all("screenshots").ok();

    let start = Instant::now();
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT));

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
                println!("[{:3}] ✗ Panic: {}", i, e);
                failed += 1;
            }
        }
    }

    let total = start.elapsed();
    println!("\nTotal: {:.2}s | Success: {} | Failed: {} | {:.2} pages/sec",
        total.as_secs_f64(), success, failed,
        NUM_PAGES as f64 / total.as_secs_f64());

    Ok(())
}

async fn process_page(id: usize, url: &str) -> Result<(String, f64)> {
    let start = Instant::now();

    let target = CdpClient::create_tab("localhost", 9222, Some(url)).await?;
    let ws_url = target.web_socket_debugger_url
        .ok_or_else(|| cdp_protocol::CdpError::InvalidUrl("No WS URL".into()))?;

    let client = CdpClient::connect(&ws_url).await?;
    client.enable_domain("Page").await?;
    client.enable_domain("Runtime").await?;

    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    let title: String = client.eval("document.title").await
        .unwrap_or_else(|_| "Unknown".to_string());

    client.screenshot_to_file(&format!("screenshots/page_{:03}.png", id)).await?;

    Ok((title, start.elapsed().as_secs_f64()))
}
```

**Output:**
```
=== Industrial Scraping Demo ===
Pages to process: 50
Max concurrent: 10

[  0] ✓ Rust Programming Language (3.2s)
[  1] ✓ Google (2.8s)
...
[ 49] ✓ Hacker News (2.9s)

Total: 18.42s | Success: 50 | Failed: 0 | 2.71 pages/sec
```

---

## Final Thoughts

### Protocol Selection Matrix

| Requirement | Recommendation |
|-------------|----------------|
| Cross-browser testing | WebDriver |
| Chrome-only, max performance | CDP |
| Network mocking | CDP |
| AI agent automation | CDP |
| Existing Selenium codebase | WebDriver (+ BiDi for CDP features) |
| Console/log capture | CDP |
| Performance profiling | CDP |
| Simple E2E tests | Either works |

### The Future

**WebDriver BiDi** is bridging the gap - adding CDP-like capabilities to WebDriver. Selenium 4 already supports it. Eventually, you'll get the best of both worlds through a unified spec.

Until then:
- **WebDriver** for cross-browser standardization
- **CDP** for Chrome power-user features

Our Rust implementation gives you CDP's full power with ergonomic abstractions. Low-level when you need it, high-level when you don't.

---

## Resources

- [W3C WebDriver Spec](https://www.w3.org/TR/webdriver2/)
- [CDP Protocol Viewer](https://chromedevtools.github.io/devtools-protocol/)
- [Puppeteer Docs](https://pptr.dev/)
- [Playwright Docs](https://playwright.dev/)
- [Selenium BiDi](https://www.selenium.dev/documentation/webdriver/bidirectional/)

---

*Built for engineers who need to understand the protocol, not just use a wrapper.*
