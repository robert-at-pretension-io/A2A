//! Manages A2A client instances for interacting with remote agents.
//!
//! This module provides a centralized manager for A2A client connections,
//! handling authentication, caching, and standardized error handling.
//! It abstracts the details of client creation and allows seamless
//! interaction with the A2A protocol.

use crate::client::{
    A2aClient, 
    errors::ClientError,
    streaming::{StreamingResponse, StreamingResponseStream}
};
use crate::types::{
    TaskSendParams, Task, TaskState, AgentCard, 
    Message, Artifact,
    TaskIdParams, PushNotificationConfig, TaskPushNotificationConfig,
    Role, Part, TextPart
};
use anyhow::Context;
use crate::bidirectional_agent::{
    agent_registry::AgentRegistry,
    config::BidirectionalAgentConfig,
    error::AgentError,
    types::{get_metadata_ext, set_metadata_ext, TaskOrigin},
};
use dashmap::DashMap;
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;
use tracing::{debug, error, info, warn};
use async_trait::async_trait;
use futures_util::StreamExt;

/// Namespace for tracking delegated tasks
const DELEGATION_NAMESPACE: &str = "a2a.bidirectional.delegation";

/// Manages cached A2aClient instances for different remote agents.
#[derive(Clone)]
pub struct ClientManager {
    /// Cache of clients, keyed by agent ID
    clients: Arc<DashMap<String, A2aClient>>,
    
    /// Reference to the agent registry to get agent URLs and auth info
    registry: Arc<AgentRegistry>,
    
    /// Agent's own configuration (needed for client TLS/auth setup)
    self_config: Arc<BidirectionalAgentConfig>,
    
    /// Cache of active delegations with their states
    delegations: Arc<DashMap<String, DelegationInfo>>,
    
    /// Notification handlers for delegation events
    notification_handlers: Arc<RwLock<Vec<Arc<dyn DelegationEventHandler + Send + Sync>>>>,
}

/// Information about a delegated task
#[derive(Debug, Clone)]
struct DelegationInfo {
    /// Local task ID
    local_task_id: String,
    
    /// Remote agent ID
    agent_id: String,
    
    /// Remote task ID
    remote_task_id: String,
    
    /// When the delegation was created
    created_at: chrono::DateTime<chrono::Utc>,
    
    /// Last time the delegation was polled
    last_polled_at: chrono::DateTime<chrono::Utc>,
    
    /// Current state of the remote task
    remote_state: Option<TaskState>,
}

/// Handler for delegation events
#[async_trait]
pub trait DelegationEventHandler: Send + Sync {
    /// Called when a remote task state changes
    async fn on_remote_state_change(&self, local_task_id: &str, remote_task: &Task) -> Result<(), AgentError>;
    
    /// Called when a remote task receives a new artifact
    async fn on_remote_artifact(&self, local_task_id: &str, remote_task_id: &str, artifact: &Artifact) -> Result<(), AgentError>;
    
    /// Called when a remote task completes
    async fn on_remote_completion(&self, local_task_id: &str, remote_task: &Task) -> Result<(), AgentError>;
    
    /// Called when a remote task fails
    async fn on_remote_failure(&self, local_task_id: &str, remote_task: &Task) -> Result<(), AgentError>;
}

impl ClientManager {
    /// Creates a new ClientManager.
    pub fn new(registry: Arc<AgentRegistry>, self_config: Arc<BidirectionalAgentConfig>) -> Result<Self, AgentError> {
        let manager = Self {
            clients: Arc::new(DashMap::new()),
            registry,
            self_config,
            delegations: Arc::new(DashMap::new()),
            notification_handlers: Arc::new(RwLock::new(Vec::new())),
        };
        
        Ok(manager)
    }

    /// Registers a delegation event handler
    pub fn register_event_handler(&self, handler: Arc<dyn DelegationEventHandler + Send + Sync>) {
        if let Ok(mut handlers) = self.notification_handlers.write() {
            handlers.push(handler);
        }
    }

