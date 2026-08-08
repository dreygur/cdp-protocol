#![deny(clippy::all)]

//! Node/Deno/Bun bindings for the `cdp-protocol` crate.
//!
//! Three classes are exported:
//! - [`CdpClient`](client::CdpClient) low-level CDP client (1:1 with the Rust `CdpClient`).
//! - [`BrowserAgent`](agent::BrowserAgent) high-level action runner (navigate/click/fill/...).
//! - [`Cluster`](cluster::Cluster) fixed-size pool of agents for concurrent work.

pub mod action_result;
pub mod agent;
pub mod client;
pub mod cluster;
pub mod config;
pub mod errors;
