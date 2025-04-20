use crate::client::A2aClient;
use crate::client::errors::{ClientError, A2aError, error_codes};
use std::error::Error;
use serde_json::json;

// Remove the ErrorCompatibility trait and its implementation as it's no longer needed
// pub trait ErrorCompatibility<T> {
//     fn into_box_error(self) -> Result<T, Box<dyn Error>>;
// }
//
// impl<T> ErrorCompatibility<T> for Result<T, ClientError> {
//     fn into_box_error(self) -> Result<T, Box<dyn Error>> {
//         match self {
//             Ok(value) => Ok(value),
//             Err(err) => Err(Box::new(err) as Box<dyn Error>),
//         }
//     }
// }

impl A2aClient {
    /// Get a task by ID with improved error handling
    pub async fn get_task_with_error_handling(&mut self, task_id: &str) -> Result<crate::types::Task, ClientError> {
        // Create request parameters using the proper TaskQueryParams type
        let params = crate::types::TaskQueryParams {
            id: task_id.to_string(),
            history_length: None,
            metadata: None,
        };
        
        let params_value = serde_json::to_value(params)
            .map_err(|e| ClientError::JsonError(format!("Failed to serialize params: {}", e)))?;
        
        self.send_jsonrpc::<crate::types::Task>("tasks/get", params_value).await
    }

    /// Simulates an invalid parameters error
    pub async fn test_invalid_parameters_error(&mut self) -> Result<serde_json::Value, ClientError> {
        // Create params with missing required fields
        let params = json!({
            // Missing 'id' parameter
        });
        
        self.send_jsonrpc::<serde_json::Value>("tasks/get", params).await
    }

    /// Simulates a method not found error
    pub async fn test_method_not_found_error(&mut self) -> Result<serde_json::Value, ClientError> {
        // Use a non-existent method
        let params = json!({
            "id": "test-id"
        });
        
        self.send_jsonrpc::<serde_json::Value>("non_existent_method", params).await
    }

    /// Try to cancel a task that's already completed
    pub async fn cancel_task_with_error_handling(&mut self, task_id: &str) -> Result<String, ClientError> {
        // Create request parameters with a special test_error flag
        // This is used by the mock server to specifically generate a "not cancelable" error
        let mut params = serde_json::Map::new();
        params.insert("id".to_string(), serde_json::Value::String(task_id.to_string()));
        params.insert("test_error".to_string(), serde_json::Value::String("task_not_cancelable".to_string()));
        
        let params_value = serde_json::Value::Object(params);
        
        // Send request and parse the response
        let response: serde_json::Value = self.send_jsonrpc("tasks/cancel", params_value).await?;
        
        // Extract the task ID from the response
        match response.get("id").and_then(|id| id.as_str()) {
            Some(id) => Ok(id.to_string()),
            None => Err(ClientError::Other("Invalid response: missing task ID".to_string())),
        }
    }

    /// Get an agent skill with error handling
    pub async fn get_skill_details_with_error_handling(&mut self, skill_id: &str) -> Result<serde_json::Value, ClientError> {
        let params = json!({
            "id": skill_id
        });
        
        self.send_jsonrpc::<serde_json::Value>("skills/get", params).await
    }

    /// Get a batch with error handling
    pub async fn get_batch_with_error_handling(&mut self, batch_id: &str) -> Result<serde_json::Value, ClientError> {
        let params = json!({
            "id": batch_id
        });
        
        self.send_jsonrpc::<serde_json::Value>("batches/get", params).await
    }

    /// Download a file with error handling
    pub async fn download_file_with_error_handling(&mut self, file_id: &str) -> Result<serde_json::Value, ClientError> {
        let params = json!({
            "fileId": file_id
        });
        
        self.send_jsonrpc::<serde_json::Value>("files/download", params).await
    }

    // Compatibility versions of core methods that retain Box<dyn Error> return type
    // These methods serve as adapters for the original API during the transition
    
    /// Compatibility version of send_task that returns Box<dyn Error>
    pub async fn send_task_compat(&mut self, text: &str) -> Result<crate::types::Task, Box<dyn Error>> {
        self.send_task(text).await.into_box_error()
    }
    
    /// Compatibility version of get_task that returns Box<dyn Error>
    pub async fn get_task_compat(&mut self, task_id: &str) -> Result<crate::types::Task, Box<dyn Error>> {
        self.get_task(task_id).await.into_box_error()
    }
    
    /// Compatibility version of get_agent_card that returns Box<dyn Error>
    pub async fn get_agent_card_compat(&self) -> Result<crate::types::AgentCard, Box<dyn Error>> {
        self.get_agent_card().await.into_box_error()
    }
}

// Remove utility function as it's no longer needed
// pub fn convert_client_error(err: ClientError) -> Box<dyn Error> {
//     Box::new(err)
// }