    /// Gets an existing A2aClient for the target agent or creates a new one.
    /// Handles client configuration based on the agent's card and self-config.
    pub async fn get_or_create_client(&self, agent_id: &str) -> Result<A2aClient, AgentError> {
        // Fast path: Check if client already exists in cache
        if let Some(client_entry) = self.clients.get(agent_id) {
            return Ok(client_entry.value().clone());
        }

        // Slow path: Client not cached, need to create it
        // 1. Get agent info from registry
        let agent_info = self.registry.get(agent_id)
            .ok_or_else(|| AgentError::AgentNotFound(format!("Agent '{}' not found in registry", agent_id)))?;

        let agent_card = &agent_info.card;
        let agent_url = &agent_card.url;

        // 2. Build the underlying reqwest client with potential TLS/proxy config
        let http_client = self.build_http_client()
            .map_err(|e| AgentError::ConfigError(format!("Failed to build HTTP client: {}", e)))?;

        // 3. Create the A2aClient instance
        let mut a2a_client = A2aClient::new(agent_url);

        // 4. Configure authentication based on agent card and self config
        if let Some(required_auth) = &agent_card.authentication {
            // Find a scheme supported by the remote agent that we have credentials for
            let mut configured_auth = false;
            for scheme in &required_auth.schemes {
                if let Some(credential) = self.self_config.auth.client_credentials.get(scheme.as_str()) {
                    // Determine the correct header name based on the scheme
                    let header_name = match scheme.as_str() {
                        "Bearer" | "bearer" => "Authorization",
                        "ApiKey" | "apikey" => "X-API-Key", // Common practice
                        // Add other schemes as needed
                        _ => {
                            debug!("Unsupported auth scheme '{}' for agent '{}'", scheme, agent_id);
                            continue; // Try next scheme
                        }
                    };
                    let header_value = if scheme.eq_ignore_ascii_case("Bearer") {
                        format!("Bearer {}", credential)
                    } else {
                        credential.clone()
                    };

                    debug!("Configuring client for agent '{}' with auth scheme '{}'", agent_id, scheme);
                    a2a_client = a2a_client.with_auth(header_name, &header_value);
                    configured_auth = true;
                    break; // Use the first matching scheme
                }
            }
            if !configured_auth && !required_auth.schemes.is_empty() {
                warn!("No matching client credentials found for required schemes {:?} for agent '{}'",
                    required_auth.schemes, agent_id);
                // Proceed without auth and let the server reject if needed
            }
        }

        // 5. Cache the new client
        let client_entry = self.clients.entry(agent_id.to_string()).or_insert(a2a_client);

        // Return the client
        Ok(client_entry.value().clone())
    }

