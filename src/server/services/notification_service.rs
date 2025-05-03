use crate::types::{PushNotificationConfig, TaskPushNotificationConfig, TaskIdParams};
use crate::server::repositories::task_repository::{InMemoryTaskRepository, TaskRepository};
use crate::server::ServerError;
use std::sync::Arc;
use tracing::{debug, error, info, trace, warn, instrument}; // Import tracing macros

/// Service for handling push notification configuration
pub struct NotificationService {
    task_repository: Arc<dyn TaskRepository>,
}

impl NotificationService {
    #[instrument(skip(task_repository))]
    pub fn new(task_repository: Arc<dyn TaskRepository>) -> Self {
        info!("Creating new NotificationService.");
        Self { task_repository }
    }

    /// Set push notification configuration for a task
    #[instrument(skip(self, params), fields(task_id = %params.id, url = %params.push_notification_config.url))]
    pub async fn set_push_notification(&self, params: TaskPushNotificationConfig) -> Result<(), ServerError> {
        info!("Setting push notification configuration for task.");
        trace!(?params, "Full push notification parameters.");

        // Check if task exists
        debug!("Checking if task exists in repository.");
        let _ = self.task_repository.get_task(&params.id).await?
            .ok_or_else(|| {
                warn!("Task not found when trying to set push notification config.");
                ServerError::TaskNotFound(params.id.clone())
            })?;
        debug!("Task found.");

        // Ensure the configuration is valid
        let mut config = params.push_notification_config.clone();
        debug!("Validating push notification configuration.");
        trace!(?config, "Configuration being validated.");

        // Validate URL
        if config.url.is_empty() || (!config.url.starts_with("http://") && !config.url.starts_with("https://")) {
            error!(url = %config.url, "Invalid push notification URL provided.");
            return Err(ServerError::InvalidParameters(format!(
                "Invalid push notification URL: {}", config.url
            )));
        }
        trace!("URL validation passed.");

        // Ensure AuthenticationInfo is properly formed when token is provided
        if let Some(token) = &config.token {
            debug!("Token provided in config. Ensuring AuthenticationInfo is consistent.");
            trace!(token_len = token.len(), "Token details (length only).");
            if config.authentication.is_none() {
                debug!("Authentication field is None. Creating default Bearer auth info.");
                // If token is provided but authentication is not, create a proper authentication object
                config.authentication = Some(crate::types::AuthenticationInfo {
                    schemes: vec!["Bearer".to_string()], // Default to Bearer scheme
                    credentials: Some(token.clone()), // Use the provided token
                    extra: serde_json::Map::new(), // No extra fields by default
                });
                trace!(?config.authentication, "Created default AuthenticationInfo.");
            } else if let Some(ref mut auth) = config.authentication {
                debug!("Authentication field exists. Ensuring credentials match token and schemes are present.");
                // Ensure credentials field matches token
                auth.credentials = Some(token.clone());
                trace!("Set credentials in AuthenticationInfo to match token.");

                // Ensure schemes contains at least one scheme
                if auth.schemes.is_empty() {
                    debug!("Authentication schemes list is empty. Adding default 'Bearer' scheme.");
                    auth.schemes = vec!["Bearer".to_string()]; // Default to Bearer scheme
                }
                trace!(?auth.schemes, "Final authentication schemes.");
            }
        } else {
            trace!("No token provided in config. Skipping AuthenticationInfo consistency check.");
        }
        trace!(?config, "Final push notification config after validation/adjustment.");

        // Save the push notification configuration
        debug!("Saving push notification configuration to repository.");
        self.task_repository.save_push_notification_config(
            &params.id,
            &config
        ).await?;
        info!("Push notification configuration saved successfully.");

        Ok(())
    }

    /// Get push notification configuration for a task
    #[instrument(skip(self, params), fields(task_id = %params.id))]
    pub async fn get_push_notification(&self, params: TaskIdParams) -> Result<PushNotificationConfig, ServerError> {
        info!("Getting push notification configuration for task.");
        trace!(?params, "Get push notification parameters.");

        // Check if task exists
        debug!("Checking if task exists in repository.");
        let _ = self.task_repository.get_task(&params.id).await?
            .ok_or_else(|| {
                warn!("Task not found when trying to get push notification config.");
                ServerError::TaskNotFound(params.id.clone())
            })?;
        debug!("Task found.");

        // Get the push notification configuration
        debug!("Fetching push notification configuration from repository.");
        let config = self.task_repository.get_push_notification_config(&params.id).await?
            .ok_or_else(|| {
                warn!("No push notification configuration found for task.");
                ServerError::InvalidParameters( // Maybe should be a different error? Or just return None/empty? Check spec.
                    format!("No push notification configuration found for task {}", params.id)
                )
            })?;
        info!("Push notification configuration retrieved successfully.");
        trace!(?config, "Retrieved push notification configuration.");

        Ok(config)
    }
}
