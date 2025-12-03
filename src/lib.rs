//! CDP Protocol - Chrome DevTools Protocol client for AI agent browser control
//!
//! This crate provides a Rust implementation of the Chrome DevTools Protocol,
//! designed for AI agents to control browsers programmatically.
//!
//! # Quick Start
//!
//! ```no_run
//! use cdp_protocol::{BrowserAgent, BrowserAction};
//!
//! #[tokio::main]
//! async fn main() -> cdp_protocol::Result<()> {
//!     // Connect to Chrome (must be running with --remote-debugging-port=9222)
//!     let agent = BrowserAgent::connect("localhost", 9222).await?;
//!
//!     // Navigate and interact
//!     agent.execute(BrowserAction::Navigate {
//!         url: "https://example.com".to_string()
//!     }).await;
//!
//!     // Take screenshot
//!     agent.execute(BrowserAction::Screenshot {
//!         path: Some("screenshot.png".to_string())
//!     }).await;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Architecture
//!
//! - `CdpClient`: Low-level CDP client with WebSocket connection
//! - `BrowserAgent`: High-level AI-friendly interface
//! - `BrowserAction`: Enum of all supported browser actions
//! - `ActionResult`: Result type for action execution

pub mod agent;
pub mod client;
pub mod error;
pub mod types;

pub use agent::{ActionBuilder, ActionResult, BrowserAction, BrowserAgent};
pub use client::CdpClient;
pub use error::{CdpError, Result};
pub use types::*;
