//! Anthropic Claude integration for LLM-based routing.
//! 
//! This module provides a concrete implementation of LLM routing
//! using Anthropic's Claude API.

use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use reqwest::Client;
use std::time::Duration;

/// Configuration for Claude API
#[derive(Debug, Clone)]
pub struct ClaudeConfig {
    /// API key for authentication
    pub api_key: String,
    
    /// Claude model to use
    pub model: String,
    
    /// Maximum tokens to generate
    pub max_tokens: u32,
    
    /// Temperature parameter (0.0-1.0)
    pub temperature: f32,
    
    /// Request timeout in seconds
    pub timeout_seconds: u64,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            api_key: "".to_string(),
            model: "claude-3-haiku-20240307".to_string(), // Faster model for routing
            max_tokens: 2048,
            temperature: 0.1, // Low temperature for more deterministic routing
            timeout_seconds: 30,
        }
    }
}

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
    config: ClaudeConfig,
    
    /// API URL
    api_url: String,
}

impl ClaudeClient {
    /// Creates a new Claude client.
    pub fn new(config: ClaudeConfig) -> Result<Self> {
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
        let mut config = ClaudeConfig::default();
        config.api_key = api_key;
        Self::new(config)
    }
    
    /// Sends a prompt to Claude and returns the completion.
    pub async fn complete(&self, prompt: &str) -> Result<String> {
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
        
        // Send request to Claude API
        let response = self.client
            .post(&self.api_url)
            .header("Content-Type", "application/json")
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Claude API")?;
        
        // Check for errors
        if !response.status().is_success() {
            let error_text = response.text().await
                .context("Failed to read error response from Claude API")?;
            return Err(anyhow::anyhow!("Claude API error: {}", error_text));
        }
        
        // Parse response
        let claude_response: ClaudeResponse = response.json()
            .await
            .context("Failed to parse Claude API response")?;
        
        // Extract text from response
        let result_text = claude_response.content.iter()
            .filter(|c| c.content_type == "text")
            .map(|c| c.text.clone())
            .collect::<Vec<_>>()
            .join("\n");
        
        Ok(result_text)
    }
    
    /// Sends a prompt to Claude and extracts JSON from the response.
    pub async fn complete_json<T: for<'de> Deserialize<'de>>(&self, prompt: &str) -> Result<T> {
        let completion = self.complete(prompt).await?;
        
        // Extract JSON from the completion
        let json_str = self.extract_json(&completion)
            .context("Failed to extract JSON from Claude response")?;
        
        // Parse JSON
        let result: T = serde_json::from_str(&json_str)
            .context("Failed to parse JSON from Claude response")?;
        
        Ok(result)
    }
    
    /// Extracts JSON from a text that might contain other content.
    fn extract_json(&self, text: &str) -> Result<String> {
        // Look for JSON between triple backticks
        if let Some(start) = text.find("```json") {
            if let Some(end) = text[start..].find("```") {
                // +7 to skip ```json
                return Ok(text[start + 7..start + end].trim().to_string());
            }
        }
        
        // Try to find JSON between regular backticks
        if let Some(start) = text.find('{') {
            if let Some(end) = text[start..].rfind('}') {
                return Ok(text[start..start + end + 1].to_string());
            }
        }
        
        // If no JSON found, return error
        Err(anyhow::anyhow!("No JSON found in Claude response"))
    }
}

// Re-export for easier access
pub use ClaudeClient as LlmClient;
pub use ClaudeConfig as LlmClientConfig;
