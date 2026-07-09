//! Chrome DevTools Protocol (CDP) client for browser automation.
//!
//! Connects to a Chrome instance started with `--remote-debugging-port` and drives it
//! over the CDP WebSocket: navigation, DOM/JS evaluation, screenshots, network
//! interception, cookies, and more.
//!
//! Two API layers are available:
//! - [`CdpClient`]: low-level, one method per CDP command.
//! - [`BrowserAgent`] + [`BrowserAction`]: higher-level actions suited to driving
//!   the browser from an LLM tool call (see [`BrowserAgent::execute_json`]).
//!
//! Enable the `blocking` feature for a synchronous API ([`blocking`]) that needs no
//! async runtime from the caller.
//!
//! # Example
//!
//! ```no_run
//! use cdp_driver::{BrowserAgent, BrowserAction, Config};
//!
//! # async fn run() -> cdp_driver::Result<()> {
//! let agent = BrowserAgent::connect_with_config(&Config::default()).await?;
//!
//! agent.execute(BrowserAction::Navigate {
//!     url: "https://example.com".to_string(),
//! }).await;
//! # Ok(())
//! # }
//! ```

pub mod agent;
pub mod client;
pub mod cluster;
pub mod config;
pub mod error;
pub mod network;
pub mod page;
pub mod types;

#[cfg(feature = "blocking")]
pub mod blocking;

pub use agent::{ActionBuilder, ActionResult, BrowserAction, BrowserAgent};
pub use client::CdpClient;
pub use config::Config;
pub use error::{CdpError, Result};
pub use types::*;
