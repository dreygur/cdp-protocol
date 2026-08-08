//! The low-level CDP client, bound for JS.

use std::collections::HashMap;
use std::sync::Arc;

use cdp_driver::CdpClient as CoreClient;
use napi::bindgen_prelude::{Buffer, Result};
use napi_derive::napi;
use serde_json::Value;

use crate::errors::{json_err, to_napi};

/// Chrome DevTools Protocol client.
///
/// Connects to a Chrome/Chromium already listening with
/// `--remote-debugging-port`. It does not spawn the browser.
#[napi]
pub struct CdpClient {
    inner: Arc<CoreClient>,
}

#[napi]
impl CdpClient {
    /// Connect to a target WebSocket debugger URL.
    #[napi(factory)]
    pub async fn connect(ws_url: String) -> Result<CdpClient> {
        let inner = CoreClient::connect(&ws_url).await.map_err(to_napi)?;
        Ok(CdpClient {
            inner: Arc::new(inner),
        })
    }

    /// Discover the first `page` target on `host:port` and connect to it.
    #[napi(factory)]
    pub async fn connect_to_page(host: String, port: u16) -> Result<CdpClient> {
        let inner = CoreClient::connect_to_page(&host, port)
            .await
            .map_err(to_napi)?;
        Ok(CdpClient {
            inner: Arc::new(inner),
        })
    }

    /// `GET /json/version`  browser + protocol version.
    #[napi]
    pub async fn get_version(host: String, port: u16) -> Result<Value> {
        let v = CoreClient::get_version(&host, port)
            .await
            .map_err(to_napi)?;
        serde_json::to_value(v).map_err(json_err)
    }

    /// `GET /json/list`  all inspectable targets.
    #[napi]
    pub async fn list_targets(host: String, port: u16) -> Result<Value> {
        let t = CoreClient::list_targets(&host, port)
            .await
            .map_err(to_napi)?;
        serde_json::to_value(t).map_err(json_err)
    }

    /// `PUT /json/new`  open a new tab, optionally at `url`.
    #[napi]
    pub async fn create_tab(host: String, port: u16, url: Option<String>) -> Result<Value> {
        let t = CoreClient::create_tab(&host, port, url.as_deref())
            .await
            .map_err(to_napi)?;
        serde_json::to_value(t).map_err(json_err)
    }

    /// Enable a CDP domain, e.g. `"Page"`, `"Runtime"`, `"DOM"`, `"Network"`.
    #[napi]
    pub async fn enable_domain(&self, domain: String) -> Result<()> {
        self.inner.enable_domain(&domain).await.map_err(to_napi)
    }

    /// Navigate to `url`; returns the CDP frameId.
    #[napi]
    pub async fn navigate(&self, url: String) -> Result<String> {
        let inner = self.inner.clone();
        Ok(inner.navigate(&url).await.map_err(to_napi)?.frame_id)
    }

    /// Navigate and resolve once `Page.loadEventFired` arrives (needs `Page` enabled).
    #[napi]
    pub async fn navigate_and_wait(&self, url: String, timeout_ms: i64) -> Result<String> {
        let inner = self.inner.clone();
        let r = inner
            .navigate_and_wait(&url, timeout_ms as u64)
            .await
            .map_err(to_napi)?;
        Ok(r.frame_id)
    }

    /// Evaluate a JS expression, returning the value coerced to a string.
    #[napi]
    pub async fn eval(&self, expression: String) -> Result<String> {
        let inner = self.inner.clone();
        inner.eval(&expression).await.map_err(to_napi)
    }

    /// Evaluate a JS expression, returning the full result as a JSON value.
    #[napi]
    pub async fn evaluate(&self, expression: String) -> Result<Value> {
        let inner = self.inner.clone();
        let r = inner.evaluate(&expression).await.map_err(to_napi)?;
        Ok(r.result.value.unwrap_or(Value::Null))
    }

    /// Wait for a CDP event `method`, returning its params (needs the domain enabled).
    #[napi]
    pub async fn wait_for_event(&self, method: String, timeout_ms: i64) -> Result<Value> {
        let inner = self.inner.clone();
        inner
            .wait_for_event(&method, timeout_ms as u64)
            .await
            .map_err(to_napi)
    }

