pub mod client;
pub mod agent;
pub mod config;
pub mod network;
pub mod page;
pub mod types;
pub mod error;

pub use client::CdpClient;
pub use agent::{ActionBuilder, ActionResult, BrowserAction, BrowserAgent};
pub use config::Config;
pub use types::*;
pub use error::{CdpError, Result};
