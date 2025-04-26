use std::error::Error;
use serde_json::{json, Value};
use mockito::Server;

use crate::client::A2aClient;
use crate::client::errors::ClientError;
// Remove ErrorCompatibility import
// use crate::client::error_handling::ErrorCompatibility;

// Extend A2aClient with auth-related helper methods if needed
// Note: `validate_auth_typed` and related tests removed as `auth/validate` is non-standard.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AgentCard, AgentCapabilities, AgentAuthentication}; // Import necessary types
    
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
    
    // Keep only standard A2A operations tests
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
}
