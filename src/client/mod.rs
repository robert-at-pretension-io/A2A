use reqwest::{Client as ReqwestClient, StatusCode};
use serde_json::{Value, json};
use crate::types::{
    Task, Message, Part, TextPart, Role, AgentCard,
    TaskQueryParams, TaskSendParams
};
use std::time::Duration;
use std::error::Error;

#[cfg(test)]
mod tests;

// Feature-specific modules
mod cancel_task;
pub mod streaming;
mod push_notifications;
mod file_operations;
mod data_operations;
mod artifacts;
mod state_history;
pub mod task_batch;
pub mod agent_skills;
mod auth; // Add authentication module
pub mod errors; // Add error handling module
pub mod error_handling; // Add specialized error handling module with new functions

use errors::{ClientError, A2aError};

/// A2A Client for interacting with A2A-compatible servers
pub struct A2aClient {
    http_client: ReqwestClient,
    base_url: String,
    auth_header: Option<String>,
    auth_value: Option<String>,
    request_id: i64,
}

impl A2aClient {
    /// Create a new A2A client with the specified base URL
    pub fn new(base_url: &str) -> Self {
        let client = ReqwestClient::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
            
        Self {
            http_client: client,
            base_url: base_url.to_string(),
            auth_header: None,
            auth_value: None,
            request_id: 1,
        }
    }
    
    /// Set authentication for subsequent requests
    pub fn with_auth(mut self, auth_header: &str, auth_value: &str) -> Self {
        self.auth_header = Some(auth_header.to_string());
        self.auth_value = Some(auth_value.to_string());
        self
    }
    
    /// Get the next request ID
    fn next_request_id(&mut self) -> i64 {
        let id = self.request_id;
        self.request_id += 1;
        id
    }
    
    /// Send a JSON-RPC request and receive a response
    async fn send_jsonrpc<T: serde::de::DeserializeOwned>(
        &mut self, 
        method: &str, 
        params: Value
    ) -> Result<T, ClientError> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_request_id(),
            "method": method,
            "params": params
        });
        
        let mut http_request = self.http_client.post(&self.base_url)
            .json(&request);
            
        if let (Some(header), Some(value)) = (&self.auth_header, &self.auth_value) {
            http_request = http_request.header(header, value);
        }
        
        let response = http_request.send().await?;
        
        if !response.status().is_success() {
            return Err(ClientError::HttpError(format!("Request failed with status: {}", response.status())));
        }
        
        // Parse the response as a generic JSON-RPC response
        let json_response: Value = response.json().await?;
        
        // Check for errors
        if let Some(error) = json_response.get("error") {
            let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
            let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
            let data = error.get("data").cloned();
            
            // Create a proper A2A error
            return Err(ClientError::A2aError(A2aError::new(code, message, data)));
        }
        
        // Extract the result
        if let Some(result) = json_response.get("result") {
            // Parse the result into the expected type
            match serde_json::from_value::<T>(result.clone()) {
                Ok(typed_result) => Ok(typed_result),
                Err(e) => Err(ClientError::JsonError(format!("Failed to parse result: {}", e))),
            }
        } else {
            Err(ClientError::Other("Invalid JSON-RPC response: missing 'result' field".to_string()))
        }
    }
    
    /// Get agent card from .well-known endpoint
    pub async fn get_agent_card(&self) -> Result<AgentCard, ClientError> {
        // Use the provided URL directly if it contains agent.json
        let url = if self.base_url.contains("agent.json") {
            self.base_url.to_string()
        } else {
            format!("{}/.well-known/agent.json", self.base_url)
        };
        
        let mut request = self.http_client.get(&url);
        
        if let (Some(header), Some(value)) = (&self.auth_header, &self.auth_value) {
            request = request.header(header, value);
        }
        
        let response = request.send().await?;
        
        if response.status() != StatusCode::OK {
            return Err(ClientError::HttpError(format!("Failed to get agent card: {}", response.status())));
        }
        
        match response.json().await {
            Ok(agent_card) => Ok(agent_card),
            Err(e) => Err(ClientError::JsonError(format!("Failed to parse agent card: {}", e)))
        }
    }
    
    /// Send a task to the A2A server
    pub async fn send_task(&mut self, text: &str) -> Result<Task, ClientError> {
        // Create a simple text message
        let text_part = TextPart {
            type_: "text".to_string(),
            text: text.to_string(),
            metadata: None,
        };
        
        let message = Message {
            role: Role::User,
            parts: vec![Part::TextPart(text_part)],
            metadata: None,
        };
        
        // Create request parameters using the proper TaskSendParams type
        let params = TaskSendParams {
            id: uuid::Uuid::new_v4().to_string(),
            message: message,
            history_length: None,
            metadata: None,
            push_notification: None,
            session_id: None,
        };
        
        // Send request and return result
        let params_value = match serde_json::to_value(params) {
            Ok(v) => v,
            Err(e) => return Err(ClientError::JsonError(format!("Failed to serialize params: {}", e)))
        };
        
        self.send_jsonrpc::<Task>("tasks/send", params_value).await
    }
    
    /// Get a task by ID
    pub async fn get_task(&mut self, task_id: &str) -> Result<Task, ClientError> {
        // Create request parameters using the proper TaskQueryParams type
        let params = crate::types::TaskQueryParams {
            id: task_id.to_string(),
            history_length: None,
            metadata: None,
        };
        
        let params_value = match serde_json::to_value(params) {
            Ok(v) => v,
            Err(e) => return Err(ClientError::JsonError(format!("Failed to serialize params: {}", e)))
        };
        
        self.send_jsonrpc::<Task>("tasks/get", params_value).await
    }
}