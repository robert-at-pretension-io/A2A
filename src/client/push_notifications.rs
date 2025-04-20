use std::error::Error;
use serde_json::{json, Value};

use crate::client::A2aClient;
use crate::client::errors::ClientError; // Add ClientError import
// Remove ErrorCompatibility import
// use crate::client::error_handling::ErrorCompatibility;
use crate::types::{PushNotificationConfig, AuthenticationInfo, TaskPushNotificationConfig, TaskIdParams};

impl A2aClient {
    /// Set a push notification webhook for a task (typed error version)
    pub async fn set_task_push_notification_typed(
        &mut self,
        task_id: &str,
        webhook_url: &str,
        auth_scheme: Option<&str>,
        token: Option<&str>
    ) -> Result<String, ClientError> { // Changed return type
        // Create the push notification config using the proper types
        let mut auth_info: Option<AuthenticationInfo> = None;
        if let Some(scheme) = auth_scheme {
            auth_info = Some(AuthenticationInfo {
                schemes: vec![scheme.to_string()],
                credentials: None,
                extra: serde_json::Map::new(),
            });
        }
        
        let config = PushNotificationConfig {
            url: webhook_url.to_string(),
            authentication: auth_info,
            token: token.map(|t| t.to_string()),
        };
        
        // Create request parameters using the proper TaskPushNotificationConfig type
        let params = crate::types::TaskPushNotificationConfig {
            id: task_id.to_string(),
            push_notification_config: config
        };
        
        // Send request and return result
        let response: Value = self.send_jsonrpc("tasks/pushNotification/set", serde_json::to_value(params)?).await?;
        
        // Extract the task ID from the response
        match response.get("id").and_then(|id| id.as_str()) {
            Some(id) => Ok(id.to_string()),
            None => Err(ClientError::Other("Invalid response: missing task ID".to_string())),
        }
    }

    /// Set a push notification webhook for a task (backward compatible)
    pub async fn set_task_push_notification(
        &mut self,
        task_id: &str,
        webhook_url: &str,
        auth_scheme: Option<&str>,
        token: Option<&str>
    ) -> Result<String, Box<dyn Error>> {
        self.set_task_push_notification_typed(task_id, webhook_url, auth_scheme, token).await.into_box_error()
    }

    /// Get push notification configuration for a task (typed error version)
    pub async fn get_task_push_notification_typed(
        &mut self,
        task_id: &str
    ) -> Result<PushNotificationConfig, ClientError> { // Changed return type
        // Create request parameters using the proper TaskIdParams type
        let params = TaskIdParams {
            id: task_id.to_string(),
            metadata: None
        };
        
        // Send request and return result
        let response: Value = self.send_jsonrpc("tasks/pushNotification/get", serde_json::to_value(params)?).await?;
        
        // Extract the push notification config from the response
        match response.get("pushNotificationConfig") {
            Some(config) => {
                match serde_json::from_value::<PushNotificationConfig>(config.clone()) {
                    Ok(config) => Ok(config),
                    Err(e) => Err(ClientError::JsonError(format!("Failed to parse push notification config: {}", e)))
                }
            },
            None => Err(ClientError::Other("Invalid response: missing push notification config".to_string())),
        }
    }

    // Remove backward compatible version
    // pub async fn get_task_push_notification(
    //     &mut self,
    //     task_id: &str
    // ) -> Result<PushNotificationConfig, Box<dyn Error>> {
    //     self.get_task_push_notification_typed(task_id).await.into_box_error()
    // }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use tokio::test;
    
    #[test]
    async fn test_set_task_push_notification() {
        // Arrange
        let task_id = "test-task-123";
        let webhook_url = "https://example.com/webhook";
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "id": task_id
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Using PartialJson matcher for request body validation
        let mock = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "tasks/pushNotification/set",
                "params": {
                    "id": task_id,
                    "pushNotificationConfig": {
                        "url": webhook_url
                    }
                }
            })))
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url());
        // Call the _typed version
        let result = client.set_task_push_notification_typed(task_id, webhook_url, None, None).await.unwrap();

        // Assert
        assert_eq!(result, task_id);
        
        mock.assert_async().await;
    }
    
    #[test]
    async fn test_set_task_push_notification_with_auth() {
        // Arrange
        let task_id = "test-task-123";
        let webhook_url = "https://example.com/webhook";
        let auth_scheme = "Bearer";
        let token = "my-secret-token";
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "id": task_id
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Using PartialJson matcher for request body validation
        let mock = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "tasks/pushNotification/set",
                "params": {
                    "id": task_id,
                    "pushNotificationConfig": {
                        "url": webhook_url,
                        "authentication": {
                            "schemes": [auth_scheme]
                        },
                        "token": token
                    }
                }
            })))
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url());
        // Call the _typed version
        let result = client.set_task_push_notification_typed(
            task_id, webhook_url, Some(auth_scheme), Some(token)
        ).await.unwrap();

        // Assert
        assert_eq!(result, task_id);
        
        mock.assert_async().await;
    }
    
    #[test]
    async fn test_get_task_push_notification() {
        // Arrange
        let task_id = "test-task-123";
        let webhook_url = "https://example.com/webhook";
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "id": task_id,
                "pushNotificationConfig": {
                    "url": webhook_url
                }
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Using PartialJson matcher for request body validation
        let mock = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "tasks/pushNotification/get",
                "params": {
                    "id": task_id
                }
            })))
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url());
        // Call the _typed version
        let config = client.get_task_push_notification_typed(task_id).await.unwrap();

        // Assert
        assert_eq!(config.url, webhook_url);
        
        mock.assert_async().await;
    }
    
    #[test]
    async fn test_get_task_push_notification_with_auth() {
        // Arrange
        let task_id = "test-task-123";
        let webhook_url = "https://example.com/webhook";
        let auth_scheme = "Bearer";
        let token = "my-secret-token";
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "id": task_id,
                "pushNotificationConfig": {
                    "url": webhook_url,
                    "authentication": {
                        "schemes": [auth_scheme]
                    },
                    "token": token
                }
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Using PartialJson matcher for request body validation
        let mock = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "tasks/pushNotification/get",
                "params": {
                    "id": task_id
                }
            })))
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url());
        // Call the _typed version
        let config = client.get_task_push_notification_typed(task_id).await.unwrap();

        // Assert
        assert_eq!(config.url, webhook_url);
        assert_eq!(config.authentication.as_ref().unwrap().schemes[0], auth_scheme);
        assert_eq!(config.token.as_ref().unwrap(), token);
        
        mock.assert_async().await;
    }
    
    #[test]
    async fn test_push_notification_not_supported_error() {
        // Arrange
        let task_id = "test-task-123";
        let webhook_url = "https://example.com/webhook";
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32003,
                "message": "Push notifications not supported"
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Mock response for server that doesn't support push notifications
        let mock = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url());
        // Call the _typed version
        let result = client.set_task_push_notification_typed(task_id, webhook_url, None, None).await;

        // Assert
        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert!(error.contains("Push notifications not supported"));
        
        mock.assert_async().await;
    }
}
