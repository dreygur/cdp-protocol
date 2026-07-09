//! Synchronous mirror of [`crate::client`] and [`crate::agent`], gated behind the
//! `blocking` feature. Each type owns its own single-threaded-caller Tokio runtime
//! internally, so no async runtime is required of the caller.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;
use tokio::runtime::Runtime;

use crate::agent::BrowserAction;
use crate::config::Config;
use crate::error::{CdpError, Result};
use crate::types::*;

fn build_rt() -> Result<Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(CdpError::Io)
}

/// Blocking counterpart of [`crate::client::CdpClient`]; every method here has the
/// same behavior as its async namesake there, called via `Runtime::block_on`.
pub struct CdpClient {
    inner: crate::client::CdpClient,
    rt: Arc<Runtime>,
}

impl CdpClient {
    pub fn connect(ws_url: &str) -> Result<Self> {
        let rt = Arc::new(build_rt()?);
        let inner = rt.block_on(crate::client::CdpClient::connect(ws_url))?;
        Ok(CdpClient { inner, rt })
    }

    pub fn connect_to_page(host: &str, port: u16) -> Result<Self> {
        let rt = Arc::new(build_rt()?);
        let inner = rt.block_on(crate::client::CdpClient::connect_to_page(host, port))?;
        Ok(CdpClient { inner, rt })
    }

    pub fn get_version(host: &str, port: u16) -> Result<BrowserVersion> {
        let rt = build_rt()?;
        rt.block_on(crate::client::CdpClient::get_version(host, port))
    }

    pub fn list_targets(host: &str, port: u16) -> Result<Vec<Target>> {
        let rt = build_rt()?;
        rt.block_on(crate::client::CdpClient::list_targets(host, port))
    }

    pub fn create_tab(host: &str, port: u16, url: Option<&str>) -> Result<Target> {
        let rt = build_rt()?;
        rt.block_on(crate::client::CdpClient::create_tab(host, port, url))
    }

    pub fn enable_domain(&self, domain: &str) -> Result<()> {
        self.rt.block_on(self.inner.enable_domain(domain))
    }

    pub fn navigate(&self, url: &str) -> Result<NavigationResult> {
        self.rt.block_on(self.inner.navigate(url))
    }

    pub fn navigate_and_wait(&self, url: &str, timeout_ms: u64) -> Result<NavigationResult> {
        self.rt
            .block_on(self.inner.navigate_and_wait(url, timeout_ms))
    }

    pub fn wait_for_event(&self, method: &str, timeout_ms: u64) -> Result<Value> {
        self.rt
            .block_on(self.inner.wait_for_event(method, timeout_ms))
    }

    pub fn eval(&self, expression: &str) -> Result<String> {
        self.rt.block_on(self.inner.eval(expression))
    }

    pub fn evaluate(&self, expression: &str) -> Result<EvaluateResult> {
        self.rt.block_on(self.inner.evaluate(expression))
    }

    pub fn get_document(&self) -> Result<DocumentNode> {
        self.rt.block_on(self.inner.get_document())
    }

    pub fn query_selector(&self, node_id: i64, selector: &str) -> Result<Option<i64>> {
        self.rt
            .block_on(self.inner.query_selector(node_id, selector))
    }

    pub fn get_outer_html(&self, node_id: i64) -> Result<String> {
        self.rt.block_on(self.inner.get_outer_html(node_id))
    }

    pub fn set_viewport(&self, width: i32, height: i32, mobile: bool) -> Result<()> {
        self.rt
            .block_on(self.inner.set_viewport(width, height, mobile))
    }

    pub fn screenshot(&self) -> Result<Vec<u8>> {
        self.rt.block_on(self.inner.screenshot())
    }

    pub fn screenshot_to_file(&self, path: &str) -> Result<()> {
        self.rt.block_on(self.inner.screenshot_to_file(path))
    }

    pub fn full_page_screenshot(&self) -> Result<Vec<u8>> {
        self.rt.block_on(self.inner.full_page_screenshot())
    }

