//! `Network` and `Fetch` domain commands (cookies, headers, request interception),
//! added to [`CdpClient`] here.

use std::collections::HashMap;

use serde_json::json;

use crate::client::CdpClient;
use crate::error::Result;

impl CdpClient {
    /// Set a cookie. `url` or `domain` is required by CDP to resolve which
    /// site the cookie belongs to.
    pub async fn set_cookie(
        &self,
        name: &str,
        value: &str,
        url: Option<&str>,
        domain: Option<&str>,
        path: Option<&str>,
    ) -> Result<()> {
        let mut params = json!({ "name": name, "value": value });
        if let Some(u) = url {
            params["url"] = json!(u);
        }
        if let Some(d) = domain {
            params["domain"] = json!(d);
        }
        if let Some(p) = path {
            params["path"] = json!(p);
        }
        self.send_command("Network.setCookie", params).await?;
        Ok(())
    }

    /// Delete cookies matching `name` (and optionally scoped to `url`).
    pub async fn delete_cookies(&self, name: &str, url: Option<&str>) -> Result<()> {
        let mut params = json!({ "name": name });
        if let Some(u) = url {
            params["url"] = json!(u);
        }
        self.send_command("Network.deleteCookies", params).await?;
        Ok(())
    }

    /// Send `headers` with every subsequent request from this target.
    pub async fn set_extra_headers(&self, headers: &HashMap<String, String>) -> Result<()> {
        self.send_command("Network.setExtraHTTPHeaders", json!({ "headers": headers }))
            .await?;
        Ok(())
    }

    /// Block requests whose URL matches any of `patterns` (CDP wildcard syntax, e.g.
    /// `"*.example.com/*"`).
    pub async fn block_urls(&self, patterns: &[&str]) -> Result<()> {
        self.send_command("Network.setBlockedURLs", json!({ "urls": patterns }))
            .await?;
        Ok(())
    }

    /// Fetch a response body by request id, decoding it if Chrome returned it base64-encoded.
    pub async fn get_response_body(&self, request_id: &str) -> Result<String> {
        let result = self
            .send_command(
                "Network.getResponseBody",
                json!({ "requestId": request_id }),
            )
            .await?;
        let body = result["body"].as_str().unwrap_or("").to_string();
        let encoded = result["base64Encoded"].as_bool().unwrap_or(false);
        if encoded {
            use base64::Engine;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(&body)
                .unwrap_or_default();
            Ok(String::from_utf8_lossy(&bytes).into_owned())
        } else {
            Ok(body)
        }
    }

    /// Enable request interception for URLs matching `url_patterns`. Matching requests
    /// pause and must be resolved with [`continue_request`](Self::continue_request) or
    /// [`fulfill_request`](Self::fulfill_request), delivered as `Fetch.requestPaused`
    /// events (see [`subscribe_events`](Self::subscribe_events)).
    pub async fn intercept_requests(&self, url_patterns: &[&str]) -> Result<()> {
        self.enable_domain("Fetch").await?;
        let patterns: Vec<_> = url_patterns
            .iter()
            .map(|p| json!({ "urlPattern": p, "requestStage": "Request" }))
            .collect();
        self.send_command("Fetch.enable", json!({ "patterns": patterns }))
            .await?;
        Ok(())
    }

    /// Let a paused request proceed unmodified.
    pub async fn continue_request(&self, request_id: &str) -> Result<()> {
        self.send_command("Fetch.continueRequest", json!({ "requestId": request_id }))
            .await?;
        Ok(())
    }

    /// Respond to a paused request with a synthetic response instead of letting it
    /// reach the network.
    pub async fn fulfill_request(
        &self,
        request_id: &str,
        status: u16,
        body: &str,
        content_type: &str,
    ) -> Result<()> {
        use base64::Engine;
        let body_b64 = base64::engine::general_purpose::STANDARD.encode(body);
        self.send_command(
            "Fetch.fulfillRequest",
            json!({
                "requestId":       request_id,
                "responseCode":    status,
                "body":            body_b64,
                "responseHeaders": [{ "name": "content-type", "value": content_type }],
            }),
        )
        .await?;
        Ok(())
    }
}
