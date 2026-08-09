//! Capturing the page as a PNG.

use base64::Engine;
use serde_json::{json, Value};

use crate::client::CdpClient;
use crate::error::{CdpError, Result};

/// Used when the page reports no scroll size of its own, and as a floor so a
/// full-page capture is never smaller than an ordinary viewport.
const FALLBACK_VIEWPORT_WIDTH: i32 = 1920;

/// The height counterpart of [`FALLBACK_VIEWPORT_WIDTH`].
const FALLBACK_VIEWPORT_HEIGHT: i32 = 1200;

/// Decode the base64 payload a screenshot command answers with.
fn png_bytes_from(result: &Value) -> Result<Vec<u8>> {
    let data = result["data"]
        .as_str()
        .ok_or_else(|| CdpError::Protocol("screenshot response has no data".into()))?;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|e| CdpError::Protocol(e.to_string()))
}

impl CdpClient {
    /// Capture a PNG screenshot of the current viewport.
    pub async fn screenshot(&self) -> Result<Vec<u8>> {
        let result = self
            .send_command(
                "Page.captureScreenshot",
                json!({ "format": "png", "fromSurface": true }),
            )
            .await?;
        png_bytes_from(&result)
    }

    /// [`screenshot`](Self::screenshot), written directly to `path`.
    pub async fn screenshot_to_file(&self, path: &str) -> Result<()> {
        tokio::fs::write(path, self.screenshot().await?).await?;
        Ok(())
    }

    /// Capture a PNG screenshot of the full page, resizing the viewport to the
    /// page's scroll size first (restoring it is the caller's responsibility).
    pub async fn full_page_screenshot(&self) -> Result<Vec<u8>> {
        let size = self
            .evaluate(
                "(() => ({ \
                w: Math.max(document.body.scrollWidth, document.documentElement.scrollWidth), \
                h: Math.max(document.body.scrollHeight, document.documentElement.scrollHeight) \
            }))()",
            )
            .await?;

        let dims = size.result.value.as_ref();
        let w = dims
            .and_then(|v| v["w"].as_i64())
            .unwrap_or(FALLBACK_VIEWPORT_WIDTH as i64) as i32;
        let h = dims
            .and_then(|v| v["h"].as_i64())
            .unwrap_or(FALLBACK_VIEWPORT_HEIGHT as i64) as i32;
        self.set_viewport(
            w.max(FALLBACK_VIEWPORT_WIDTH),
            h.max(FALLBACK_VIEWPORT_HEIGHT),
            false,
        )
        .await?;

        let result = self
            .send_command(
                "Page.captureScreenshot",
                json!({
                    "format": "png",
                    "captureBeyondViewport": true,
                    "fromSurface": true,
                }),
            )
            .await?;
        png_bytes_from(&result)
    }

    /// [`full_page_screenshot`](Self::full_page_screenshot), written directly to `path`.
    pub async fn full_page_screenshot_to_file(&self, path: &str) -> Result<()> {
        tokio::fs::write(path, self.full_page_screenshot().await?).await?;
        Ok(())
    }
}
