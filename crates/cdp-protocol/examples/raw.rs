//! Reaching CDP domains this crate has no typed wrapper for.
//!
//! `CdpClient` wraps 38 of the protocol's 664 commands. The rest are reachable
//! through `call` and `call_raw`, keyed by the constants in `cdp_driver::methods`
//! so the compiler rejects a misspelled method name.
//!
//! Every domain touched here (`Browser`, `Target`, `Storage`) has no wrapper at
//! all, so none of this is reachable any other way.

use cdp_driver::methods::{browser, storage, target};
use cdp_driver::{CdpClient, CdpError, Config, Result};
use serde::Deserialize;
use serde_json::json;

#[path = "common/logging.rs"]
mod logging;

/// The origin whose storage gets cleared, chosen because it is nobody's real site.
const ORIGIN: &str = "https://example.com";

/// A method no browser implements, used to show what a rejection looks like.
const NONSENSE: &str = "Nonsense.command";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserVersion {
    product: String,
    js_version: String,
    protocol_version: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TargetInfo {
    target_id: String,
    #[serde(rename = "type")]
    kind: String,
    title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Targets {
    target_infos: Vec<TargetInfo>,
}

/// `call` deserializes the command's result into whatever type you ask for.
async fn print_browser_version(client: &CdpClient) -> Result<()> {
    let version: BrowserVersion = client.call(browser::GET_VERSION, json!({})).await?;

    println!("Product:  {}", version.product);
    println!("JS:       {}", version.js_version);
    println!("Protocol: {}", version.protocol_version);
    Ok(())
}

/// A result with nested arrays deserializes as readily as a flat one.
async fn print_targets(client: &CdpClient) -> Result<()> {
    let targets: Targets = client.call(target::GET_TARGETS, json!({})).await?;

    println!("\nTargets ({}):", targets.target_infos.len());
    for info in &targets.target_infos {
        println!("  - {} [{}]: {}", info.kind, info.target_id, info.title);
    }
    Ok(())
}

/// `call_raw` hands back the untouched result, which is what you want when you
/// are still learning a command's shape and have no type for it yet.
async fn print_storage_usage(client: &CdpClient) -> Result<()> {
    let usage = client
        .call_raw(storage::GET_USAGE_AND_QUOTA, json!({ "origin": ORIGIN }))
        .await?;

    println!(
        "\nStorage for {ORIGIN}: {} bytes used of {} quota",
        usage["usage"].as_f64().unwrap_or(0.0),
        usage["quota"].as_f64().unwrap_or(0.0),
    );
    Ok(())
}

/// Commands that answer with nothing still succeed; the result is `Null`.
async fn clear_storage(client: &CdpClient) -> Result<()> {
    let answer = client
        .call_raw(
            storage::CLEAR_DATA_FOR_ORIGIN,
            json!({ "origin": ORIGIN, "storageTypes": "cookies,local_storage" }),
        )
        .await?;

    println!("\nCleared storage for {ORIGIN}, result: {answer}");
    Ok(())
}

/// A method the browser does not know becomes `CdpError::Protocol`, carrying
/// the browser's own message.
async fn show_rejection(client: &CdpClient) {
    match client.call_raw(NONSENSE, json!({})).await {
        Err(CdpError::Protocol(message)) => println!("\nChrome rejected it: {message}"),
        Err(other) => println!("\nUnexpected error: {other}"),
        Ok(_) => println!("\nChrome accepted a method that does not exist"),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _guard = logging::init();

    let cfg = Config::default();
    let client = CdpClient::connect_to_page(&cfg.host, cfg.port).await?;

    print_browser_version(&client).await?;
    print_targets(&client).await?;
    print_storage_usage(&client).await?;
    clear_storage(&client).await?;
    show_rejection(&client).await;

    Ok(())
}
