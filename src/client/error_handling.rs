use crate::client::A2aClient;
use crate::client::errors::{ClientError, A2aError, error_codes};
use std::error::Error;
use serde_json::json;

pub trait ErrorCompatibility<T> {
    fn into_box_error(self) -> Result<T, Box<dyn Error>>;
}

impl<T> ErrorCompatibility<T> for Result<T, ClientError> {
    fn into_box_error(self) -> Result<T, Box<dyn Error>> {
        match self {
            Ok(value) => Ok(value),
            Err(err) => Err(Box::new(err) as Box<dyn Error>),
        }
    }
}

impl A2aClient {
    /// Resubscribe to an existing task's streaming updates with error handling
    pub async fn resubscribe_task_with_error_handling(&mut self, task_id: &str) -> Result<crate::client::streaming::StreamingResponseStream, ClientError> {
        // Delegate to the implementation in streaming.rs
        self.resubscribe_task_typed(task_id).await
    }
    /// Send a task with improved error handling
    pub async fn send_task_with_error_handling(&mut self, task_id: &str, text: &str) -> Result<crate::types::Task, ClientError> {
        // Create a message with the text content
        let message = self.create_text_message(text);
        
        // Create request parameters
        let params = crate::types::TaskSendParams {
            id: task_id.to_string(),
            message: message,
            history_length: None,
            metadata: None,
            push_notification: None,
            session_id: None,
        };
        
        let params_value = serde_json::to_value(params)
            .map_err(|e| ClientError::JsonError(format!("Failed to serialize params: {}", e)))?;
        
        self.send_jsonrpc::<crate::types::Task>("tasks/send", params_value).await
    }

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

    // Removed test_invalid_parameters_error and test_method_not_found_error as they are less relevant now.

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

    // Removed non-standard methods: get_skill_details_with_error_handling, get_batch_with_error_handling, download_file_with_error_handling

    // Compatibility versions of core methods that retain Box<dyn Error> return type
    // These methods serve as adapters for the original API during the transition
    
    /// Compatibility version of send_task that returns Box<dyn Error>
    pub async fn send_task_compat(&mut self, text: &str) -> Result<crate::types::Task, Box<dyn Error>> {
        match self.send_task(text).await {
            Ok(val) => Ok(val),
            Err(err) => Err(Box::new(err))
        }
    }
    
    /// Compatibility version of get_task that returns Box<dyn Error>
    pub async fn get_task_compat(&mut self, task_id: &str) -> Result<crate::types::Task, Box<dyn Error>> {
        match self.get_task(task_id).await {
            Ok(val) => Ok(val),
            Err(err) => Err(Box::new(err))
        }
    }
    
    /// Compatibility version of get_agent_card that returns Box<dyn Error>
    pub async fn get_agent_card_compat(&self) -> Result<crate::types::AgentCard, Box<dyn Error>> {
        match self.get_agent_card().await {
            Ok(val) => Ok(val),
            Err(err) => Err(Box::new(err))
        }
    }
}

pub fn convert_client_error(err: ClientError) -> Box<dyn Error> {
    Box::new(err)
}
