//! LLM Client Abstraction and Implementation

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest;
use serde_json::{json, Value};
use tracing::{debug, error, /* info, */ trace, instrument}; // Removed info

/// Simple LLM client interface
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Generates a text completion for the given prompt.
    async fn complete(&self, prompt_text: &str, system_prompt_override: Option<&str>) -> Result<String>;

    /// Generates a structured JSON completion for the given prompt and schema.
    async fn complete_structured(
        &self,
        prompt_text: &str,
        system_prompt_override: Option<&str>,
        output_schema: Value,
    ) -> Result<Value>;
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
    #[instrument(skip(self, prompt_text, system_prompt_override), fields(prompt_len = prompt_text.len()))]
    async fn complete(&self, prompt_text: &str, system_prompt_override: Option<&str>) -> Result<String> {
        debug!("Sending request to Claude LLM for text completion.");
        trace!(?prompt_text, "LLM prompt content.");
        let client = reqwest::Client::new();
        let system = system_prompt_override.unwrap_or(&self.system_prompt);
        let payload = json!({
            "model": "claude-3-haiku-20240307", // Using a faster model for general completion
            "max_tokens": 4096, // Adjusted max_tokens
            "system": system,
            "messages": [{
                "role": "user",
                "content": prompt_text
            }]
        });
        trace!(payload = %payload, system_prompt = %system, "Claude API request payload.");

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

    #[instrument(skip(self, prompt_text, system_prompt_override, output_schema), fields(prompt_len = prompt_text.len()))]
    async fn complete_structured(
        &self,
        prompt_text: &str,
        system_prompt_override: Option<&str>,
        _output_schema: Value, // Claude doesn't directly use the schema in the API call
    ) -> Result<Value> {
        debug!("Attempting structured completion with Claude (text-based).");
        // For Claude, we'll get a text completion and try to parse it as JSON.
        // The prompt itself should guide Claude to produce a JSON string.
        let text_response = self.complete(prompt_text, system_prompt_override).await?;
        trace!(%text_response, "Received text response from Claude for structured attempt.");

        // Attempt to parse the text response as JSON
        // Use the helper from task_router to extract JSON even if there's surrounding text.
        let json_str = crate::bidirectional::task_router::extract_json_from_text(&text_response);
        
        serde_json::from_str(&json_str)
            .map_err(|e| {
                error!(error = %e, raw_response = %text_response, extracted_json = %json_str, "Failed to parse Claude's text response as JSON.");
                anyhow!("Claude response was not valid JSON: {}. Response: '{}'", e, text_response)
            })
    }
}
