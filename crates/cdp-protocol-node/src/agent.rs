//! The high-level action runner, bound for JS.

use std::sync::Arc;

use cdp_driver::{BrowserAgent as CoreAgent, Config as CoreConfig};
use napi::bindgen_prelude::Result;
use napi_derive::napi;
use serde_json::Value;

use crate::action_result::{core_result, ActionResult};
use crate::config::Config;
use crate::errors::{json_err, to_napi};

/// High-level action runner. Enables `Page`, `Runtime`, `DOM`, `Network` on connect.
///
/// Actions are plain objects, e.g. `{ action: 'navigate', url: 'https://...' }`.
/// See the README for the full action list.
#[napi]
pub struct BrowserAgent {
    inner: Arc<CoreAgent>,
}

#[napi]
impl BrowserAgent {
    /// Connect to the first `page` target on `host:port` and enable the core domains.
    #[napi(factory)]
    pub async fn connect(host: String, port: u16) -> Result<BrowserAgent> {
        let inner = CoreAgent::connect(&host, port).await.map_err(to_napi)?;
        Ok(BrowserAgent {
            inner: Arc::new(inner),
        })
    }

    /// Connect using a [`Config`] (also applies the viewport).
    #[napi(factory)]
    pub async fn connect_with_config(config: Config) -> Result<BrowserAgent> {
        let core: CoreConfig = config.into();
        let inner = CoreAgent::connect_with_config(&core)
            .await
            .map_err(to_napi)?;
        Ok(BrowserAgent {
            inner: Arc::new(inner),
        })
    }

    /// Run one action given as an object, e.g. `{ action: 'navigate', url }`.
    #[napi]
    pub async fn execute(&self, action: Value) -> Result<ActionResult> {
        let inner = self.inner.clone();
        let json = serde_json::to_string(&action).map_err(json_err)?;
        Ok(core_result(inner.execute_json(&json).await))
    }

    /// Run a JSON string action (`{ "action": "navigate", "url": "..." }`).
    #[napi]
    pub async fn execute_json(&self, json: String) -> Result<ActionResult> {
        let inner = self.inner.clone();
        Ok(core_result(inner.execute_json(&json).await))
    }

    /// Run an array of action objects sequentially.
    #[napi]
    pub async fn execute_many(&self, actions: Vec<Value>) -> Result<Vec<ActionResult>> {
        let inner = self.inner.clone();
        let mut out = Vec::with_capacity(actions.len());
        for a in actions {
            let json = serde_json::to_string(&a).map_err(json_err)?;
            out.push(core_result(inner.execute_json(&json).await));
        }
        Ok(out)
    }

    /// Convenience: navigate.
    #[napi]
    pub async fn navigate(&self, url: String) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "navigate", "url": url }))
            .await
    }

    /// Convenience: click a selector.
    #[napi]
    pub async fn click(&self, selector: String) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "click", "selector": selector }))
            .await
    }

    /// Convenience: fill an input.
    #[napi]
    pub async fn fill(&self, selector: String, value: String) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "fill", "selector": selector, "value": value }))
            .await
    }

    /// Convenience: press a key.
    #[napi]
    pub async fn press_key(&self, key: String) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "press_key", "key": key }))
            .await
    }

    /// Convenience: page title.
    #[napi]
    pub async fn get_title(&self) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "get_title" })).await
    }

    /// Convenience: visible text.
    #[napi]
    pub async fn get_text(&self) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "get_text" })).await
    }

    /// Convenience: `[{ href, text }]` for every anchor.
    #[napi]
    pub async fn get_links(&self) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "get_links" })).await
    }

    /// Convenience: does a selector match?
    #[napi]
    pub async fn exists(&self, selector: String) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "exists", "selector": selector }))
            .await
    }

    /// Convenience: wait until a selector appears or timeout.
    #[napi]
    pub async fn wait_for_selector(
        &self,
        selector: String,
        timeout_ms: i64,
    ) -> Result<ActionResult> {
        self.run(serde_json::json!({
            "action": "wait_for_selector",
            "selector": selector,
            "timeout_ms": timeout_ms,
        }))
        .await
    }

    /// Convenience: full-page screenshot to `path` (returns byte count if omitted).
    #[napi]
    pub async fn screenshot(&self, path: Option<String>) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "screenshot", "path": path }))
            .await
    }

    /// Convenience: evaluate a JS expression, return its JSON value.
    #[napi]
    pub async fn evaluate(&self, expression: String) -> Result<ActionResult> {
        self.run(serde_json::json!({ "action": "evaluate", "expression": expression }))
            .await
    }

    /// Close the underlying tab.
    #[napi]
    pub async fn close(&self) -> Result<()> {
        let inner = self.inner.clone();
        inner.close().await.map_err(to_napi)
    }

    async fn run(&self, action: Value) -> Result<ActionResult> {
        let inner = self.inner.clone();
        let json = serde_json::to_string(&action).map_err(json_err)?;
        Ok(core_result(inner.execute_json(&json).await))
    }
}
