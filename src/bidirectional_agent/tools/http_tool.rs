//! Example HTTP Tool implementation.



use super::Tool;
use crate::bidirectional_agent::tool_executor::ToolError;
use async_trait::async_trait;
use reqwest::{Client, Method};
use serde_json::{json, Value};
use tokio::time::{timeout, Duration};

/// A tool for making HTTP requests.
#[derive(Clone)]
pub struct HttpTool {
    client: Client,
    default_timeout_ms: u64,
}

impl HttpTool {
    pub fn new() -> Self {
        // In a real implementation, configure client (proxies, certs) from agent config
        Self {
            client: Client::new(),
            default_timeout_ms: 10000, // 10 second default timeout
        }
    }
}

#[async_trait]
impl Tool for HttpTool {
    fn name(&self) -> &str {
        "http"
    }

    fn description(&self) -> &str {
        "Makes HTTP requests (GET, POST) to specified URLs."
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let url = params
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParams(self.name().to_string(), "Missing 'url' parameter".to_string())
            })?;

        let method_str = params
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET")
            .to_uppercase();

        let method = match Method::from_bytes(method_str.as_bytes()) {
            Ok(m) => m,
            Err(_) => {
                return Err(ToolError::InvalidParams(
                    self.name().to_string(),
                    format!("Invalid HTTP method: {}", method_str),
                ))
            }
        };

        let body = params.get("body").cloned(); // Can be string or JSON object
        let headers = params
            .get("headers")
            .and_then(|v| v.as_object())
            .cloned();

        println!("🌐 Making HTTP {} request to: {}", method, url);

        let mut request_builder = self.client.request(method, url);

        // Add headers
        if let Some(header_map) = headers {
            for (key, value) in header_map {
                if let Some(value_str) = value.as_str() {
                    request_builder = request_builder.header(&key, value_str);
                }
            }
        }

        // Add body for POST/PUT etc.
        if body.is_some() && (method_str == "POST" || method_str == "PUT" || method_str == "PATCH") {
            if let Some(body_val) = body {
                if body_val.is_string() {
                    request_builder = request_builder.body(body_val.as_str().unwrap().to_string());
                } else {
                    request_builder = request_builder.json(&body_val);
                }
            }
        }

        // Execute request with timeout
        let execution_timeout = Duration::from_millis(self.default_timeout_ms);
        match timeout(execution_timeout, request_builder.send()).await {
            Ok(Ok(response)) => {
                let status = response.status().as_u16();
                println!("  Response Status: {}", status);

                // Read response body as text (limit size)
                let body_text = match timeout(Duration::from_secs(5), response.text()).await {
                     Ok(Ok(text)) => {
                         if text.len() > 1024 { // Limit response size
                             format!("{}...", &text[..1024])
                         } else {
                             text
                         }
                     },
                     Ok(Err(e)) => format!("Error reading response body: {}", e),
                     Err(_) => "Timeout reading response body".to_string(),
                };

                Ok(json!({
                    "status_code": status,
                    "body": body_text // Return truncated body
                }))
            }
            Ok(Err(e)) => Err(ToolError::ExecutionFailed(
                self.name().to_string(),
                format!("HTTP request failed: {}", e),
            )),
            Err(_) => Err(ToolError::ExecutionFailed(
                self.name().to_string(),
                format!("HTTP request timed out after {}ms", self.default_timeout_ms),
            )),
        }
    }

    fn capabilities(&self) -> &[&'static str] {
        &["http_request", "web_access"] // Example capabilities
    }
}
