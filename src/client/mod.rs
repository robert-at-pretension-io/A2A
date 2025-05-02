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
// Removed: file_operations, data_operations, state_history, task_batch, agent_skills
mod artifacts; // Keep artifacts as it might be used by standard methods
mod auth; // Add authentication module
pub mod errors; // Add error handling module
pub mod error_handling; // Add specialized error handling module with new functions

use errors::{ClientError, A2aError};

/// A2A Client for interacting with A2A-compatible servers
#[derive(Clone, Debug)] // Add Debug derive
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
    pub async fn send_jsonrpc<T: serde::de::DeserializeOwned>(
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
            return Err(ClientError::ReqwestError { msg: format!("Request failed with status: {}", response.status()), status_code: Some(response.status().as_u16()) });
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
            return Err(ClientError::ReqwestError { msg: format!("Failed to get agent card: {}", response.status()), status_code: Some(response.status().as_u16()) });
        }
        
        match response.json().await {
            Ok(agent_card) => Ok(agent_card),
            Err(e) => Err(ClientError::JsonError(format!("Failed to parse agent card: {}", e)))
        }
    }
    
    /// Send a task to the A2A server
    pub async fn send_task(&mut self, text: &str) -> Result<Task, ClientError> {
        // Call send_task_with_metadata with no metadata
        self.send_task_with_metadata(text, None).await
    }
    
    /// Send a task to the A2A server with optional metadata
    /// 
    /// Metadata can include testing parameters like:
    /// - "_mock_delay_ms": For simulating network latency 
    /// - "_mock_duration_ms": For task lifecycle simulation (duration of processing)
    /// - "_mock_require_input": For simulating tasks that require additional input
    /// - "_mock_fail": For simulating task failures
    /// - "_mock_fail_message": For custom failure messages
    /// 
    /// # Arguments
    /// * `text` - The message text to send 
    /// * `metadata_json` - Optional JSON string containing metadata
    ///
    /// # Examples
    /// ```
    /// // Send task with a 2-second simulated delay
    /// let task = client.send_task_with_metadata(
    ///     "Hello, server!", 
    ///     Some(r#"{"_mock_delay_ms": 2000}"#)
    /// ).await?;
    /// 
    /// // Send task with state machine simulation
    /// let task = client.send_task_with_metadata(
    ///     "Task with realistic state transitions",
    ///     Some(r#"{"_mock_duration_ms": 5000, "_mock_require_input": true}"#)
    /// ).await?;
    /// ```
    pub async fn send_task_with_metadata(&mut self, text: &str, metadata_json: Option<&str>) -> Result<Task, ClientError> {
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
        
        // Parse metadata if provided
        let metadata = if let Some(meta_str) = metadata_json {
            match serde_json::from_str(meta_str) {
                Ok(parsed) => Some(parsed),
                Err(e) => return Err(ClientError::JsonError(format!("Failed to parse metadata JSON: {}", e)))
            }
        } else {
            None
        };
        
        // Create request parameters using the proper TaskSendParams type
        let params = TaskSendParams {
            id: uuid::Uuid::new_v4().to_string(),
            message: message,
            history_length: None,
            metadata: metadata,
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
    
    /// Send a task with state machine simulation
    /// 
    /// This is a convenience method for testing with simulated task lifecycles
    /// 
    /// # Arguments
    /// * `text` - The message text to send
    /// * `task_id` - Optional custom task ID to use (generated if None)
    /// * `duration_ms` - Duration in milliseconds for the task to complete
    /// * `require_input` - Whether the task requires additional input
    /// * `should_fail` - Whether the task should fail instead of completing
    /// * `fail_message` - Custom failure message (if should_fail is true)
    /// 
    /// # Examples
    /// ```
    /// // Simulated task that takes 5 seconds and requires input
    /// let task = client.simulate_task_lifecycle(
    ///     "Simulate a conversation requiring input",
    ///     None,    // Generate random ID
    ///     5000,    // 5 seconds
    ///     true,    // require input
    ///     false,   // don't fail
    ///     None     // no fail message
    /// ).await?;
    /// ```
    pub async fn simulate_task_lifecycle(
        &mut self, 
        text: &str,
        task_id: Option<&str>,
        duration_ms: u64,
        require_input: bool,
        should_fail: bool,
        fail_message: Option<&str>
    ) -> Result<Task, ClientError> {
        // Build the metadata JSON
        let mut metadata = serde_json::Map::new();
        metadata.insert("_mock_duration_ms".to_string(), serde_json::Value::Number(serde_json::Number::from(duration_ms)));
        metadata.insert("_mock_require_input".to_string(), serde_json::Value::Bool(require_input));
        metadata.insert("_mock_fail".to_string(), serde_json::Value::Bool(should_fail));
        
        if let Some(message) = fail_message {
            metadata.insert("_mock_fail_message".to_string(), serde_json::Value::String(message.to_string()));
        }
        
        let metadata_json = serde_json::Value::Object(metadata).to_string();
        
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
        
        // Parse metadata if provided
        let metadata_value = serde_json::from_str(&metadata_json)
            .map_err(|e| ClientError::JsonError(format!("Failed to parse metadata JSON: {}", e)))?;
        
        // Create request parameters with explicit ID if provided
        let params = TaskSendParams {
            id: task_id.map(|id| id.to_string()).unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            message: message,
            history_length: None,
            metadata: Some(metadata_value),
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

    /// Send a raw request and handle response for testing
    async fn send_request(&mut self, request: Value) -> Result<Value, ClientError> {
        let mut http_request = self.http_client.post(&self.base_url)
            .json(&request);
            
        if let (Some(header), Some(value)) = (&self.auth_header, &self.auth_value) {
            http_request = http_request.header(header, value);
        }
        
        let response = http_request.send().await?;
        
        if !response.status().is_success() {
            return Err(ClientError::ReqwestError { msg: format!("Request failed with status: {}", response.status()), status_code: Some(response.status().as_u16()) });
        }
        
        // Parse the response as a generic JSON-RPC response
        let json_response: Value = response.json().await?;
        Ok(json_response)
    }
    
    /// Helper for parsing error responses
    fn handle_error_response(&self, response: &Value) -> ClientError {
        if let Some(error) = response.get("error") {
            let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
            let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
            let data = error.get("data").cloned();
            
            ClientError::A2aError(A2aError::new(code, message, data))
        } else {
            ClientError::Other("Expected error response but none was found".to_string())
        }
    }
    
    /// Test method for invalid parameters error
    pub async fn test_invalid_parameters_error(&mut self) -> Result<String, ClientError> {
        // Create a request with missing required parameters
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": "test-invalid-params",
            "method": "tasks/get",
            "params": {}  // Missing required 'id' parameter
        });
        
        let response = self.send_request(request_body).await?;
        
        // This should fail with an invalid params error
        // but we'll handle the response normally and return the parsed error
        Err(self.handle_error_response(&response))
    }
    
    /// Test method for method not found error
    pub async fn test_method_not_found_error(&mut self) -> Result<String, ClientError> {
        // Create a request with a non-existent method
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": "test-method-not-found",
            "method": "non_existent_method",
            "params": {}
        });
        
        let response = self.send_request(request_body).await?;
        
        // This should fail with a method not found error
        // but we'll handle the response normally and return the parsed error
        Err(self.handle_error_response(&response))
    }
    
    // Added stubs for the other error handling test methods
    pub async fn get_skill_details_with_error_handling(&mut self, skill_id: &str) -> Result<String, ClientError> {
        // For now, just use the tasks/get endpoint with a non-existent method
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": "get-skill-details",
            "method": "skills/get",
            "params": {
                "id": skill_id
            }
        });
        
        let response = self.send_request(request_body).await?;
        
        // This should fail with a method not found error
        // but we'll handle the response normally and return the parsed error
        Err(self.handle_error_response(&response))
    }
    
    pub async fn get_batch_with_error_handling(&mut self, batch_id: &str) -> Result<String, ClientError> {
        // For now, just use the tasks/get endpoint with a non-existent method
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": "get-batch",
            "method": "batches/get",
            "params": {
                "id": batch_id
            }
        });
        
        let response = self.send_request(request_body).await?;
        
        // This should fail with a method not found error
        // but we'll handle the response normally and return the parsed error
        Err(self.handle_error_response(&response))
    }
    
    pub async fn download_file_with_error_handling(&mut self, file_id: &str) -> Result<String, ClientError> {
        // For now, just use the tasks/get endpoint with a non-existent method
        let request_body = json!({
            "jsonrpc": "2.0",
            "id": "download-file",
            "method": "files/download",
            "params": {
                "id": file_id
            }
        });
        
        let response = self.send_request(request_body).await?;
        
        // This should fail with a method not found error
        // but we'll handle the response normally and return the parsed error
        Err(self.handle_error_response(&response))
    }
}
