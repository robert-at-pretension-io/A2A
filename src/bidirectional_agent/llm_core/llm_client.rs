/// LLM client interface for bidirectional agent

use crate::bidirectional_agent::error::AgentError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// LLM client configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmClientConfig {
    /// API key for the LLM service
    pub api_key: String,
    
    /// Model to use
    pub model: String,
    
    /// Maximum tokens to generate
    pub max_tokens: u32,
    
    /// Temperature for generation
    pub temperature: f32,
    
    /// Timeout in seconds
    pub timeout_seconds: u64,
}

impl Default for LlmClientConfig {
    fn default() -> Self {
        Self {
            api_key: "".to_string(),
            model: "claude-3-haiku-20240307".to_string(),
            max_tokens: 2048,
            temperature: 0.1,
            timeout_seconds: 30,
        }
    }
}

/// Trait for LLM clients
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Send a prompt to the LLM and get a completion
    async fn complete(&self, prompt: &str) -> Result<String, AgentError>;
    
    /// Send a multi-turn conversation to the LLM and get a completion
    async fn complete_conversation(&self, messages: &[LlmMessage]) -> Result<String, AgentError>;
}

/// Message for LLM conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    /// Role of the message sender
    pub role: String,
    
    /// Content of the message
    pub content: String,
}

/// Mock LLM client for testing
pub struct MockLlmClient;

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn complete(&self, _prompt: &str) -> Result<String, AgentError> {
        // Return a JSON structure as a basic mock response
        Ok(r#"{"decision_type": "LOCAL", "reason": "Mock response", "tool_names": ["echo"]}"#.to_string())
    }
    
    async fn complete_conversation(&self, _messages: &[LlmMessage]) -> Result<String, AgentError> {
        // Return a JSON structure as a basic mock response
        Ok(r#"{"decision_type": "LOCAL", "reason": "Mock response", "tool_names": ["echo"]}"#.to_string())
    }
}

/// Factory function to create a mock LLM client
pub fn create_mock_llm_client() -> impl LlmClient {
    MockLlmClient
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mock_llm_client() {
        let client = MockLlmClient;
        
        let result = client.complete("test prompt").await.unwrap();
        assert!(result.contains("decision_type"));
        assert!(result.contains("LOCAL"));
        
        let messages = vec![
            LlmMessage {
                role: "user".to_string(),
                content: "test message".to_string(),
            },
        ];
        
        let result = client.complete_conversation(&messages).await.unwrap();
        assert!(result.contains("decision_type"));
        assert!(result.contains("LOCAL"));
    }
}