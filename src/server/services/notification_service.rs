use crate::types::{PushNotificationConfig, TaskPushNotificationConfig, TaskIdParams};
use crate::server::repositories::task_repository::{InMemoryTaskRepository, TaskRepository};
use crate::server::ServerError;
use std::sync::Arc;

/// Service for handling push notification configuration
pub struct NotificationService {
    task_repository: Arc<dyn TaskRepository>,
}

impl NotificationService {
    pub fn new(task_repository: Arc<dyn TaskRepository>) -> Self {
        Self { task_repository }
    }
    
    /// Set push notification configuration for a task
    pub async fn set_push_notification(&self, params: TaskPushNotificationConfig) -> Result<(), ServerError> {
        // Check if task exists
        let _ = self.task_repository.get_task(&params.id).await?
            .ok_or_else(|| ServerError::TaskNotFound(params.id.clone()))?;
        
        // Ensure the configuration is valid
        let mut config = params.push_notification_config.clone();
        
        // Validate URL
        if config.url.is_empty() || (!config.url.starts_with("http://") && !config.url.starts_with("https://")) {
            return Err(ServerError::InvalidParameters(format!(
                "Invalid push notification URL: {}", config.url
            )));
        }
        
        // Ensure AuthenticationInfo is properly formed when token is provided
        if let Some(token) = &config.token {
            if config.authentication.is_none() {
                // If token is provided but authentication is not, create a proper authentication object
                config.authentication = Some(crate::types::AuthenticationInfo {
                    schemes: vec!["Bearer".to_string()], // Default to Bearer scheme
                    credentials: Some(token.clone()),
                    extra: serde_json::Map::new(),
                });
            } else if let Some(ref mut auth) = config.authentication {
                // Ensure credentials field matches token
                auth.credentials = Some(token.clone());
                
                // Ensure schemes contains at least one scheme
                if auth.schemes.is_empty() {
                    auth.schemes = vec!["Bearer".to_string()]; // Default to Bearer scheme
                }
            }
        }
        
        // Save the push notification configuration
        self.task_repository.save_push_notification_config(
            &params.id, 
            &config
        ).await?;
        
        Ok(())
    }
    
    /// Get push notification configuration for a task
    pub async fn get_push_notification(&self, params: TaskIdParams) -> Result<PushNotificationConfig, ServerError> {
        // Check if task exists
        let _ = self.task_repository.get_task(&params.id).await?
            .ok_or_else(|| ServerError::TaskNotFound(params.id.clone()))?;
            
        // Get the push notification configuration
        let config = self.task_repository.get_push_notification_config(&params.id).await?
            .ok_or_else(|| ServerError::InvalidParameters(
                format!("No push notification configuration found for task {}", params.id)
            ))?;
            
        Ok(config)
    }
}