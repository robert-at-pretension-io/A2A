//! Core LLM client interfaces and implementations.
//!
//! This module provides a unified interface for interacting with large language
//! models (LLMs) like Claude, GPT, etc. It defines a common trait (`LlmClient`)
//! that all LLM implementations should implement, along with utilities for
//! prompt management and response parsing.

use std::time::Duration;
use anyhow::{Result, Context};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Serialize, Deserialize};

/// Common trait for all LLM clients
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Simple text completion - send a prompt and get a text response
    async fn complete(&self, prompt: &str) -> Result<String>;
    
    /// Structured JSON completion - send a prompt and parse response as JSON
    async fn complete_json<T: DeserializeOwned + Send>(&self, prompt: &str) -> Result<T> {
        let completion = self.complete(prompt).await?;
        
        // Extract JSON from the completion
        let json_str = self.extract_json(&completion)
            .context("Failed to extract JSON from LLM response")?;
        
        // Parse JSON
        let result: T = serde_json::from_str(&json_str)
            .context("Failed to parse JSON from LLM response")?;
        
        Ok(result)
    }
    
    /// Extract JSON from a text that might contain other content
    fn extract_json(&self, text: &str) -> Result<String> {
        // Try to find JSON directly (primary approach)
        if let Some(start) = text.find('{') {
            if let Some(end) = text[start..].rfind('}') {
                return Ok(text[start..start + end + 1].to_string());
            }
        }
        
        // Look for JSON between triple backticks (legacy format support)
        if let Some(start) = text.find("```json") {
            if let Some(end) = text[start..].find("```") {
                // +7 to skip ```json
                return Ok(text[start + 7..start + end].trim().to_string());
            }
        }
        
        // Try to find JSON between regular backticks (legacy format support)
        if let Some(start) = text.find('`') {
            if let Some(end) = text[start + 1..].find('`') {
                let content = text[start + 1..start + 1 + end].trim();
                if content.starts_with('{') && content.ends_with('}') {
                    return Ok(content.to_string());
                }
            }
        }
        
        // If no JSON found, return error
        Err(anyhow::anyhow!("No JSON found in LLM response"))
    }
}

/// Configuration for LLM client
#[derive(Debug, Clone)]
pub struct LlmConfig {
    /// API key for authentication
    pub api_key: String,
    
    /// Model to use
    pub model: String,
    
    /// Maximum tokens to generate
    pub max_tokens: u32,
    
    /// Temperature parameter (0.0-1.0)
    pub temperature: f32,
    
    /// Request timeout in seconds
    pub timeout_seconds: u64,
}

impl Default for LlmConfig {
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

pub mod claude;
pub mod mock;
pub mod template;
pub mod integration;

#[cfg(test)]
mod tests;

// Re-export concrete implementations
pub use claude::ClaudeClient;
pub use mock::MockLlmClient;
pub use template::TemplateManager;
pub use integration::{create_llm_client, create_integrated_llm_router, create_transitional_llm_router};