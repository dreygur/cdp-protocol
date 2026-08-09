//! `Page`, `Emulation`, and `DOM` mutation commands, added to [`CdpClient`] here.

use serde_json::json;
use tokio::sync::broadcast;

use crate::client::CdpClient;
use crate::error::{CdpError, Result};
use crate::types::NavigationResult;

/// Judge a `Page.navigate` result. Chrome answers a navigation it could not
/// perform with a frame id, a loader id and an `errorText`, so a result that
/// looks complete still has to be read as a failure.
fn navigation_outcome(url: &str, navigation: NavigationResult) -> Result<NavigationResult> {
    match &navigation.error_text {
        Some(reason) => Err(CdpError::Protocol(format!(
            "navigation to {url} failed: {reason}"
        ))),
        None => Ok(navigation),
    }
}

impl CdpClient {
    /// Replace the current document's content with `html`.
    pub async fn set_content(&self, html: &str) -> Result<()> {
        let frame_id = {
            let result = self.send_command("Page.getFrameTree", json!({})).await?;
            result["frameTree"]["frame"]["id"]
                .as_str()
                .unwrap_or("")
                .to_string()
        };
        self.send_command(
            "Page.setDocumentContent",
            json!({
                "frameId": frame_id,
                "html":    html,
            }),
        )
        .await?;
        Ok(())
    }

    /// Render the current page to PDF and write it to `path`.
    pub async fn print_to_pdf(&self, path: &str) -> Result<()> {
        let result = self
            .send_command(
                "Page.printToPDF",
                json!({
                    "printBackground": true,
                }),
            )
            .await?;
        let data = result["data"].as_str().unwrap_or("");
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data)
            .map_err(|e| crate::error::CdpError::Protocol(e.to_string()))?;
        tokio::fs::write(path, bytes).await?;
        Ok(())
    }

    /// Register `source` to run before every future document on this target
    /// (before any page script runs). Returns an identifier for [`remove_init_script`](Self::remove_init_script).
    pub async fn add_init_script(&self, source: &str) -> Result<String> {
        let result = self
            .send_command(
                "Page.addScriptToEvaluateOnNewDocument",
                json!({ "source": source }),
            )
            .await?;
        Ok(result["identifier"].as_str().unwrap_or("").to_string())
    }

    /// Unregister a script added via [`add_init_script`](Self::add_init_script).
    pub async fn remove_init_script(&self, identifier: &str) -> Result<()> {
        self.send_command(
            "Page.removeScriptToEvaluateOnNewDocument",
            json!({ "identifier": identifier }),
        )
        .await?;
        Ok(())
    }

    /// Override the `User-Agent` header and `navigator.userAgent`.
    pub async fn set_user_agent(&self, ua: &str) -> Result<()> {
        self.send_command("Emulation.setUserAgentOverride", json!({ "userAgent": ua }))
            .await?;
        Ok(())
    }

    /// Override the geolocation API's reported position.
    pub async fn set_geolocation(
        &self,
        latitude: f64,
        longitude: f64,
        accuracy: f64,
    ) -> Result<()> {
        self.send_command(
            "Emulation.setGeolocationOverride",
            json!({
                "latitude":  latitude,
                "longitude": longitude,
                "accuracy":  accuracy,
            }),
        )
        .await?;
        Ok(())
    }

    /// Simulate going offline (or restore normal networking).
    pub async fn set_offline(&self, offline: bool) -> Result<()> {
        self.send_command(
            "Network.emulateNetworkConditions",
            json!({
                "offline":            offline,
                "latency":            0,
                "downloadThroughput": -1,
                "uploadThroughput":   -1,
            }),
        )
        .await?;
        Ok(())
    }

    /// Set (or add) an attribute on a DOM node.
    pub async fn set_attribute(&self, node_id: i64, name: &str, value: &str) -> Result<()> {
        self.send_command(
            "DOM.setAttributeValue",
            json!({
                "nodeId": node_id,
                "name":   name,
                "value":  value,
            }),
        )
        .await?;
        Ok(())
    }

    /// Replace a DOM node's outer HTML.
    pub async fn set_outer_html(&self, node_id: i64, html: &str) -> Result<()> {
        self.send_command(
            "DOM.setOuterHTML",
            json!({
                "nodeId":    node_id,
                "outerHTML": html,
            }),
        )
        .await?;
        Ok(())
    }

    /// Remove a DOM node from the document.
    pub async fn remove_node(&self, node_id: i64) -> Result<()> {
        self.send_command("DOM.removeNode", json!({ "nodeId": node_id }))
            .await?;
        Ok(())
    }

    /// Call `function_declaration` with the object identified by `object_id` as `this`,
    /// returning its result by value.
    pub async fn call_function_on(
        &self,
        object_id: &str,
        function_declaration: &str,
    ) -> Result<serde_json::Value> {
        let result = self
            .send_command(
                "Runtime.callFunctionOn",
                json!({
                    "objectId":            object_id,
                    "functionDeclaration": function_declaration,
                    "returnByValue":       true,
                }),
            )
            .await?;
        Ok(result["result"]["value"].clone())
    }

    /// Navigate to `url`. Returns as soon as navigation starts, without waiting for
    /// the page to finish loading; use [`navigate_and_wait`](Self::navigate_and_wait)
    /// to block until `Page.loadEventFired`.
    ///
    /// A navigation Chrome refused (an unresolvable host, a refused connection, a
    /// blocked request) is a [`CdpError::Protocol`] carrying Chrome's `errorText`,
    /// not a successful result.
    pub async fn navigate(&self, url: &str) -> Result<NavigationResult> {
        let result = self
            .send_command("Page.navigate", json!({ "url": url }))
            .await?;
        navigation_outcome(url, serde_json::from_value(result)?)
    }

    /// Navigate to `url` and wait for `Page.loadEventFired`, up to `timeout_ms`.
    /// Requires the `"Page"` domain to be enabled.
    ///
    /// Fails the same way [`navigate`](Self::navigate) does, before waiting for a
    /// load event that a refused navigation would never fire.
    pub async fn navigate_and_wait(&self, url: &str, timeout_ms: u64) -> Result<NavigationResult> {
        let mut rx = self.subscribe_events();
        let nav = self.navigate(url).await?;
        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), async move {
            loop {
                match rx.recv().await {
                    Ok((m, _)) if m == "Page.loadEventFired" => return Ok(nav),
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => return Err(CdpError::Protocol("event channel closed".into())),
                }
            }
        })
        .await
        .map_err(|_| CdpError::Timeout)?
    }

    /// Override the viewport size and mobile emulation flag.
    pub async fn set_viewport(&self, width: i32, height: i32, mobile: bool) -> Result<()> {
        self.send_command(
            "Emulation.setDeviceMetricsOverride",
            json!({ "width": width, "height": height, "deviceScaleFactor": 1, "mobile": mobile }),
        )
        .await?;
        Ok(())
    }
}
