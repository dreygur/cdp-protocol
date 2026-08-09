//! Finding a browser and its targets over Chrome's HTTP endpoint.
//!
//! These are the only calls in the crate that speak HTTP rather than CDP: they
//! are how a caller gets a WebSocket URL to open a session with in the first
//! place.

use crate::client::CdpClient;
use crate::error::{CdpError, Result};
use crate::types::{BrowserVersion, Target};

impl CdpClient {
    /// `GET /json/version` on Chrome's debugging HTTP endpoint.
    pub async fn get_version(host: &str, port: u16) -> Result<BrowserVersion> {
        let url = format!("http://{host}:{port}/json/version");
        Ok(reqwest::get(&url).await?.json().await?)
    }

    /// `GET /json/list`: enumerate debuggable targets (tabs, workers, ...).
    pub async fn list_targets(host: &str, port: u16) -> Result<Vec<Target>> {
        let url = format!("http://{host}:{port}/json/list");
        Ok(reqwest::get(&url).await?.json().await?)
    }

    /// Connect to the first target of type `"page"` reported by [`list_targets`](Self::list_targets).
    /// Returns [`CdpError::NoTarget`] if none exists.
    pub async fn connect_to_page(host: &str, port: u16) -> Result<Self> {
        let targets = Self::list_targets(host, port).await?;
        let page = targets
            .into_iter()
            .find(|t| t.target_type == "page")
            .ok_or(CdpError::NoTarget)?;
        let ws_url = page
            .web_socket_debugger_url
            .ok_or_else(|| CdpError::InvalidUrl("target has no debugger URL".into()))?;
        Self::connect(&ws_url).await
    }

    /// Open a new tab, optionally navigating it to `url` immediately. Uses `PUT
    /// /json/new` since modern Chrome rejects `GET` for this endpoint.
    pub async fn create_tab(host: &str, port: u16, url: Option<&str>) -> Result<Target> {
        // Chrome requires PUT for /json/new (GET returns 405 in modern versions)
        let endpoint = match url {
            Some(u) => format!("http://{host}:{port}/json/new?{u}"),
            None => format!("http://{host}:{port}/json/new"),
        };
        Ok(reqwest::Client::new()
            .put(&endpoint)
            .send()
            .await?
            .json()
            .await?)
    }
}
