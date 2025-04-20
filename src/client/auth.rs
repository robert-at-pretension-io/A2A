use std::error::Error;
use serde_json::{json, Value};
use mockito::Server;

use crate::client::A2aClient;

// Extend A2aClient with auth-related helper methods if needed
impl A2aClient {
    /// Validate authentication credentials with the server (typed error version)
    pub async fn validate_auth_typed(&mut self) -> Result<bool, ClientError> {
        // This method makes a request to a specific endpoint designed for auth validation
        let response: Value = self.send_jsonrpc("auth/validate", json!({})).await?;
        
        // Extract the result from the response
        match response.get("valid").and_then(|v| v.as_bool()) {
            Some(valid) => Ok(valid),
            None => Err(ClientError::Other("Invalid response: missing validation result".to_string())),
        }
    }

    /// Validate authentication credentials with the server (backward compatible)
    pub async fn validate_auth(&mut self) -> Result<bool, Box<dyn Error>> {
        self.validate_auth_typed().await.into_box_error()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_client_with_bearer_auth() {
        // Arrange
        let auth_header = "Authorization";
        let auth_value = "Bearer test-token-123";
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "valid": true
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Create a mock that expects an auth header
        let mock = server.mock("POST", "/")
            .match_header(auth_header, auth_value)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url())
            .with_auth(auth_header, auth_value);
        
        let result = client.validate_auth().await.unwrap();
        
        // Assert
        assert!(result);
        mock.assert_async().await;
    }
    
    #[tokio::test]
    async fn test_client_without_auth_rejected() {
        // Arrange
        let error_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32001,
                "message": "Unauthorized request"
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Create a mock that returns 401 for unauthorized requests
        let mock = server.mock("POST", "/")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(error_response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url());
        // Notably, we don't add auth here
        
        let result = client.validate_auth().await;
        
        // Assert
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("401"));
        
        mock.assert_async().await;
    }
    
    #[tokio::test]
    async fn test_agent_card_with_auth_requirements() {
        // Arrange
        let auth_schemes = vec!["Bearer".to_string()];
        
        // Create a mock agent card with auth requirements
        let agent_card = json!({
            "name": "Auth Test Agent",
            "description": "An agent that requires authentication",
            "url": "http://localhost:8080/",
            "version": "1.0.0",
            "capabilities": {
                "streaming": true,
                "pushNotifications": false,
                "stateTransitionHistory": true
            },
            "authentication": {
                "schemes": auth_schemes
            },
            "defaultInputModes": ["text"],
            "defaultOutputModes": ["text"],
            "skills": []
        });
        
        let mut server = Server::new_async().await;
        
        // Mock the agent card endpoint
        let mock = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(agent_card.to_string())
            .create_async().await;
        
        // Act
        let client = A2aClient::new(&server.url());
        let card = client.get_agent_card().await.unwrap();
        
        // Assert
        assert!(card.authentication.is_some());
        let auth = card.authentication.unwrap();
        assert_eq!(auth.schemes, auth_schemes);
        
        mock.assert_async().await;
    }
    
    #[tokio::test]
    async fn test_oauth2_auth_scheme() {
        // Arrange
        let auth_header = "Authorization";
        let auth_value = "Bearer oauth2-test-token";
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "valid": true
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Create a mock that expects an OAuth2 auth header
        let mock = server.mock("POST", "/")
            .match_header(auth_header, auth_value)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url())
            .with_auth(auth_header, auth_value);
        
        let result = client.validate_auth().await.unwrap();
        
        // Assert
        assert!(result);
        mock.assert_async().await;
    }
    
    #[tokio::test]
    async fn test_api_key_auth_scheme() {
        // Arrange
        let auth_header = "X-API-Key";
        let auth_value = "test-api-key-123";
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "valid": true
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Create a mock that expects an API key header
        let mock = server.mock("POST", "/")
            .match_header(auth_header, auth_value)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url())
            .with_auth(auth_header, auth_value);
        
        let result = client.validate_auth().await.unwrap();
        
        // Assert
        assert!(result);
        mock.assert_async().await;
    }
    
    #[tokio::test]
    async fn test_invalid_auth_token_rejected() {
        // Arrange
        let auth_header = "Authorization";
        let auth_value = "Bearer invalid-token";
        let error_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32001,
                "message": "Invalid authentication token"
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Mock that rejects an invalid token
        let mock = server.mock("POST", "/")
            .match_header(auth_header, auth_value)
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(error_response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url())
            .with_auth(auth_header, auth_value);
        
        let result = client.validate_auth().await;
        
        // Assert
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("401"));
        
        mock.assert_async().await;
    }
    
    #[tokio::test]
    async fn test_expired_auth_token_rejected() {
        // Arrange
        let auth_header = "Authorization";
        let auth_value = "Bearer expired-token";
        let error_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32001,
                "message": "Expired authentication token"
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Mock that rejects an expired token
        let mock = server.mock("POST", "/")
            .match_header(auth_header, auth_value)
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(error_response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url())
            .with_auth(auth_header, auth_value);
        
        let result = client.validate_auth().await;
        
        // Assert
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("401"));
        
        mock.assert_async().await;
    }
    
    // Additional tests for specific A2A operations
    #[tokio::test]
    async fn test_auth_with_tasks_get() {
        // Arrange
        let auth_header = "Authorization";
        let auth_value = "Bearer test-token-123";
        let task_id = "task-123";
        
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "id": task_id,
                "status": {
                    "state": "completed",
                    "timestamp": "2023-10-20T12:00:00Z"
                },
                "sessionId": "test-session"
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Mock that requires auth for tasks/get
        let mock = server.mock("POST", "/")
            .match_header(auth_header, auth_value)
            .match_body(mockito::Matcher::PartialJson(json!({"method": "tasks/get"})))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url())
            .with_auth(auth_header, auth_value);
        
        let result = client.get_task(task_id).await;
        
        // Assert
        assert!(result.is_ok());
        
        mock.assert_async().await;
    }
    
    #[tokio::test]
    async fn test_auth_with_tasks_cancel() {
        // Arrange
        let auth_header = "Authorization";
        let auth_value = "Bearer test-token-123";
        let task_id = "task-123";
        
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "id": task_id,
                "status": {
                    "state": "canceled",
                    "timestamp": "2023-10-20T12:00:00Z"
                }
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Mock that requires auth for tasks/cancel
        let mock = server.mock("POST", "/")
            .match_header(auth_header, auth_value)
            .match_body(mockito::Matcher::PartialJson(json!({"method": "tasks/cancel"})))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url())
            .with_auth(auth_header, auth_value);
        
        let result = client.cancel_task(task_id).await;
        
        // Assert
        assert!(result.is_ok());
        
        mock.assert_async().await;
    }
}
