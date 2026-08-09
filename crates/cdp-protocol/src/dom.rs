//! Reading the document tree.

use serde_json::json;

use crate::client::CdpClient;
use crate::error::{CdpError, Result};
use crate::types::DocumentNode;

impl CdpClient {
    /// Fetch the document's root node. Requires the `"DOM"` domain to be enabled.
    pub async fn get_document(&self) -> Result<DocumentNode> {
        let result = self
            .send_command("DOM.getDocument", json!({ "depth": 0 }))
            .await?;
        let root = result["root"].clone();
        if root.is_null() {
            return Err(CdpError::Protocol(
                "DOM.getDocument returned no root".into(),
            ));
        }
        Ok(serde_json::from_value(root)?)
    }

    /// Returns the matched `nodeId`, or `None` when nothing matches.
    /// CDP reports a missing match as `nodeId` 0; this surfaces that as `None`
    /// rather than a node id that looks valid.
    pub async fn query_selector(&self, node_id: i64, selector: &str) -> Result<Option<i64>> {
        let result = self
            .send_command(
                "DOM.querySelector",
                json!({ "nodeId": node_id, "selector": selector }),
            )
            .await?;
        Ok(match result["nodeId"].as_i64() {
            Some(id) if id > 0 => Some(id),
            _ => None,
        })
    }

    /// Serialize a node's outer HTML.
    pub async fn get_outer_html(&self, node_id: i64) -> Result<String> {
        let result = self
            .send_command("DOM.getOuterHTML", json!({ "nodeId": node_id }))
            .await?;
        Ok(result["outerHTML"].as_str().unwrap_or("").to_string())
    }
}