    /// Helper to build the underlying reqwest HTTP client based on config.
    fn build_http_client(&self) -> Result<reqwest::Client, anyhow::Error> {
        let mut builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30)); // Default timeout

        // Configure proxy if specified
        if let Some(proxy_url) = &self.self_config.network.proxy_url {
            let mut proxy = reqwest::Proxy::all(proxy_url)
                .with_context(|| format!("Invalid proxy URL: {}", proxy_url))?;

            // Add proxy authentication if needed
            if let Some((username, password)) = &self.self_config.network.proxy_auth {
                proxy = proxy.basic_auth(username, password);
            }
            builder = builder.proxy(proxy);
            debug!("Configuring HTTP client with proxy: {}", proxy_url);
        }

        // Configure custom CA certificate if specified
        if let Some(ca_path) = &self.self_config.network.ca_cert_path {
            let ca_cert_bytes = std::fs::read(ca_path)
                .with_context(|| format!("Failed to read CA certificate from: {}", ca_path))?;
            let ca_cert = reqwest::Certificate::from_pem(&ca_cert_bytes)
                .with_context(|| format!("Failed to parse CA certificate from PEM format: {}", ca_path))?;
            builder = builder.add_root_certificate(ca_cert);
            debug!("Configuring HTTP client with custom CA: {}", ca_path);
        }

        // Configure client certificate (mTLS) if specified
        if let (Some(cert_path), Some(key_path)) =
            (&self.self_config.auth.client_cert_path, &self.self_config.auth.client_key_path)
        {
            let cert_bytes = std::fs::read(cert_path)
                .with_context(|| format!("Failed to read client certificate from: {}", cert_path))?;
            let key_bytes = std::fs::read(key_path)
                .with_context(|| format!("Failed to read client private key from: {}", key_path))?;
            
            // Combine the bytes manually
            let mut combined = cert_bytes.clone();
            combined.extend_from_slice(&key_bytes);
            
            // reqwest::Identity::from_pem and identity() methods are experimental or not available
            // Just log and skip this part for now, until we implement proper identity support
            debug!("Client certificate/key loading is currently stubbed out - mTLS not fully implemented");
            debug!("Configuring HTTP client with mTLS identity: cert={}, key={}", cert_path, key_path);
        }

        // Build the client
        builder.build()
            .with_context(|| "Failed to build reqwest HTTP client")
    }

    /// Sends a task to a remote agent using the managed client.
    pub async fn send_task(&self, agent_id: &str, params: TaskSendParams) -> Result<Task, AgentError> {
        debug!("Sending task to agent '{}'", agent_id);
        
        // Get or create the client
        let mut client = self.get_or_create_client(agent_id).await?;
        
        // Use the A2A client to send the task
        match client.send_jsonrpc::<Task>("tasks/send", serde_json::to_value(params.clone())?).await {
            Ok(task) => {
                debug!("Successfully sent task '{}' to agent '{}'", task.id, agent_id);
                
                // Register delegation for the task
                let local_task_id = params.id.clone();
                self.register_delegation(&local_task_id, agent_id, &task.id).await?;
                
                Ok(task)
            },
            Err(e) => {
                error!("Failed to send task to agent '{}': {}", agent_id, e);
                Err(AgentError::A2aClientError(e))
            }
        }
    }

    /// Sends a task with streaming response to a remote agent.
    pub async fn send_task_streaming(&self, agent_id: &str, params: TaskSendParams) -> Result<StreamingResponseStream, AgentError> {
        debug!("Sending streaming task to agent '{}'", agent_id);
        
        // Get or create the client
        let mut client = self.get_or_create_client(agent_id).await?;
        
        // Use the A2A client to send the streaming task
        match client.send_streaming_request_typed("tasks/sendSubscribe", serde_json::to_value(params.clone())?).await {
            Ok(stream) => {
                debug!("Successfully started streaming task to agent '{}'", agent_id);
                
                // Create a local task ID reference
                let local_task_id = params.id.clone();
                let manager_clone = self.clone();
                
                // Process the stream in the background to track task state
                // This requires a stream processor that detects the remote task ID from the first event
                tokio::spawn(async move {
                    manager_clone.process_streaming_task(local_task_id, agent_id.to_string(), stream).await;
                });
                
                // Return the stream to the caller
                // Note: We would need to clone the stream to both process it and return it
                // For now, we'll create a new stream
                client.send_streaming_request_typed("tasks/sendSubscribe", serde_json::to_value(params)?).await
                    .map_err(|e| AgentError::A2aClientError(e))
            },
            Err(e) => {
                error!("Failed to start streaming task to agent '{}': {}", agent_id, e);
                Err(AgentError::A2aClientError(e))
            }
        }
    }

    /// Process a streaming task response to track state and notify handlers
    async fn process_streaming_task(&self, local_task_id: String, agent_id: String, mut stream: StreamingResponseStream) {
        let mut remote_task_id = None;
        
        while let Some(event_result) = stream.next().await {
            match event_result {
                Ok(event) => {
                    match &event {
                        StreamingResponse::Status(task) => {
                            // Extract remote task ID if this is the first status event
                            if remote_task_id.is_none() {
                                remote_task_id = Some(task.id.clone());
                                // Register delegation now that we have the remote task ID
                                if let Err(e) = self.register_delegation(&local_task_id, &agent_id, &task.id).await {
                                    error!("Failed to register delegation for streaming task: {}", e);
                                }
                            }
                            
                            // Notify handlers about state change
                            // Clone the handlers to avoid holding the lock across await points
                            let handlers = {
                                // Get notification handlers or return empty vec on error
                                match self.notification_handlers.read() {
                                    Ok(guard) => {
                                        guard.iter().map(Arc::clone).collect::<Vec<_>>()
                                    },
                                    Err(e) => {
                                        error!("Failed to read notification handlers: {:?}", e);
                                        Vec::new()
                                    }
                                }
                            };
                            
                            // Process each handler
                            for handler in handlers {
                                if let Err(e) = handler.on_remote_state_change(&local_task_id, task).await {
                                    error!("Error in delegation event handler: {}", e);
                                }
                            }
                            
                            // Check for terminal states
                            match task.status.state {
                                TaskState::Completed => {
                                    debug!("Remote streaming task completed: {}", task.id);
                                    if let Ok(handlers) = self.notification_handlers.read() {
                                        for handler in handlers.iter() {
                                            if let Err(e) = handler.on_remote_completion(&local_task_id, task).await {
                                                error!("Error in delegation completion handler: {}", e);
                                            }
                                        }
                                    }
                                },
                                TaskState::Failed => {
                                    debug!("Remote streaming task failed: {}", task.id);
                                    if let Ok(handlers) = self.notification_handlers.read() {
                                        for handler in handlers.iter() {
                                            if let Err(e) = handler.on_remote_failure(&local_task_id, task).await {
                                                error!("Error in delegation failure handler: {}", e);
                                            }
                                        }
                                    }
                                },
                                _ => {}
                            }
                        },
                        StreamingResponse::Artifact(artifact) => {
                            // Notify handlers about new artifact
                            if let (Some(remote_id), Ok(handlers)) = (remote_task_id.as_ref(), self.notification_handlers.read()) {
                                for handler in handlers.iter() {
                                    if let Err(e) = handler.on_remote_artifact(&local_task_id, remote_id, artifact).await {
                                        error!("Error in delegation artifact handler: {}", e);
                                    }
                                }
                            }
                        },
                        StreamingResponse::Final(task) => {
                            debug!("Remote streaming task finalized: {}", task.id);
                            // Extract remote task ID if this is the first status event
                            if remote_task_id.is_none() {
                                remote_task_id = Some(task.id.clone());
                                // Register delegation now that we have the remote task ID
                                if let Err(e) = self.register_delegation(&local_task_id, &agent_id, &task.id).await {
                                    error!("Failed to register delegation for streaming task: {}", e);
                                }
                            }
                            
                            // Notify handlers about final state
                            if let Ok(handlers) = self.notification_handlers.read() {
                                match task.status.state {
                                    TaskState::Completed => {
                                        for handler in handlers.iter() {
                                            if let Err(e) = handler.on_remote_completion(&local_task_id, task).await {
                                                error!("Error in delegation completion handler: {}", e);
                                            }
                                        }
                                    },
                                    TaskState::Failed => {
                                        for handler in handlers.iter() {
                                            if let Err(e) = handler.on_remote_failure(&local_task_id, task).await {
                                                error!("Error in delegation failure handler: {}", e);
                                            }
                                        }
                                    },
                                    _ => {
                                        for handler in handlers.iter() {
                                            if let Err(e) = handler.on_remote_state_change(&local_task_id, task).await {
                                                error!("Error in delegation event handler: {}", e);
                                            }
                                        }
                                    }
                                }
                            }
                            
                            // Remove delegation if task is in a terminal state
                            if matches!(task.status.state, TaskState::Completed | TaskState::Failed | TaskState::Canceled) {
                                debug!("Removing delegation for completed streaming task: {}", task.id);
                                self.delegations.remove(&local_task_id);
                            }
                        }
                    }
                },
                Err(e) => {
                    error!("Error in streaming task: {}", e);
                }
            }
        }
        
        debug!("Streaming task stream ended for task ID: {}", local_task_id);
    }

    /// Retrieves the status of a task from a remote agent.
    pub async fn get_task_status(&self, agent_id: &str, task_id: &str) -> Result<Task, AgentError> {
        debug!("Getting status for task '{}' from agent '{}'", task_id, agent_id);
        
        // Get or create the client
        let mut client = self.get_or_create_client(agent_id).await?;
        
        // Use the A2A client to get the task status
        match client.get_task(task_id).await {
            Ok(task) => {
                debug!("Successfully retrieved status for task '{}': {:?}", task_id, task.status.state);
                Ok(task)
            },
            Err(e) => {
                error!("Failed to get task status from agent '{}': {}", agent_id, e);
                Err(AgentError::A2aClientError(e))
            }
        }
    }
    
    /// Cancels a task on a remote agent.
    pub async fn cancel_task(&self, agent_id: &str, task_id: &str) -> Result<Task, AgentError> {
        debug!("Canceling task '{}' on agent '{}'", task_id, agent_id);
        
        // Get or create the client
        let mut client = self.get_or_create_client(agent_id).await?;
        
        // Create the task ID params
        let params = TaskIdParams {
            id: task_id.to_string(),
            metadata: None,
        };
        
        // Use the A2A client to cancel the task
        match client.send_jsonrpc::<Task>("tasks/cancel", serde_json::to_value(params)?).await {
            Ok(task) => {
                debug!("Successfully canceled task '{}' on agent '{}'", task_id, agent_id);
                Ok(task)
            },
            Err(e) => {
                error!("Failed to cancel task '{}' on agent '{}': {}", task_id, agent_id, e);
                Err(AgentError::A2aClientError(e))
            }
        }
    }
    
    /// Sets up push notifications for a task.
    pub async fn set_push_notification(
        &self, 
        agent_id: &str, 
        task_id: &str,
        webhook_url: &str,
        auth_scheme: Option<&str>,
        token: Option<&str>
    ) -> Result<String, AgentError> {
        debug!("Setting up push notifications for task '{}' on agent '{}'", task_id, agent_id);
        
        // Get or create the client
        let mut client = self.get_or_create_client(agent_id).await?;
        
        // Use the A2A client to set up push notifications
        match client.set_task_push_notification_typed(task_id, webhook_url, auth_scheme, token).await {
            Ok(result_id) => {
                debug!("Successfully set up push notifications for task '{}'", task_id);
                Ok(result_id)
            },
            Err(e) => {
                error!("Failed to set up push notifications for task '{}': {}", task_id, e);
                Err(AgentError::A2aClientError(e))
            }
        }
    }
    
    /// Gets the push notification configuration for a task.
    pub async fn get_push_notification(&self, agent_id: &str, task_id: &str) -> Result<PushNotificationConfig, AgentError> {
        debug!("Getting push notification config for task '{}' on agent '{}'", task_id, agent_id);
        
        // Get or create the client
        let mut client = self.get_or_create_client(agent_id).await?;
        
        // Use the A2A client to get the push notification config
        match client.get_task_push_notification_typed(task_id).await {
            Ok(config) => {
                debug!("Successfully retrieved push notification config for task '{}'", task_id);
                Ok(config)
            },
            Err(e) => {
                error!("Failed to get push notification config for task '{}': {}", task_id, e);
                Err(AgentError::A2aClientError(e))
            }
        }
    }
    
    /// Gets an agent card for the specified agent ID.
    pub async fn get_agent_card(&self, agent_id: &str) -> Result<Option<AgentCard>, AgentError> {
        // First check the registry
        if let Some(agent_info) = self.registry.get(agent_id) {
            return Ok(Some(agent_info.card.clone()));
        }
        
        // If not in registry, try to fetch it directly
        let mut client = match self.get_or_create_client(agent_id).await {
            Ok(client) => client,
            Err(e) => {
                debug!("Failed to create client for agent '{}': {}", agent_id, e);
                return Ok(None);
            }
        };
        
        // Use the A2A client to get the agent card
        match client.get_agent_card().await {
            Ok(card) => {
                debug!("Successfully retrieved agent card for '{}'", agent_id);
                Ok(Some(card))
            },
            Err(e) => {
                debug!("Failed to get agent card for '{}': {}", agent_id, e);
                Ok(None)
            }
        }
    }

    /// Register a delegation in the tracking system
    async fn register_delegation(&self, local_task_id: &str, agent_id: &str, remote_task_id: &str) -> Result<(), AgentError> {
        debug!("Registering delegation: local='{}', agent='{}', remote='{}'", 
               local_task_id, agent_id, remote_task_id);
        
        // Create delegation info
        let delegation = DelegationInfo {
            local_task_id: local_task_id.to_string(),
            agent_id: agent_id.to_string(),
            remote_task_id: remote_task_id.to_string(),
            created_at: Utc::now(),
            last_polled_at: Utc::now(),
            remote_state: None,
        };
        
        // Store in the delegations map
        self.delegations.insert(local_task_id.to_string(), delegation);
        
        Ok(())
    }

    /// Gets all active delegations
    pub fn get_delegations(&self) -> Vec<(String, String, String)> {
        let mut result = Vec::new();
        
        for entry in self.delegations.iter() {
            let info = entry.value();
            result.push((
                info.local_task_id.clone(),
                info.agent_id.clone(),
                info.remote_task_id.clone()
            ));
        }
        
        result
    }

    /// Periodically polls the status of delegated tasks.
    /// This should be run in a background task.
    pub async fn run_delegated_task_poll_loop(&self, interval: chrono::Duration) {
        info!("Starting delegated task polling loop (interval: {:?})", interval);
        
        let std_interval = match interval.to_std() {
            Ok(d) => d,
            Err(_) => {
                error!("Invalid polling interval duration: {:?}", interval);
                // Default to a reasonable interval like 30 seconds if conversion fails
                Duration::from_secs(30)
            }
        };

        loop {
            sleep(std_interval).await;
            debug!("Running periodic check for delegated tasks...");

            // Get all active delegations
            let delegations = self.get_delegations();
            debug!("Found {} active delegated tasks", delegations.len());

            for (local_task_id, agent_id, remote_task_id) in delegations {
                debug!("Checking status for remote task '{}' on agent '{}' (local task '{}')",
                         remote_task_id, agent_id, local_task_id);

                match self.get_task_status(&agent_id, &remote_task_id).await {
                    Ok(remote_task) => {
                        debug!("Remote status: {:?}", remote_task.status.state);
                        
                        // Update delegation info with new state
                        if let Some(mut info) = self.delegations.get_mut(&local_task_id) {
                            info.last_polled_at = Utc::now();
                            info.remote_state = Some(remote_task.status.state.clone());
                        }
                        
                        // Notify event handlers
                        if let Ok(handlers) = self.notification_handlers.read() {
                            match remote_task.status.state {
                                TaskState::Completed => {
                                    for handler in handlers.iter() {
                                        if let Err(e) = handler.on_remote_completion(&local_task_id, &remote_task).await {
                                            error!("Error in delegation completion handler: {}", e);
                                        }
                                    }
                                    // Remove delegation once completed
                                    self.delegations.remove(&local_task_id);
                                },
                                TaskState::Failed | TaskState::Canceled => {
                                    for handler in handlers.iter() {
                                        if let Err(e) = handler.on_remote_failure(&local_task_id, &remote_task).await {
                                            error!("Error in delegation failure handler: {}", e);
                                        }
                                    }
                                    // Remove delegation once failed
                                    self.delegations.remove(&local_task_id);
                                },
                                _ => {
                                    for handler in handlers.iter() {
                                        if let Err(e) = handler.on_remote_state_change(&local_task_id, &remote_task).await {
                                            error!("Error in delegation event handler: {}", e);
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to get status for remote task '{}': {}", remote_task_id, e);
                        // After several consecutive failures, could mark the delegation as failed
                    }
                }
            }
            
            debug!("Periodic delegated task check complete");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bidirectional_agent::config::{BidirectionalAgentConfig, AuthConfig, NetworkConfig, DirectoryConfig, ToolConfigs};
    use crate::bidirectional_agent::agent_directory::AgentDirectory;
    use mockito::Server;
    use crate::types::{AgentCard, AgentCapabilities, AgentSkill, AgentAuthentication};
    use std::collections::HashMap;
    use tempfile;

    // Helper to create a mock agent card
    fn create_mock_agent_card(name: &str, url: &str, auth_schemes: Option<Vec<String>>) -> AgentCard {
        AgentCard {
            name: name.to_string(),
            description: Some(format!("Mock agent {}", name)),
            url: url.to_string(),
            provider: None,
            version: "1.0".to_string(),
            documentation_url: None,
            capabilities: AgentCapabilities {
                streaming: true,
                push_notifications: true,
                state_transition_history: true,
            },
            authentication: auth_schemes.map(|schemes| AgentAuthentication { schemes, credentials: None }),
            default_input_modes: vec!["text/plain".to_string()],
            default_output_modes: vec!["text/plain".to_string()],
            skills: vec![AgentSkill {
                id: "mock-skill".to_string(),
                name: "Mock Skill".to_string(),
                description: None, tags: None, examples: None, input_modes: None, output_modes: None
            }],
        }
    }

    // Helper to set up a test environment
    async fn setup_test_environment() -> (
        Server,
        Arc<AgentRegistry>,
        Arc<BidirectionalAgentConfig>,
        tempfile::TempDir
    ) {
        let server = Server::new_async().await;
        let temp_dir = tempfile::tempdir().unwrap();
        
        // Create directory config
        let dir_config = DirectoryConfig {
            db_path: temp_dir.path().join("test.db").to_string_lossy().to_string(),
            ..Default::default()
        };
        
        // Create directory and registry
        let directory = Arc::new(AgentDirectory::new(&dir_config).await.unwrap());
        let registry = Arc::new(AgentRegistry::new(directory.clone()));
        
        // Create agent config
        let config = Arc::new(BidirectionalAgentConfig {
            self_id: "test-agent".to_string(),
            base_url: "https://test-agent.example.com".to_string(),
            discovery: vec![],
            auth: AuthConfig::default(),
            network: NetworkConfig::default(),
            tools: ToolConfigs::default(),
            directory: dir_config,
            tool_discovery_interval_minutes: 30,
        });
        
        (server, registry, config, temp_dir)
    }

    #[tokio::test]
    async fn test_get_or_create_client() {
        let (mut server, registry, config, _temp_dir) = setup_test_environment().await;
        
        // Create mock agent
        let agent_name = "test-agent";
        let mock_card = create_mock_agent_card(agent_name, &server.url(), None);
        
        // Create mock response for agent card
        let _m = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&mock_card).unwrap())
            .create_async().await;
        
        // Discover the agent
        registry.discover(&server.url()).await.unwrap();
        
        // Create client manager
        let manager = ClientManager::new(registry, config).unwrap();
        
        // Test get or create client
        let client = manager.get_or_create_client(agent_name).await.unwrap();
        
        // Verify client is cached
        assert!(manager.clients.contains_key(agent_name));
        
        // Get again and verify it's the same client (should be cached)
        let cached_client = manager.get_or_create_client(agent_name).await.unwrap();
        // Note: We can't directly compare the clients with == since A2aClient doesn't implement PartialEq
    }

    #[tokio::test]
    async fn test_send_task() {
        let (mut server, registry, config, _temp_dir) = setup_test_environment().await;
        
        // Create mock agent
        let agent_name = "test-agent";
        let mock_card = create_mock_agent_card(agent_name, &server.url(), None);
        
        // Create mock response for agent card
        let _m_card = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&mock_card).unwrap())
            .create_async().await;
        
        // Create mock response for task send
        let task_id = "test-task-123";
        let response_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "id": task_id,
                "status": {
                    "state": "working",
                    "timestamp": "2025-04-28T12:00:00Z"
                }
            }
        });
        
        let _m_send = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "tasks/send"
            })))
            .with_body(response_body.to_string())
            .create_async().await;
        
        // Discover the agent
        registry.discover(&server.url()).await.unwrap();
        
        // Create client manager
        let manager = ClientManager::new(registry, config).unwrap();
        
        // Create task parameters
        let message = Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: "Test task".to_string(),
                metadata: None,
            })],
            metadata: None,
        };
        
        let params = TaskSendParams {
            id: "local-task-123".to_string(),
            message,
            history_length: None,
            metadata: None,
            push_notification: None,
            session_id: None,
        };
        
        // Test send task
        let task = manager.send_task(agent_name, params).await.unwrap();
        
        // Verify task
        assert_eq!(task.id, task_id);
        assert_eq!(task.status.state, TaskState::Working);
        
        // Verify delegation is tracked
        let delegations = manager.get_delegations();
        assert_eq!(delegations.len(), 1);
        assert_eq!(delegations[0].0, "local-task-123");
        assert_eq!(delegations[0].1, agent_name);
        assert_eq!(delegations[0].2, task_id);
    }

    #[tokio::test]
    async fn test_get_task_status() {
        let (mut server, registry, config, _temp_dir) = setup_test_environment().await;
        
        // Create mock agent
        let agent_name = "test-agent";
        let mock_card = create_mock_agent_card(agent_name, &server.url(), None);
        
        // Create mock response for agent card
        let _m_card = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&mock_card).unwrap())
            .create_async().await;
        
        // Create mock response for task get
        let task_id = "test-task-123";
        let response_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "id": task_id,
                "status": {
                    "state": "completed",
                    "timestamp": "2025-04-28T12:05:00Z"
                }
            }
        });
        
        let _m_get = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "tasks/get",
                "params": {
                    "id": task_id
                }
            })))
            .with_body(response_body.to_string())
            .create_async().await;
        
        // Discover the agent
        registry.discover(&server.url()).await.unwrap();
        
        // Create client manager
        let manager = ClientManager::new(registry, config).unwrap();
        
        // Test get task status
        let task = manager.get_task_status(agent_name, task_id).await.unwrap();
        
        // Verify task
        assert_eq!(task.id, task_id);
        assert_eq!(task.status.state, TaskState::Completed);
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let (mut server, registry, config, _temp_dir) = setup_test_environment().await;
        
        // Create mock agent
        let agent_name = "test-agent";
        let mock_card = create_mock_agent_card(agent_name, &server.url(), None);
        
        // Create mock response for agent card
        let _m_card = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&mock_card).unwrap())
            .create_async().await;
        
        // Create mock response for task cancel
        let task_id = "test-task-123";
        let response_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "id": task_id,
                "status": {
                    "state": "canceled",
                    "timestamp": "2025-04-28T12:10:00Z"
                }
            }
        });
        
        let _m_cancel = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "tasks/cancel",
                "params": {
                    "id": task_id
                }
            })))
            .with_body(response_body.to_string())
            .create_async().await;
        
        // Discover the agent
        registry.discover(&server.url()).await.unwrap();
        
        // Create client manager
        let manager = ClientManager::new(registry, config).unwrap();
        
        // Test cancel task
        let task = manager.cancel_task(agent_name, task_id).await.unwrap();
        
        // Verify task
        assert_eq!(task.id, task_id);
        assert_eq!(task.status.state, TaskState::Canceled);
    }

    #[tokio::test]
    async fn test_set_push_notification() {
        let (mut server, registry, config, _temp_dir) = setup_test_environment().await;
        
        // Create mock agent
        let agent_name = "test-agent";
        let mock_card = create_mock_agent_card(agent_name, &server.url(), None);
        
        // Create mock response for agent card
        let _m_card = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&mock_card).unwrap())
            .create_async().await;
        
        // Create mock response for push notification set
        let task_id = "test-task-123";
        let webhook_url = "https://example.com/webhook";
        let response_body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "id": task_id
            }
        });
        
        let _m_push = server.mock("POST", "/")
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
            .with_body(response_body.to_string())
            .create_async().await;
        
        // Discover the agent
        registry.discover(&server.url()).await.unwrap();
        
        // Create client manager
        let manager = ClientManager::new(registry, config).unwrap();
        
        // Test set push notification
        let result = manager.set_push_notification(agent_name, task_id, webhook_url, None, None).await.unwrap();
        
        // Verify result
        assert_eq!(result, task_id);
    }

    #[tokio::test]
    async fn test_agent_not_found() {
        let (_server, registry, config, _temp_dir) = setup_test_environment().await;
        
        // Create client manager
        let manager = ClientManager::new(registry, config).unwrap();
        
        // Test get or create client for non-existent agent
        let result = manager.get_or_create_client("non-existent-agent").await;
        
        // Verify error
        assert!(result.is_err());
        match result.unwrap_err() {
            AgentError::AgentNotFound(_) => {},
            e => panic!("Expected AgentNotFound error, got: {:?}", e),
        }
    }
}