//! `Page`, `Emulation`, and `DOM` mutation commands, added to [`CdpClient`] here.

use serde_json::json;

use crate::client::CdpClient;
use crate::error::Result;

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
}
