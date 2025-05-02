//! Implementation of A2A-compliant push notification system
//! for the bidirectional agent.

use crate::bidirectional_agent::error::{AgentError, map_error};
use crate::types::{
    PushNotificationConfig, AuthenticationInfo, TaskIdParams, 
    TaskPushNotificationConfig, TaskStatusUpdateEvent, TaskArtifactUpdateEvent
};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio::time::sleep;

/// Parameters for notification service configuration
#[derive(Debug, Clone)]
pub struct NotificationServiceConfig {
    /// Maximum number of retries for failed notifications
    pub max_retries: usize,
    /// Base delay between retries in milliseconds
    pub retry_base_delay_ms: u64,
    /// JWT signing key for secure notifications (optional)
    pub jwt_signing_key: Option<String>,
    /// Retry strategy to use (exponential or fixed)
    pub retry_strategy: RetryStrategy,
}

impl Default for NotificationServiceConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_base_delay_ms: 500,
            jwt_signing_key: None,
            retry_strategy: RetryStrategy::Exponential,
        }
    }
}

/// Strategy for retry backoff
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetryStrategy {
    /// Fixed delay between retries
    Fixed,
    /// Exponential backoff (retry_delay * 2^retry_attempt)
    Exponential,
}

/// Types of events that can trigger a notification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NotificationEvent {
    /// Task status update event
    #[serde(rename = "taskStatusUpdate")]
    TaskStatusUpdate {
        status_update: TaskStatusUpdateEvent,
    },
    /// Artifact update event
    #[serde(rename = "taskArtifactUpdate")]
    TaskArtifactUpdate {
        artifact_update: TaskArtifactUpdateEvent,
    },
}

/// JWT claims for push notification authentication
#[derive(Debug, Serialize, Deserialize)]
struct NotificationClaims {
    /// Subject (task ID)
    sub: String,
    /// Issuer
    iss: String,
    /// Expiration time
    exp: u64,
    /// Issued at
    iat: u64,
    /// Type of notification
    #[serde(rename = "type")]
    notification_type: String,
}

/// A service for sending push notifications for task events
pub struct NotificationService {
    /// HTTP client for sending notifications
    client: Client,
    /// Configuration for registered notification endpoints
    configurations: Arc<Mutex<HashMap<String, PushNotificationConfig>>>,
    /// Service configuration
    config: NotificationServiceConfig,
}

