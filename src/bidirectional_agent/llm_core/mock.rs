//! Mock LLM client for testing.
//!
//! This module provides a mock implementation of the LlmClient trait
//! for use in testing scenarios.

use std::collections::HashMap;
use std::sync::RwLock;
use anyhow::Result;
use async_trait::async_trait;
use crate::bidirectional_agent::llm_core::LlmClient;

/// Mock LLM client for testing
pub struct MockLlmClient {
    /// Map of prompts to responses for deterministic testing
    responses: RwLock<HashMap<String, String>>,
    
    /// Default response to return when no match is found
    default_response: String,
}

impl MockLlmClient {
    /// Create a new mock client with specific prompt-response pairs
    pub fn new(responses: Vec<(String, String)>) -> Self {
        let mut map = HashMap::new();
        for (prompt, response) in responses {
            map.insert(prompt, response);
        }
        
        Self {
            responses: RwLock::new(map),
            default_response: "{}".to_string(), // Default to empty JSON object
        }
    }
    
    /// Create a new mock client with a default response
    pub fn with_default_response(default_response: String) -> Self {
        Self {
            responses: RwLock::new(HashMap::new()),
            default_response,
        }
    }
    
    /// Add a prompt-response pair to the mock
    pub fn add_response(&self, prompt: String, response: String) {
        let mut responses = self.responses.write().unwrap();
        responses.insert(prompt, response);
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn complete(&self, prompt: &str) -> Result<String> {
        // Check for an exact match first
        let responses = self.responses.read().unwrap();
        if let Some(response) = responses.get(prompt) {
            return Ok(response.clone());
        }
        
        // If no exact match, look for partial matches
        for (stored_prompt, response) in responses.iter() {
            if prompt.contains(stored_prompt) {
                return Ok(response.clone());
            }
        }
        
        // Return default response if no match found
        Ok(self.default_response.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_mock_llm_client_exact_match() {
        let client = MockLlmClient::new(vec![
            ("test prompt".to_string(), "test response".to_string()),
        ]);
        
        let response = client.complete("test prompt").await.unwrap();
        assert_eq!(response, "test response");
    }
    
    #[tokio::test]
    async fn test_mock_llm_client_partial_match() {
        let client = MockLlmClient::new(vec![
            ("keyword".to_string(), "found keyword".to_string()),
        ]);
        
        let response = client.complete("this is a prompt with keyword in it").await.unwrap();
        assert_eq!(response, "found keyword");
    }
    
    #[tokio::test]
    async fn test_mock_llm_client_default_response() {
        let client = MockLlmClient::with_default_response("default response".to_string());
        
        let response = client.complete("unknown prompt").await.unwrap();
        assert_eq!(response, "default response");
    }
    
    #[tokio::test]
    async fn test_mock_llm_client_json() {
        let client = MockLlmClient::new(vec![
            ("json test".to_string(), r#"{"key": "value"}"#.to_string()),
        ]);
        
        #[derive(serde::Deserialize)]
        struct TestResponse {
            key: String,
        }
        
        let response: TestResponse = client.complete_json("json test").await.unwrap();
        assert_eq!(response.key, "value");
    }
}