    /// `DOM.querySelector` from the document root; returns the matched nodeId,
    /// or `null` when nothing matches.
    #[napi]
    pub async fn query_selector(&self, selector: String) -> Result<Option<i64>> {
        let inner = self.inner.clone();
        let doc = inner.get_document().await.map_err(to_napi)?;
        inner
            .query_selector(doc.node_id, &selector)
            .await
            .map_err(to_napi)
    }

    /// `DOM.getOuterHTML` for a nodeId.
    #[napi]
    pub async fn get_outer_html(&self, node_id: i64) -> Result<String> {
        let inner = self.inner.clone();
        inner.get_outer_html(node_id).await.map_err(to_napi)
    }

    /// PNG screenshot of the current viewport.
    #[napi]
    pub async fn screenshot(&self) -> Result<Buffer> {
        let inner = self.inner.clone();
        Ok(inner.screenshot().await.map_err(to_napi)?.into())
    }

    /// Write a viewport PNG screenshot to `path`.
    #[napi]
    pub async fn screenshot_to_file(&self, path: String) -> Result<()> {
        let inner = self.inner.clone();
        inner.screenshot_to_file(&path).await.map_err(to_napi)
    }

    /// Full-page PNG screenshot.
    #[napi]
    pub async fn full_page_screenshot(&self) -> Result<Buffer> {
        let inner = self.inner.clone();
        Ok(inner.full_page_screenshot().await.map_err(to_napi)?.into())
    }

    /// Write a full-page PNG screenshot to `path`.
    #[napi]
    pub async fn full_page_screenshot_to_file(&self, path: String) -> Result<()> {
        let inner = self.inner.clone();
        inner
            .full_page_screenshot_to_file(&path)
            .await
            .map_err(to_napi)
    }

    /// Override device metrics (viewport).
    #[napi]
    pub async fn set_viewport(&self, width: i32, height: i32, mobile: bool) -> Result<()> {
        let inner = self.inner.clone();
        inner
            .set_viewport(width, height, mobile)
            .await
            .map_err(to_napi)
    }

    /// `DOM.getDocument` root node.
    #[napi]
    pub async fn get_document(&self) -> Result<Value> {
        let inner = self.inner.clone();
        let doc = inner.get_document().await.map_err(to_napi)?;
        serde_json::to_value(doc).map_err(json_err)
    }

    // --- Network ---------------------------------------------------------

    /// `Network.getCookies`.
    #[napi]
    pub async fn get_cookies(&self) -> Result<Value> {
        let inner = self.inner.clone();
        let cookies = inner.get_cookies().await.map_err(to_napi)?;
        serde_json::to_value(cookies).map_err(json_err)
    }

    /// `Network.setCookie`.
    #[napi]
    pub async fn set_cookie(
        &self,
        name: String,
        value: String,
        url: Option<String>,
        domain: Option<String>,
        path: Option<String>,
    ) -> Result<()> {
        let inner = self.inner.clone();
        inner
            .set_cookie(
                &name,
                &value,
                url.as_deref(),
                domain.as_deref(),
                path.as_deref(),
            )
            .await
            .map_err(to_napi)
    }

    /// `Network.deleteCookies`.
    #[napi]
    pub async fn delete_cookies(&self, name: String, url: Option<String>) -> Result<()> {
        let inner = self.inner.clone();
        inner
            .delete_cookies(&name, url.as_deref())
            .await
            .map_err(to_napi)
    }

    /// `Network.setExtraHTTPHeaders`.
    #[napi]
    pub async fn set_extra_headers(&self, headers: HashMap<String, String>) -> Result<()> {
        let inner = self.inner.clone();
        inner.set_extra_headers(&headers).await.map_err(to_napi)
    }

    /// `Network.setBlockedURLs`.
    #[napi]
    pub async fn block_urls(&self, patterns: Vec<String>) -> Result<()> {
        let inner = self.inner.clone();
        let refs: Vec<&str> = patterns.iter().map(String::as_str).collect();
        inner.block_urls(&refs).await.map_err(to_napi)
    }

    /// `Network.getResponseBody` (base64 payloads are decoded to text).
    #[napi]
    pub async fn get_response_body(&self, request_id: String) -> Result<String> {
        let inner = self.inner.clone();
        inner.get_response_body(&request_id).await.map_err(to_napi)
    }

