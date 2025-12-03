# CDP Protocol - AI Agent Browser Control

A Rust implementation of the Chrome DevTools Protocol (CDP) designed for AI agents to control browsers programmatically.

## Features

- **WebSocket CDP Client**: Full async CDP communication
- **High-level Agent API**: AI-friendly browser control interface
- **JSON Actions**: Execute browser actions via JSON (perfect for LLM integration)
- **Type-safe**: Full Rust type safety for CDP messages
- **Action Builder**: Fluent API for chaining browser actions

## Quick Start

### 1. Start Chrome with Remote Debugging

```bash
google-chrome --remote-debugging-port=9222
# or
chromium --remote-debugging-port=9222
```

### 2. Use the Library

```rust
use cdp_protocol::{BrowserAgent, BrowserAction};

#[tokio::main]
async fn main() -> cdp_protocol::Result<()> {
    // Connect to Chrome
    let agent = BrowserAgent::connect("localhost", 9222).await?;

    // Navigate
    agent.execute(BrowserAction::Navigate {
        url: "https://example.com".to_string()
    }).await;

    // Take screenshot
    agent.execute(BrowserAction::Screenshot {
        path: Some("screenshot.png".to_string())
    }).await;

    // Get page content
    let result = agent.execute(BrowserAction::GetText).await;

    Ok(())
}
```

### 3. AI Integration (JSON Actions)

```rust
// Execute actions from JSON - perfect for LLM tool calls
let result = agent.execute_json(r#"{"action": "navigate", "url": "https://example.com"}"#).await;
let result = agent.execute_json(r#"{"action": "click", "selector": "button.submit"}"#).await;
let result = agent.execute_json(r#"{"action": "fill", "selector": "#email", "value": "test@example.com"}"#).await;
```

## Available Actions

| Action | Parameters | Description |
|--------|------------|-------------|
| `navigate` | `url` | Navigate to URL |
| `click` | `selector` or `x,y` | Click element or coordinates |
| `type` | `text`, `selector?` | Type text |
| `fill` | `selector`, `value` | Fill form field |
| `press_key` | `key` | Press keyboard key |
| `screenshot` | `path?` | Capture screenshot |
| `evaluate` | `expression` | Execute JavaScript |
| `get_title` | - | Get page title |
| `get_url` | - | Get current URL |
| `get_text` | - | Get page text content |
| `get_content` | `selector?` | Get HTML content |
| `get_links` | - | Get all links |
| `wait` | `ms` | Wait milliseconds |
| `wait_for_selector` | `selector`, `timeout_ms?` | Wait for element |
| `scroll` | `x`, `y` | Scroll page |
| `go_back` | - | Navigate back |
| `go_forward` | - | Navigate forward |
| `reload` | - | Reload page |
| `set_viewport` | `width`, `height`, `mobile?` | Set viewport size |
| `exists` | `selector` | Check element exists |
| `get_attributes` | `selector` | Get element attributes |
| `submit` | `selector?` | Submit form |
| `get_metrics` | - | Get performance metrics |

## Low-level CDP Client

```rust
use cdp_protocol::CdpClient;

let client = CdpClient::connect_to_page("localhost", 9222).await?;

// Enable domains
client.enable_domain("Page").await?;
client.enable_domain("Runtime").await?;

// Send raw CDP commands
let result = client.send("Page.navigate", Some(json!({"url": "https://example.com"}))).await?;

// Convenience methods
client.navigate("https://example.com").await?;
client.screenshot_to_file("screenshot.png").await?;
let title: String = client.eval("document.title").await?;
```

## Examples

```bash
# Basic CDP client usage
cargo run --example basic

# AI agent demo
cargo run --example agent
```

## Project Structure

```
src/
├── lib.rs      # Library exports
├── client.rs   # CDP WebSocket client
├── agent.rs    # High-level AI agent interface
├── types.rs    # CDP message types
└── error.rs    # Error handling

examples/
├── basic.rs    # Basic CDP usage
└── agent.rs    # AI agent demo
```

## Resources

- [CDP Protocol Viewer](https://chromedevtools.github.io/devtools-protocol/)
- [CDP GitHub](https://github.com/ChromeDevTools/devtools-protocol)
- [Notion Notes](https://www.notion.so/Protocol-Monitor-2bdc516a350780978d52fc513dd83295)