    pub fn full_page_screenshot_to_file(&self, path: &str) -> Result<()> {
        self.rt
            .block_on(self.inner.full_page_screenshot_to_file(path))
    }

    pub fn get_cookies(&self) -> Result<Vec<Cookie>> {
        self.rt.block_on(self.inner.get_cookies())
    }

    pub fn set_cookie(
        &self,
        name: &str,
        value: &str,
        url: Option<&str>,
        domain: Option<&str>,
        path: Option<&str>,
    ) -> Result<()> {
        self.rt
            .block_on(self.inner.set_cookie(name, value, url, domain, path))
    }

    pub fn delete_cookies(&self, name: &str, url: Option<&str>) -> Result<()> {
        self.rt.block_on(self.inner.delete_cookies(name, url))
    }

    pub fn set_extra_headers(&self, headers: &HashMap<String, String>) -> Result<()> {
        self.rt.block_on(self.inner.set_extra_headers(headers))
    }

    pub fn block_urls(&self, patterns: &[&str]) -> Result<()> {
        self.rt.block_on(self.inner.block_urls(patterns))
    }

    pub fn get_response_body(&self, request_id: &str) -> Result<String> {
        self.rt.block_on(self.inner.get_response_body(request_id))
    }

    pub fn set_content(&self, html: &str) -> Result<()> {
        self.rt.block_on(self.inner.set_content(html))
    }

    pub fn print_to_pdf(&self, path: &str) -> Result<()> {
        self.rt.block_on(self.inner.print_to_pdf(path))
    }

    pub fn add_init_script(&self, source: &str) -> Result<String> {
        self.rt.block_on(self.inner.add_init_script(source))
    }

    pub fn set_user_agent(&self, ua: &str) -> Result<()> {
        self.rt.block_on(self.inner.set_user_agent(ua))
    }

    pub fn set_geolocation(&self, latitude: f64, longitude: f64, accuracy: f64) -> Result<()> {
        self.rt
            .block_on(self.inner.set_geolocation(latitude, longitude, accuracy))
    }

    pub fn set_offline(&self, offline: bool) -> Result<()> {
        self.rt.block_on(self.inner.set_offline(offline))
    }

    pub fn set_attribute(&self, node_id: i64, name: &str, value: &str) -> Result<()> {
        self.rt
            .block_on(self.inner.set_attribute(node_id, name, value))
    }

    pub fn set_outer_html(&self, node_id: i64, html: &str) -> Result<()> {
        self.rt.block_on(self.inner.set_outer_html(node_id, html))
    }

    pub fn remove_node(&self, node_id: i64) -> Result<()> {
        self.rt.block_on(self.inner.remove_node(node_id))
    }

    pub fn close(&self) -> Result<()> {
        self.rt.block_on(self.inner.close())
    }
}

/// Blocking counterpart of [`crate::agent::BrowserAgent`]; every method here has the
/// same behavior as its async namesake there, called via `Runtime::block_on`.
pub struct BrowserAgent {
    inner: crate::agent::BrowserAgent,
    rt: Arc<Runtime>,
}

impl BrowserAgent {
    pub fn connect(host: &str, port: u16) -> Result<Self> {
        let rt = Arc::new(build_rt()?);
        let inner = rt.block_on(crate::agent::BrowserAgent::connect(host, port))?;
        Ok(BrowserAgent { inner, rt })
    }

    pub fn connect_with_config(config: &Config) -> Result<Self> {
        let rt = Arc::new(build_rt()?);
        let inner = rt.block_on(crate::agent::BrowserAgent::connect_with_config(config))?;
        Ok(BrowserAgent { inner, rt })
    }

    pub fn execute(&self, action: BrowserAction) -> crate::agent::ActionResult {
        self.rt.block_on(self.inner.execute(action))
    }

    pub fn execute_many(&self, actions: Vec<BrowserAction>) -> Vec<crate::agent::ActionResult> {
        self.rt.block_on(self.inner.execute_many(actions))
    }

    pub fn execute_json(&self, json_str: &str) -> crate::agent::ActionResult {
        self.rt.block_on(self.inner.execute_json(json_str))
    }
}
