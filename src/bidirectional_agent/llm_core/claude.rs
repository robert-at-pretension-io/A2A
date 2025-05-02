//! Anthropic Claude integration.
//!
//! This module provides a concrete implementation of the LlmClient trait
//! using Anthropic's Claude API.

use std::time::Duration;
use anyhow::{Result, Context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use async_trait::async_trait;
use crate::bidirectional_agent::llm_core::{LlmClient, LlmConfig};

/// Request to Claude API
#[derive(Serialize)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    temperature: f32,
    messages: Vec<ClaudeMessage>,
}

/// Message format for Claude API
#[derive(Serialize, Clone)]
struct ClaudeMessage {
    role: String,
    content: Vec<ClaudeContent>,
}

/// Content part for Claude API
#[derive(Serialize, Clone)]
struct ClaudeContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

/// Response from Claude API
#[derive(Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeResponseContent>,
}

/// Content in Claude API response
#[derive(Deserialize)]
struct ClaudeResponseContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

/// Client for interacting with Claude API
pub struct ClaudeClient {
    /// HTTP client
    client: Client,
    
    /// Configuration
    config: LlmConfig,
    
    /// API URL
    api_url: String,
}

impl ClaudeClient {
    /// Creates a new Claude client.
    pub fn new(config: LlmConfig) -> Result<Self> {
        // Validate config
        if config.api_key.is_empty() {
            return Err(anyhow::anyhow!("API key cannot be empty"));
        }
        
        if config.model.is_empty() {
            return Err(anyhow::anyhow!("Model name cannot be empty"));
        }
        
        if config.max_tokens == 0 {
            return Err(anyhow::anyhow!("Max tokens must be greater than 0"));
        }
        
        if config.timeout_seconds == 0 {
            return Err(anyhow::anyhow!("Timeout seconds must be greater than 0"));
        }
        
        // Create HTTP client with timeout
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_seconds))
            .build()
            .context("Failed to create HTTP client")?;
        
        Ok(Self {
            client,
            config,
            api_url: "https://api.anthropic.com/v1/messages".to_string(),
        })
    }
    
    /// Creates a Claude client with default configuration.
    pub fn with_default_config(api_key: String) -> Result<Self> {
        let mut config = LlmConfig::default();
        config.api_key = api_key;
        Self::new(config)
    }
    
    /// Validates the API key with a minimal request
    pub async fn validate_api_key(&self) -> Result<bool> {
        // Create a minimal request that just checks if the API key is valid
        let request = ClaudeRequest {
            model: self.config.model.clone(),
            max_tokens: 1, // Minimal tokens
            temperature: 0.0,
            messages: vec![
                ClaudeMessage {
                    role: "user".to_string(),
                    content: vec![
                        ClaudeContent {
                            content_type: "text".to_string(),
                            text: "Hello".to_string(),
                        },
                    ],
                },
            ],
        };
        
        // Send request to Claude API
        let response = self.client
            .post(&self.api_url)
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await;
        
        // Check if request was successful
        match response {
            Ok(res) => {
                if res.status().is_success() {
                    Ok(true)
                } else if res.status().as_u16() == 401 {
                    Ok(false) // Invalid API key
                } else {
                    Err(anyhow::anyhow!("API returned unexpected status: {}", res.status()))
                }
            },
            Err(e) => Err(anyhow::anyhow!("Failed to validate API key: {}", e)),
        }
    }
    
    /// Get the current rate limit and usage information from the API
    pub async fn get_rate_limit_info(&self) -> Result<RateLimitInfo> {
        // Create a minimal request
        let request = ClaudeRequest {
            model: self.config.model.clone(),
            max_tokens: 1,
            temperature: 0.0,
            messages: vec![
                ClaudeMessage {
                    role: "user".to_string(),
                    content: vec![
                        ClaudeContent {
                            content_type: "text".to_string(),
                            text: "Hello".to_string(),
                        },
                    ],
                },
            ],
        };
        
        // Send request to Claude API
        let response = self.client
            .post(&self.api_url)
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await?;
        
        // Extract rate limit headers
        let rate_limit = RateLimitInfo {
            limit: response.headers()
                .get("x-ratelimit-limit")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0),
            remaining: response.headers()
                .get("x-ratelimit-remaining")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0),
            reset: response.headers()
                .get("x-ratelimit-reset")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0),
        };
        
        Ok(rate_limit)
    }
}

/// Rate limit information from the API
#[derive(Debug, Clone)]
pub struct RateLimitInfo {
    /// The total request limit
    pub limit: u32,
    /// The number of requests remaining
    pub remaining: u32,
    /// Time until the rate limit resets (in seconds)
    pub reset: u32,
}
}

