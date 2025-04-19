use tokio::test;
use mockito;
use serde_json::json;
use uuid::Uuid;

use crate::client::A2aClient;
use crate::types::{Part, TextPart, Role, TaskState};

/// Test suite for the A2A client implementation
#[test]
async fn test_agent_card_discovery() {
    // Arrange
    let agent_card = json!({
        "name": "Test Agent",
        "description": "A test agent for A2A protocol",
        "url": "https://example.com/agent",
        "version": "1.0.0",
        "capabilities": {
            "streaming": true,
            "pushNotifications": false
        },
        "authentication": {
            "schemes": ["Bearer"]
        },
        "defaultInputModes": ["text/plain"],
        "defaultOutputModes": ["text/plain"],
        "skills": []
    });

    // Create a server with the async API
    let mut server = mockito::Server::new_async().await;
    
    // Create the mock with the async API
    let mock = server.mock("GET", "/.well-known/agent.json")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(agent_card.to_string())
        .create_async().await;

    // Act
    let client = A2aClient::new(&server.url());
    let result = client.get_agent_card().await.unwrap();

    // Assert
    assert_eq!(result.name, "Test Agent");
    assert_eq!(result.version, "1.0.0");
    assert_eq!(result.capabilities.streaming, true);
    assert_eq!(result.capabilities.push_notifications, false);
    
    // Assert the mock was called with the async API
    mock.assert_async().await;
}

#[test]
async fn test_agent_card_not_found() {
    // Arrange
    let mut server = mockito::Server::new_async().await;
    let mock = server.mock("GET", "/.well-known/agent.json")
        .with_status(404)
        .create_async().await;

    // Act
    let client = A2aClient::new(&server.url());
    let result = client.get_agent_card().await;

    // Assert
    assert!(result.is_err());
    let error = result.unwrap_err().to_string();
    assert!(error.contains("Failed to get agent card: 404"));
    
    mock.assert_async().await;
}

#[test]
async fn test_send_task() {
    // Arrange
    let task_id = Uuid::new_v4().to_string();
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "id": task_id,
            "status": {
                "state": "submitted",
                "timestamp": "2025-04-19T12:00:00Z"
            }
        }
    });

    let mut server = mockito::Server::new_async().await;
    
    // Using PartialJson matcher for request body validation
    let mock = server.mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .match_body(mockito::Matcher::PartialJson(json!({
            "jsonrpc": "2.0",
            "method": "tasks/send"
        })))
        .with_body(response.to_string())
        .create_async().await;

    // Act
    let mut client = A2aClient::new(&server.url());
    let result = client.send_task("Hello").await.unwrap();

    // Assert
    assert_eq!(result.id, task_id);
    assert_eq!(result.status.state, TaskState::Submitted);
    
    mock.assert_async().await;
}

#[test]
async fn test_get_task() {
    // Arrange
    let task_id = Uuid::new_v4().to_string();
    let response = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "result": {
            "id": task_id,
            "status": {
                "state": "completed",
                "timestamp": "2025-04-19T12:05:00Z",
                "message": {
                    "role": "agent",
                    "parts": [{
                        "type": "text",
                        "text": "Task completed successfully!"
                    }]
                }
            },
            "artifacts": []
        }
    });

    let mut server = mockito::Server::new_async().await;
    
    // Using PartialJson matcher for request body validation
    let mock = server.mock("POST", "/")
        .with_status(200)
        .with_header("content-type", "application/json")
        .match_body(mockito::Matcher::PartialJson(json!({
            "jsonrpc": "2.0",
            "method": "tasks/get",
            "params": {
                "id": task_id
            }
        })))
        .with_body(response.to_string())
        .create_async().await;

    // Act
    let mut client = A2aClient::new(&server.url());
    let result = client.get_task(&task_id).await.unwrap();

    // Assert
    assert_eq!(result.id, task_id);
    assert_eq!(result.status.state, TaskState::Completed);
    
    // Check status message
    let status_message = result.status.message.unwrap();
    assert_eq!(status_message.role, Role::Agent);
    assert_eq!(status_message.parts.len(), 1);
    
    if let Part::TextPart(TextPart { text, .. }) = &status_message.parts[0] {
        assert_eq!(text, "Task completed successfully!");
    } else {
        panic!("Expected TextPart");
    }
    
    mock.assert_async().await;
}