    /// Enable `Fetch` request interception for the given URL patterns.
    #[napi]
    pub async fn intercept_requests(&self, url_patterns: Vec<String>) -> Result<()> {
        let inner = self.inner.clone();
        let refs: Vec<&str> = url_patterns.iter().map(String::as_str).collect();
        inner.intercept_requests(&refs).await.map_err(to_napi)
    }

    /// `Fetch.continueRequest`.
    #[napi]
    pub async fn continue_request(&self, request_id: String) -> Result<()> {
        let inner = self.inner.clone();
        inner.continue_request(&request_id).await.map_err(to_napi)
    }

    /// `Fetch.fulfillRequest` with a canned response body.
    #[napi]
    pub async fn fulfill_request(
        &self,
        request_id: String,
        status: u16,
        body: String,
        content_type: String,
    ) -> Result<()> {
        let inner = self.inner.clone();
        inner
            .fulfill_request(&request_id, status, &body, &content_type)
            .await
            .map_err(to_napi)
    }

    // --- Page / emulation / DOM mutation ---------------------------------

    /// Replace the document HTML (`Page.setDocumentContent`).
    #[napi]
    pub async fn set_content(&self, html: String) -> Result<()> {
        let inner = self.inner.clone();
        inner.set_content(&html).await.map_err(to_napi)
    }

    /// Print the page to a PDF file (`Page.printToPDF`).
    #[napi]
    pub async fn print_to_pdf(&self, path: String) -> Result<()> {
        let inner = self.inner.clone();
        inner.print_to_pdf(&path).await.map_err(to_napi)
    }

    /// Register a script to run on every new document; returns its identifier.
    #[napi]
    pub async fn add_init_script(&self, source: String) -> Result<String> {
        let inner = self.inner.clone();
        inner.add_init_script(&source).await.map_err(to_napi)
    }

    /// Remove a previously registered init script.
    #[napi]
    pub async fn remove_init_script(&self, identifier: String) -> Result<()> {
        let inner = self.inner.clone();
        inner.remove_init_script(&identifier).await.map_err(to_napi)
    }

    /// Override the User-Agent string.
    #[napi]
    pub async fn set_user_agent(&self, ua: String) -> Result<()> {
        let inner = self.inner.clone();
        inner.set_user_agent(&ua).await.map_err(to_napi)
    }

    /// Override geolocation.
    #[napi]
    pub async fn set_geolocation(
        &self,
        latitude: f64,
        longitude: f64,
        accuracy: f64,
    ) -> Result<()> {
        let inner = self.inner.clone();
        inner
            .set_geolocation(latitude, longitude, accuracy)
            .await
            .map_err(to_napi)
    }

    /// Toggle offline network emulation.
    #[napi]
    pub async fn set_offline(&self, offline: bool) -> Result<()> {
        let inner = self.inner.clone();
        inner.set_offline(offline).await.map_err(to_napi)
    }

    /// `DOM.setAttributeValue`.
    #[napi]
    pub async fn set_attribute(&self, node_id: i64, name: String, value: String) -> Result<()> {
        let inner = self.inner.clone();
        inner
            .set_attribute(node_id, &name, &value)
            .await
            .map_err(to_napi)
    }

    /// `DOM.setOuterHTML`.
    #[napi]
    pub async fn set_outer_html(&self, node_id: i64, html: String) -> Result<()> {
        let inner = self.inner.clone();
        inner.set_outer_html(node_id, &html).await.map_err(to_napi)
    }

    /// `DOM.removeNode`.
    #[napi]
    pub async fn remove_node(&self, node_id: i64) -> Result<()> {
        let inner = self.inner.clone();
        inner.remove_node(node_id).await.map_err(to_napi)
    }

    /// `Runtime.callFunctionOn` for a remote object id; returns the JSON value.
    #[napi]
    pub async fn call_function_on(
        &self,
        object_id: String,
        function_declaration: String,
    ) -> Result<Value> {
        let inner = self.inner.clone();
        inner
            .call_function_on(&object_id, &function_declaration)
            .await
            .map_err(to_napi)
    }

    /// Close the current tab.
    #[napi]
    pub async fn close(&self) -> Result<()> {
        let inner = self.inner.clone();
        inner.close().await.map_err(to_napi)
    }
}
