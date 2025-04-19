use crate::client::A2aClient;
use std::error::Error;
use crate::types::TaskIdParams;

impl A2aClient {
    /// Cancel a task by ID
    pub async fn cancel_task(&mut self, task_id: &str) -> Result<String, Box<dyn Error>> {
        // Create request parameters using the proper TaskIdParams type
        let params = TaskIdParams {
            id: task_id.to_string(),
            metadata: None,
        };
        
        // Send request and return result
        let response: serde_json::Value = self.send_jsonrpc("tasks/cancel", serde_json::to_value(params)?).await?;
        
        // Extract the task ID from the response
        match response.get("id").and_then(|id| id.as_str()) {
            Some(id) => Ok(id.to_string()),
            None => Err("Invalid response: missing task ID".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_cancel_task() {
        let task_id = "test-task-456";
        let mock_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "id": task_id
            }
        });
        
        let mut server = mockito::Server::new_async().await;
        
        // Using PartialJson matcher for request body validation
        let mock = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "tasks/cancel",
                "params": {
                    "id": task_id
                }
            })))
            .with_body(mock_response.to_string())
            .create_async().await;
            
        let mut client = A2aClient::new(&server.url());
        let result = client.cancel_task(task_id).await.unwrap();
        
        assert_eq!(result, task_id);
        
        mock.assert_async().await;
    }
    
    #[tokio::test]
    async fn test_cancel_task_error() {
        let task_id = "non-existent-task";
        let mock_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32001,
                "message": "Task not found"
            }
        });
        
        let mut server = mockito::Server::new_async().await;
        let mock = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response.to_string())
            .create_async().await;
            
        let mut client = A2aClient::new(&server.url());
        let result = client.cancel_task(task_id).await;
        
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Task not found"));
        
        mock.assert_async().await;
    }
}