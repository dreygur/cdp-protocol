use std::collections::HashMap;

use serde_json::json;

use crate::client::CdpClient;
use crate::error::Result;

impl CdpClient {
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

    pub async fn delete_cookies(&self, name: &str, url: Option<&str>) -> Result<()> {
        let mut params = json!({ "name": name });
        if let Some(u) = url {
            params["url"] = json!(u);
        }
        self.send_command("Network.deleteCookies", params).await?;
        Ok(())
    }

    pub async fn set_extra_headers(&self, headers: &HashMap<String, String>) -> Result<()> {
        self.send_command("Network.setExtraHTTPHeaders", json!({ "headers": headers }))
            .await?;
        Ok(())
    }

    pub async fn block_urls(&self, patterns: &[&str]) -> Result<()> {
        self.send_command("Network.setBlockedURLs", json!({ "urls": patterns }))
            .await?;
        Ok(())
    }

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

    pub async fn continue_request(&self, request_id: &str) -> Result<()> {
        self.send_command("Fetch.continueRequest", json!({ "requestId": request_id }))
            .await?;
        Ok(())
    }

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
