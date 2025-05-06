//! LLM Client Abstraction and Implementation

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest;
use serde_json::{json, Value};
use std::any::Any;
use tracing::{debug, error, instrument, /* info, */ trace}; // Removed info

/// Simple LLM client interface
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Generates a text completion for the given prompt.
    async fn complete(
        &self,
        prompt_text: &str,
        system_prompt_override: Option<&str>,
    ) -> Result<String>;

    /// Generates a structured JSON completion for the given prompt and schema.
    async fn complete_structured(
        &self,
        prompt_text: &str,
        system_prompt_override: Option<&str>,
        output_schema: Value,
    ) -> Result<Value>;
    
    /// Allow downcasting to concrete types
    fn as_any(&self) -> &dyn std::any::Any;
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
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    #[instrument(skip(self, prompt_text, system_prompt_override), fields(prompt_len = prompt_text.len()))]
    async fn complete(
        &self,
        prompt_text: &str,
        system_prompt_override: Option<&str>,
    ) -> Result<String> {
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
        let response = client
            .post("https://api.anthropic.com/v1/messages")
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
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!(%status, error_body = %error_text, "Claude API request failed.");
            return Err(anyhow!("Claude API error ({}): {}", status, error_text));
        }

        // Parse the response
        trace!("Parsing Claude API JSON response.");
        let response_json: Value = response
            .json()
            .await
            .context("Failed to parse Claude API response")?;
        trace!(response_json = %response_json, "Parsed Claude API response.");

        // Extract the completion
        trace!("Extracting completion text https://www.reddit.com/r/ArtificialInteligence/comments/1kfhh13/free_will_alignment_prompt/from response.");
        let completion = response_json["content"][0]["text"]
            .as_str()
            .ok_or_else(|| anyhow!("Failed to extract completion from response"))?;
        debug!(
            completion_len = completion.len(),
            "Extracted completion from Claude API."
        );
        trace!(completion = %completion, "Completion content.");

        Ok(completion.to_string())
    }

    #[instrument(skip(self, prompt_text, system_prompt_override, _output_schema), fields(prompt_len = prompt_text.len()))]
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

/// LLM client for Google's Gemini models
pub struct GeminiLlmClient {
    api_key: String,
    model_id: String,
    api_endpoint: String, // Base endpoint, e.g., "https://generativelanguage.googleapis.com/v1beta/models"
    system_prompt: String, // General system prompt, can be overridden
}

impl GeminiLlmClient {
    pub fn new(
        api_key: String,
        model_id: String,
        api_endpoint: String,
        system_prompt: String,
    ) -> Self {
        Self {
            api_key,
            model_id,
            api_endpoint,
            system_prompt,
        }
    }
    
}

#[async_trait]
impl LlmClient for GeminiLlmClient {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    #[instrument(skip(self, system_prompt_override), fields(prompt_len = prompt_text.len()))]
    async fn complete(
        &self,
        prompt_text: &str,
        system_prompt_override: Option<&str>,
    ) -> Result<String> {
        debug!("Sending request to Gemini LLM for text completion.");
        // For text completion, we use complete_structured with a simple text schema.
        let schema = json!({
            "type": "object",
            "properties": {
                "response": {
                    "type": "string"
                }
            },
            "required": ["response"]
        });

        let structured_response = self
            .complete_structured(prompt_text, system_prompt_override, schema)
            .await?;

        structured_response.get("response")
            .and_then(Value::as_str)
            .map(String::from)
            .ok_or_else(|| anyhow!("Gemini structured response for text completion did not contain a 'response' string field."))
    }

    #[instrument(skip(self, _output_schema), fields(prompt_len = prompt_text.len(), system_prompt_override = ?system_prompt_override))]
    async fn complete_structured(
        &self,
        prompt_text: &str,
        system_prompt_override: Option<&str>,
        _output_schema: Value, // Schema is no longer sent to the API
    ) -> Result<Value> {
        debug!("Sending request to Gemini LLM for structured (JSON) completion.");
        trace!(
            ?prompt_text,
            //?output_schema, // Schema not logged as it's not used directly in the API call
            "LLM prompt content. Expecting JSON output based on prompt."
        );

        let client = reqwest::Client::new();
        let system_instruction = system_prompt_override.unwrap_or(&self.system_prompt);

        // API endpoint for non-streaming generation
        let generate_api = "generateContent";
        let url = format!(
            "{}/{}:{}?key={}",
            self.api_endpoint, self.model_id, generate_api, self.api_key
        );
        trace!(%url, "Gemini API URL.");

        let mut contents = Vec::new();

        // System instructions are typically provided as part of the initial user/model turns
        if !system_instruction.is_empty() {
            contents.push(json!({
                "role": "user",
                "parts": [{"text": system_instruction}]
            }));
            contents.push(json!({
                "role": "model",
                "parts": [{"text": "Understood. I will respond according to the provided schema and instructions."}]
            }));
        }

        // Add the main user prompt
        contents.push(json!({
            "role": "user",
            "parts": [{"text": prompt_text}]
        }));

        let payload = json!({
            "contents": contents,
            "generationConfig": {
                "responseMimeType": "application/json",
                // "responseSchema": output_schema, // Schema is no longer sent
            }
        });
        trace!(payload = %payload, "Gemini API request payload.");

        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .context("Failed to send request to Gemini API")?;
        debug!(status = %response.status(), "Received response from Gemini API.");

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!(%status, error_body = %error_text, "Gemini API request failed.");
            return Err(anyhow!("Gemini API error ({}): {}", status, error_text));
        }

        let response_json: Value = response
            .json()
            .await
            .context("Failed to parse Gemini API response as JSON")?;
        trace!(response_json = %response_json, "Parsed Gemini API response.");

        // Extract the structured JSON from the response
        // Gemini's response for generateContent with JSON schema is typically under `candidates[0].content.parts[0].text`
        // where `text` itself is the JSON string.
        if let Some(text_json_str) = response_json
            .get("candidates")
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
            .and_then(|cand| cand.get("content"))
            .and_then(|cont| cont.get("parts"))
            .and_then(Value::as_array)
            .and_then(|parts_arr| parts_arr.first())
            .and_then(|part| part.get("text"))
            .and_then(Value::as_str)
        {
            return serde_json::from_str(text_json_str)
                .map_err(|e| {
                    error!(error = %e, raw_json_text = %text_json_str, "Failed to parse JSON text from Gemini response part.");
                    anyhow!("Gemini response part was not valid JSON: {}. Content: '{}'", e, text_json_str)
                });
        }

        error!(full_response = %response_json, "Could not extract structured JSON from Gemini response. Unexpected format.");
        Err(anyhow!(
            "Failed to extract structured JSON from Gemini response. Response format unexpected."
        ))
    }
}