#[async_trait]
impl LlmClient for ClaudeClient {
    /// Sends a prompt to Claude and returns the completion.
    async fn complete(&self, prompt: &str) -> Result<String> {
        const MAX_RETRIES: usize = 3;
        const RETRY_DELAY_MS: u64 = 1000;
        
        // Create request body
        let request = ClaudeRequest {
            model: self.config.model.clone(),
            max_tokens: self.config.max_tokens,
            temperature: self.config.temperature,
            messages: vec![
                ClaudeMessage {
                    role: "user".to_string(),
                    content: vec![
                        ClaudeContent {
                            content_type: "text".to_string(),
                            text: prompt.to_string(),
                        },
                    ],
                },
            ],
        };
        
        // Implement retry logic
        let mut last_error = None;
        
        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                println!("🔄 Retrying Claude API request (attempt {}/{})", attempt + 1, MAX_RETRIES);
                // Exponential backoff
                tokio::time::sleep(tokio::time::Duration::from_millis(RETRY_DELAY_MS * 2_u64.pow(attempt as u32))).await;
            }
            
            // Send request to Claude API
            let response = match self.client
                .post(&self.api_url)
                .header("Content-Type", "application/json")
                .header("x-api-key", &self.config.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&request)
                .send()
                .await {
                    Ok(resp) => resp,
                    Err(e) => {
                        last_error = Some(anyhow::anyhow!("Failed to send request to Claude API: {}", e));
                        
                        // Only retry on network errors, not client errors
                        if e.is_timeout() || e.is_connect() || e.is_request() {
                            continue;
                        } else {
                            return Err(last_error.unwrap());
                        }
                    }
                };
            
            // Check for errors
            if !response.status().is_success() {
                let status = response.status();
                let error_text = match response.text().await {
                    Ok(text) => text,
                    Err(e) => format!("Failed to read error response: {}", e),
                };
                
                let error = anyhow::anyhow!("Claude API error ({}): {}", status, error_text);
                last_error = Some(error.clone());
                
                // Retry on 429 (too many requests) and 5xx errors
                if status.as_u16() == 429 || status.is_server_error() {
                    continue;
                } else {
                    return Err(error);
                }
            }
            
            // Parse response
            let claude_response: ClaudeResponse = match response.json().await {
                Ok(resp) => resp,
                Err(e) => {
                    last_error = Some(anyhow::anyhow!("Failed to parse Claude API response: {}", e));
                    continue;
                }
            };
            
            // Extract text from response
            let result_text = claude_response.content.iter()
                .filter(|c| c.content_type == "text")
                .map(|c| c.text.clone())
                .collect::<Vec<_>>()
                .join("\n");
            
            // Check if result is empty
            if result_text.trim().is_empty() {
                last_error = Some(anyhow::anyhow!("Claude API returned empty response"));
                continue;
            }
            
            // Success!
            return Ok(result_text);
        }
        
        // If we reach here, all retries failed
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All retry attempts failed")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    
    // This test is ignored by default as it requires an API key
    #[tokio::test]
    #[ignore]
    async fn test_claude_client_complete() {
        // Only run this test if ANTHROPIC_API_KEY is set
        let api_key = match env::var("ANTHROPIC_API_KEY") {
            Ok(key) => key,
            Err(_) => return,
        };
        
        let config = LlmConfig {
            api_key,
            model: "claude-3-haiku-20240307".to_string(),
            max_tokens: 100,
            temperature: 0.0,
            timeout_seconds: 30,
        };
        
        let client = ClaudeClient::new(config).unwrap();
        let response = client.complete("Say hello world").await.unwrap();
        
        assert!(response.contains("hello") || response.contains("Hello"));
    }
    
    // Test JSON extraction functionality
    #[test]
    fn test_extract_json_direct() {
        let client = ClaudeClient {
            client: Client::new(),
            config: LlmConfig::default(),
            api_url: "".to_string(),
        };
        
        let text = r#"Here's a JSON object: {"key": "value"}"#;
        let json = client.extract_json(text).unwrap();
        assert_eq!(json, r#"{"key": "value"}"#);
    }
    
    #[test]
    fn test_extract_json_code_block() {
        let client = ClaudeClient {
            client: Client::new(),
            config: LlmConfig::default(),
            api_url: "".to_string(),
        };
        
        let text = r#"```json
{"key": "value"}
```"#;
        let json = client.extract_json(text).unwrap();
        assert_eq!(json, r#"{"key": "value"}"#);
    }
}