impl NotificationService {
    /// Create a new notification service
    pub fn new(config: NotificationServiceConfig) -> Result<Self, AgentError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| AgentError::Internal(format!("Failed to create HTTP client: {}", e)))?;
            
        Ok(Self {
            client,
            configurations: Arc::new(Mutex::new(HashMap::new())),
            config,
        })
    }
    
    /// Set push notification configuration for a task
    pub async fn set_push_notification(
        &self,
        params: TaskPushNotificationConfig,
    ) -> Result<(), AgentError> {
        // Validate the configuration
        let mut config = params.push_notification_config.clone();
        
        // Validate URL
        if config.url.is_empty() || (!config.url.starts_with("http://") && !config.url.starts_with("https://")) {
            return Err(AgentError::InvalidParameters(format!(
                "Invalid push notification URL: {}", config.url
            )));
        }
        
        // Ensure AuthenticationInfo is properly formed when token is provided
        if let Some(token) = &config.token {
            if config.authentication.is_none() {
                // If token is provided but authentication is not, create a proper authentication object
                config.authentication = Some(AuthenticationInfo {
                    schemes: vec!["Bearer".to_string()], // Default to Bearer scheme
                    credentials: Some(token.clone()),
                    extra: serde_json::Map::new(),
                });
            } else if let Some(ref mut auth) = &mut config.authentication {
                // Ensure credentials field matches token
                auth.credentials = Some(token.clone());
                
                // Ensure schemes contains at least one scheme
                if auth.schemes.is_empty() {
                    auth.schemes = vec!["Bearer".to_string()]; // Default to Bearer scheme
                }
            }
        }
        
        // Store the configuration
        let mut configs = self.configurations.lock().unwrap();
        configs.insert(params.id.clone(), config);
        
        Ok(())
    }
    
    /// Get push notification configuration for a task
    pub async fn get_push_notification(
        &self,
        params: TaskIdParams,
    ) -> Result<PushNotificationConfig, AgentError> {
        let configs = self.configurations.lock().unwrap();
        
        configs.get(&params.id)
            .cloned()
            .ok_or_else(|| AgentError::InvalidParameters(
                format!("No push notification configuration found for task {}", params.id)
            ))
    }
    
    /// Send a notification for a task event
    pub async fn send_notification(
        &self,
        task_id: &str,
        event: NotificationEvent,
    ) -> Result<(), AgentError> {
        let config = {
            let configs = self.configurations.lock().unwrap();
            match configs.get(task_id) {
                Some(config) => config.clone(),
                None => return Ok(()), // No configuration, silently succeed
            }
        };
        
        // Prepare notification payload
        let payload = self.create_notification_payload(task_id, &event)?;
        
        // Add authentication if required
        let mut request = self.client.post(&config.url).json(&payload);
        
        if let Some(auth) = &config.authentication {
            request = self.add_authentication(request, auth, task_id)?;
        }
        
        // Send with retries
        self.send_with_retries(request).await
    }
    
    /// Create the notification payload for an event
    fn create_notification_payload(
        &self,
        task_id: &str,
        event: &NotificationEvent,
    ) -> Result<serde_json::Value, AgentError> {
        // Base payload structure
        let mut payload = serde_json::Map::new();
        
        // Add the event type
        match event {
            NotificationEvent::TaskStatusUpdate { status_update } => {
                payload.insert("type".to_string(), serde_json::Value::String("taskStatusUpdate".to_string()));
                payload.insert("task".to_string(), serde_json::to_value(status_update)?);
            },
            NotificationEvent::TaskArtifactUpdate { artifact_update } => {
                payload.insert("type".to_string(), serde_json::Value::String("taskArtifactUpdate".to_string()));
                payload.insert("task".to_string(), serde_json::to_value(artifact_update)?);
            }
        }
        
        Ok(serde_json::Value::Object(payload))
    }
    
    /// Add authentication to the request based on the configuration
    fn add_authentication(
        &self,
        request: reqwest::RequestBuilder,
        auth: &AuthenticationInfo,
        task_id: &str,
    ) -> Result<reqwest::RequestBuilder, AgentError> {
        // Get the credentials if available
        let credentials = match &auth.credentials {
            Some(creds) => creds,
            None => return Ok(request), // No credentials, return unchanged
        };
        
        // Use the first auth scheme (default to Bearer if empty)
        let scheme = auth.schemes.first().map(|s| s.as_str()).unwrap_or("Bearer");
        
        // If JWT signing is enabled, create a JWT token instead of using raw credentials
        let auth_header = if let Some(signing_key) = &self.config.jwt_signing_key {
            match scheme.to_lowercase().as_str() {
                "bearer" => {
                    // For Bearer scheme, create a JWT token
                    let now = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                        
                    let claims = NotificationClaims {
                        sub: task_id.to_string(),
                        iss: "a2a_test_suite".to_string(),
                        exp: now + 3600, // Valid for 1 hour
                        iat: now,
                        notification_type: "task_update".to_string(),
                    };
                    
                    let token = encode(
                        &Header::new(Algorithm::HS256),
                        &claims,
                        &EncodingKey::from_secret(signing_key.as_bytes()),
                    ).map_err(|e| AgentError::Internal(format!("Failed to create JWT token: {}", e)))?;
                    
                    format!("Bearer {}", token)
                },
                _ => format!("{} {}", scheme, credentials),
            }
        } else {
            // Use raw credentials
            format!("{} {}", scheme, credentials)
        };
        
        // Add the auth header
        Ok(request.header("Authorization", auth_header))
    }
    
    /// Send a request with retries based on the configured retry strategy
    async fn send_with_retries(&self, request: reqwest::RequestBuilder) -> Result<(), AgentError> {
        let mut retries = 0;
        
        loop {
            // Clone the request for the current attempt
            let cloned_request = request.try_clone()
                .ok_or_else(|| AgentError::Internal("Failed to clone request for retry".to_string()))?;
                
            // Send the request
            match cloned_request.send().await {
                Ok(response) if response.status().is_success() => {
                    return Ok(());
                },
                Ok(response) => {
                    let status = response.status();
                    let body = response.text().await.unwrap_or_default();
                    
                    if retries >= self.config.max_retries {
                        return Err(AgentError::Internal(format!(
                            "Failed to send notification after {} retries. Status: {}, Body: {}",
                            retries, status, body
                        )));
                    }
                    
                    // Calculate delay based on retry strategy
                    let delay = match self.config.retry_strategy {
                        RetryStrategy::Fixed => self.config.retry_base_delay_ms,
                        RetryStrategy::Exponential => self.config.retry_base_delay_ms * (2u64.pow(retries as u32)),
                    };
                    
                    sleep(Duration::from_millis(delay)).await;
                    retries += 1;
                },
                Err(e) => {
                    if retries >= self.config.max_retries {
                        return Err(AgentError::Internal(format!(
                            "Failed to send notification after {} retries: {}",
                            retries, e
                        )));
                    }
                    
                    // Calculate delay based on retry strategy
                    let delay = match self.config.retry_strategy {
                        RetryStrategy::Fixed => self.config.retry_base_delay_ms,
                        RetryStrategy::Exponential => self.config.retry_base_delay_ms * (2u64.pow(retries as u32)),
                    };
                    
                    sleep(Duration::from_millis(delay)).await;
                    retries += 1;
                }
            }
        }
    }
    
    /// Send a notification for a task status update
    pub async fn notify_task_status(
        &self,
        task_id: &str,
        status_update: TaskStatusUpdateEvent,
    ) -> Result<(), AgentError> {
        self.send_notification(
            task_id, 
            NotificationEvent::TaskStatusUpdate { status_update }
        ).await
    }
    
    /// Send a notification for a task artifact update
    pub async fn notify_task_artifact(
        &self,
        task_id: &str,
        artifact_update: TaskArtifactUpdateEvent,
    ) -> Result<(), AgentError> {
        self.send_notification(
            task_id, 
            NotificationEvent::TaskArtifactUpdate { artifact_update }
        ).await
    }
    
    /// Verify a notification challenge
    pub async fn verify_challenge(
        &self,
        task_id: &str,
        challenge: &str,
    ) -> Result<String, AgentError> {
        // In a real implementation, this would verify the challenge based on
        // a task-specific secret or other mechanism. For now, we just echo
        // the challenge back as the response.
        Ok(challenge.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TaskStatus, TaskState, Task, Artifact, Part, TextPart};
    use chrono::Utc;
    use mockito::{Server, Mock};
    use tokio::test;

    fn create_test_config() -> NotificationServiceConfig {
        NotificationServiceConfig {
            max_retries: 1,
            retry_base_delay_ms: 10,
            jwt_signing_key: Some("test_signing_key".to_string()),
            retry_strategy: RetryStrategy::Fixed,
        }
    }
    
    fn create_test_status_event(task_id: &str) -> TaskStatusUpdateEvent {
        TaskStatusUpdateEvent {
            id: task_id.to_string(),
            status: TaskStatus {
                state: TaskState::Completed,
                timestamp: Some(Utc::now()),
                message: None,
            },
            final_: true,
            metadata: None,
        }
    }
    
    fn create_test_artifact_event(task_id: &str) -> TaskArtifactUpdateEvent {
        TaskArtifactUpdateEvent {
            id: task_id.to_string(),
            artifact: Artifact {
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "Test artifact".to_string(),
                    metadata: None,
                })],
                index: 0,
                name: Some("test".to_string()),
                description: None,
                append: None,
                last_chunk: None,
                metadata: None,
            }
        }
    }
    
    #[test]
    async fn test_notification_service_set_get_config() {
        // Create service
        let service = NotificationService::new(create_test_config()).unwrap();
        
        // Create test params
        let task_id = "test-notification-task";
        let params = TaskPushNotificationConfig {
            id: task_id.to_string(),
            push_notification_config: PushNotificationConfig {
                url: "https://example.com/webhook".to_string(),
                token: Some("test_token".to_string()),
                authentication: None,
                challenge: None,
            }
        };
        
        // Set config
        service.set_push_notification(params).await.unwrap();
        
        // Get config
        let config = service.get_push_notification(TaskIdParams {
            id: task_id.to_string(),
            metadata: None,
        }).await.unwrap();
        
        // Verify config
        assert_eq!(config.url, "https://example.com/webhook");
        assert_eq!(config.token, Some("test_token".to_string()));
        assert!(config.authentication.is_some());
        
        // Verify authentication details
        let auth = config.authentication.unwrap();
        assert_eq!(auth.credentials, Some("test_token".to_string()));
        assert!(!auth.schemes.is_empty());
        assert_eq!(auth.schemes[0], "Bearer");
    }
    
    #[test]
    async fn test_notification_service_send_status() {
        // Create mock server
        let mut server = Server::new_async().await;
        
        // Create a notification success mock
        let mock = server.mock("POST", "/webhook")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"ok"}"#)
            .create_async()
            .await;
            
        // Create service
        let service = NotificationService::new(create_test_config()).unwrap();
        
        // Set config with mock server URL
        let task_id = "test-status-notification";
        let params = TaskPushNotificationConfig {
            id: task_id.to_string(),
            push_notification_config: PushNotificationConfig {
                url: format!("{}/webhook", server.url()),
                token: Some("test_token".to_string()),
                authentication: None,
                challenge: None,
            }
        };
        
        service.set_push_notification(params).await.unwrap();
        
        // Send status notification
        let status_event = create_test_status_event(task_id);
        service.notify_task_status(task_id, status_event).await.unwrap();
        
        // Verify mock was called
        mock.assert_async().await;
    }
    
    #[test]
    async fn test_notification_service_send_artifact() {
        // Create mock server
        let mut server = Server::new_async().await;
        
        // Create a notification success mock
        let mock = server.mock("POST", "/webhook")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"status":"ok"}"#)
            .create_async()
            .await;
            
        // Create service
        let service = NotificationService::new(create_test_config()).unwrap();
        
        // Set config with mock server URL
        let task_id = "test-artifact-notification";
        let params = TaskPushNotificationConfig {
            id: task_id.to_string(),
            push_notification_config: PushNotificationConfig {
                url: format!("{}/webhook", server.url()),
                token: None,
                authentication: None,
                challenge: None,
            }
        };
        
        service.set_push_notification(params).await.unwrap();
        
        // Send artifact notification
        let artifact_event = create_test_artifact_event(task_id);
        service.notify_task_artifact(task_id, artifact_event).await.unwrap();
        
        // Verify mock was called
        mock.assert_async().await;
    }
    
    #[test]
    async fn test_notification_service_retry() {
        // Create mock server
        let mut server = Server::new_async().await;
        
        // Create a series of mocks to simulate failures and then success
        let fail_mock = server.mock("POST", "/webhook")
            .with_status(500)
            .with_body(r#"{"error":"Internal server error"}"#)
            .create_async()
            .await;
            
        let success_mock = server.mock("POST", "/webhook")
            .with_status(200)
            .with_body(r#"{"status":"ok"}"#)
            .create_async()
            .await;
            
        // Create service with 1 retry
        let mut config = create_test_config();
        config.max_retries = 1;
        let service = NotificationService::new(config).unwrap();
        
        // Set config with mock server URL
        let task_id = "test-retry-notification";
        let params = TaskPushNotificationConfig {
            id: task_id.to_string(),
            push_notification_config: PushNotificationConfig {
                url: format!("{}/webhook", server.url()),
                token: None,
                authentication: None,
                challenge: None,
            }
        };
        
        service.set_push_notification(params).await.unwrap();
        
        // Send notification
        let status_event = create_test_status_event(task_id);
        service.notify_task_status(task_id, status_event).await.unwrap();
        
        // Verify both mocks were called
        fail_mock.assert_async().await;
        success_mock.assert_async().await;
    }
}