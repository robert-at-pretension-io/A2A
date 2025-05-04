//! LLM Client Abstraction and Implementation

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest;
use serde_json::{json, Value};
use tracing::{debug, error, info, trace, instrument};

/// Simple LLM client interface
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, prompt: &str) -> Result<String>;
}

/// Simple implementation of an LLM client that delegates to Claude
pub struct ClaudeLlmClient {
    api_key: String,
    system_prompt: String,
}

impl ClaudeLlmClient {
    pub fn new(api_key: String, system_prompt: String) -> Self {
        Self {
            api_key,
            system_prompt,
        }
    }
}

#[async_trait]
impl LlmClient for ClaudeLlmClient {
    #[instrument(skip(self, prompt), fields(prompt_len = prompt.len()))]
    async fn complete(&self, prompt: &str) -> Result<String> {
        debug!("Sending request to Claude LLM.");
        trace!(?prompt, "LLM prompt content.");
        // Create a new client for the Claude API
        let client = reqwest::Client::new();
        // Prepare the request payload
        let payload = json!({
            "model": "claude-3-7-sonnet-20250219",
            "max_tokens": 60000,
            "system": self.system_prompt,
            "messages": [{
                "role": "user",
                "content": prompt
            }]
        });
        trace!(payload = %payload, "Claude API request payload.");

        // Send the request to the Claude API
        debug!("Posting request to Claude API endpoint.");
        let response = client.post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key) // API key is sensitive, not logged
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .context("Failed to send request to Claude API")?;
        debug!(status = %response.status(), "Received response from Claude API.");

        // Check if the request was successful
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!(%status, error_body = %error_text, "Claude API request failed.");
            return Err(anyhow!("Claude API error ({}): {}", status, error_text));
        }

        // Parse the response
        trace!("Parsing Claude API JSON response.");
        let response_json: Value = response.json().await
            .context("Failed to parse Claude API response")?;
        trace!(response_json = %response_json, "Parsed Claude API response.");

        // Extract the completion
        trace!("Extracting completion text from response.");
        let completion = response_json["content"][0]["text"].as_str()
            .ok_or_else(|| anyhow!("Failed to extract completion from response"))?;
        debug!(completion_len = completion.len(), "Extracted completion from Claude API.");
        trace!(completion = %completion, "Completion content.");

        Ok(completion.to_string())
    }
}
