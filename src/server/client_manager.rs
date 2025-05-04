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
};
use crate::server::error::ServerError;
use crate::server::agent_registry::AgentRegistry;

use dashmap::DashMap;
use std::sync::Arc;
use std::collections::HashMap;
use chrono::Utc;
use async_trait::async_trait;

/// Manages cached A2aClient instances for different remote agents.
#[derive(Clone)]
pub struct ClientManager {
    /// Cache of clients, keyed by agent ID
    clients: Arc<DashMap<String, A2aClient>>,
    
    /// Reference to the agent registry to get agent URLs
    registry: Arc<AgentRegistry>,
    
    /// Active delegation tracking
    delegations: Arc<DashMap<String, DelegationInfo>>,
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
    async fn on_remote_state_change(&self, local_task_id: &str, remote_task: &Task) -> Result<(), ServerError>;
    
    /// Called when a remote task receives a new artifact
    async fn on_remote_artifact(&self, local_task_id: &str, remote_task_id: &str, artifact: &Artifact) -> Result<(), ServerError>;
    
    /// Called when a remote task completes
    async fn on_remote_completion(&self, local_task_id: &str, remote_task: &Task) -> Result<(), ServerError>;
    
    /// Called when a remote task fails
    async fn on_remote_failure(&self, local_task_id: &str, remote_task: &Task) -> Result<(), ServerError>;
}

impl ClientManager {
    /// Creates a new ClientManager.
    pub fn new(registry: Arc<AgentRegistry>) -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
            registry,
            delegations: Arc::new(DashMap::new()),
        }
    }

    /// Gets an existing A2aClient for the target agent or creates a new one.
    pub async fn get_or_create_client(&self, agent_id: &str) -> Result<A2aClient, ServerError> {
        // Fast path: Check if client already exists in cache
        if let Some(client_entry) = self.clients.get(agent_id) {
            return Ok(client_entry.value().clone());
        }

        // Slow path: Client not cached, need to create it
        // 1. Get agent info from registry
        let agent_url = self.registry.get_agent_url(agent_id).await?;

        // 2. Create the A2aClient instance
        let a2a_client = A2aClient::new(&agent_url);

        // 3. Cache the new client
        let client_entry = self.clients.entry(agent_id.to_string()).or_insert(a2a_client);

        // Return the client
        Ok(client_entry.value().clone())
    }

    /// Sends a task to a remote agent using the managed client.
    pub async fn send_task(&self, agent_id: &str, params: TaskSendParams) -> Result<Task, ServerError> {
        // Get or create the client
        let mut client = self.get_or_create_client(agent_id).await?;
        
        // Use the A2A client to send the task
        match client.send_jsonrpc::<Task>("tasks/send", serde_json::to_value(params.clone()).unwrap()).await {
            Ok(task) => {
                // Register delegation for the task
                let local_task_id = params.id.clone();
                self.register_delegation(&local_task_id, agent_id, &task.id);
                
                Ok(task)
            },
            Err(e) => {
                Err(ServerError::A2aClientError(format!("Failed to send task to agent '{}': {}", agent_id, e)))
            }
        }
    }

    /// Retrieves the status of a task from a remote agent.
    pub async fn get_task_status(&self, agent_id: &str, task_id: &str) -> Result<Task, ServerError> {
        // Get or create the client
        let mut client = self.get_or_create_client(agent_id).await?;
        
        // Use the A2A client to get the task status
        match client.get_task(task_id).await {
            Ok(task) => Ok(task),
            Err(e) => {
                Err(ServerError::A2aClientError(format!("Failed to get task status from agent '{}': {}", agent_id, e)))
            }
        }
    }
    
    /// Cancels a task on a remote agent.
    pub async fn cancel_task(&self, agent_id: &str, task_id: &str) -> Result<Task, ServerError> {
        // Get or create the client
        let mut client = self.get_or_create_client(agent_id).await?; // Keep mut here, it's used later
        
        // Create the task ID params
        let params = TaskIdParams {
            id: task_id.to_string(),
            metadata: None,
        };
        
        // Use the A2A client to cancel the task
        match client.send_jsonrpc::<Task>("tasks/cancel", serde_json::to_value(params).unwrap()).await {
            Ok(task) => Ok(task),
            Err(e) => {
                Err(ServerError::A2aClientError(format!("Failed to cancel task '{}' on agent '{}': {}", task_id, agent_id, e)))
            }
        }
    }

    /// Gets an agent card for the specified agent ID.
    pub async fn get_agent_card(&self, agent_id: &str) -> Result<AgentCard, ServerError> {
        // Try to fetch it directly from the agent
        let mut client = self.get_or_create_client(agent_id).await?;
        
        // Use the A2A client to get the agent card
        match client.get_agent_card().await {
            Ok(card) => Ok(card),
            Err(e) => {
                Err(ServerError::A2aClientError(format!("Failed to get agent card for '{}': {}", agent_id, e)))
            }
        }
    }

    /// Register a delegation in the tracking system
    fn register_delegation(&self, local_task_id: &str, agent_id: &str, remote_task_id: &str) {
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
}
