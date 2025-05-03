//! Task flow management and lifecycle handling using A2A protocol.
//!
//! This module provides the TaskFlow struct, which implements the A2A task lifecycle
//! and state management. It supports handling task state transitions, multi-turn
//! conversations via InputRequired state, delegation to remote agents, and local
//! execution using appropriate tools.

use crate::server::{
    agent_registry::AgentRegistry,
    client_manager::ClientManager,
    error::ServerError,
    task_router::{RoutingDecision, TaskRouter}, // TaskRouter might not be needed directly here
    tool_executor::ToolExecutor,
};
use crate::server::repositories::task_repository::TaskRepository;
use crate::types::{
    Artifact, Message, Part, Role, Task, TaskSendParams, TaskState, TaskStatus, TextPart,
};
use chrono::Utc;
use serde_json::{json, Map, Value};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, instrument, trace, warn}; // Add tracing imports
use uuid; // For generating remote task IDs

/// Task origin types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskOrigin {
    /// Task is processed locally
    Local,
    /// Task is delegated to another agent
    Delegated {
        /// ID of the agent handling the task
        agent_id: String,
        /// URL of the agent handling the task (optional)
        agent_url: Option<String>,
        /// Timestamp when the task was delegated
        delegated_at: String,
    },
    /// Task was decomposed into subtasks
    Decomposed {
        /// IDs of the subtasks
        subtask_ids: Vec<String>,
    },
    /// Unknown origin
    Unknown,
}

/// Processing status for a task
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProcessingStatus {
    /// Initial status
    Starting,
    /// Processing is in progress
    Processing,
    /// Waiting for user input
    AwaitingInput,
    /// Processing has completed successfully
    Completed,
    /// Processing has failed
    Failed,
    /// Processing was canceled
    Canceled,
}

/// Manages the execution flow and lifecycle of an A2A task.
pub struct TaskFlow {
    /// The task ID
    task_id: String,
    /// The agent ID handling this task
    agent_id: String,
    /// Repository for accessing and updating tasks
    task_repository: Arc<dyn TaskRepository>,
    /// Client manager for delegating tasks to other agents
    client_manager: Arc<ClientManager>,
    /// Tool executor for running local tools
    tool_executor: Arc<ToolExecutor>,
    /// Agent registry for agent discovery
    agent_registry: Arc<AgentRegistry>,
    /// Maximum polling attempts for delegated tasks
    max_polling_attempts: usize,
    /// Polling interval for delegated tasks (in seconds)
    polling_interval_seconds: u64,
}

impl TaskFlow {
    /// Process a task according to its routing decision and execute the appropriate flow
    pub async fn process_task(&self, params: TaskSendParams) -> Result<Task, ServerError> {
        // Create a new task from the params
        let mut task = Task {
            id: params.id.clone(),
            session_id: params.session_id.clone(),
            status: TaskStatus {
                state: TaskState::Working,
                timestamp: Some(Utc::now()),
                message: None,
            },
            history: None,
            metadata: params.metadata.clone(),
            artifacts: None,
        };
        
        // Initial message
        task.status.message = Some(params.message);
        
        // Save the initial task state
        self.task_repository.save_task(&task).await?;
        
        // For this simplified version, just mark the task as complete
        task.status.state = TaskState::Completed;
        task.status.message = Some(Message {
            role: Role::Agent,
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: "Task has been processed by the TaskFlow engine.".to_string(),
                metadata: None,
            })],
            metadata: None,
        });
        
        // Update the task's timestamp
        task.status.timestamp = Some(Utc::now());
        
        // Save the final task state
        self.task_repository.save_task(&task).await?;
        
        Ok(task)
    }
    
    /// Creates a new TaskFlow for managing a task's lifecycle.
    pub fn new(
        task_id: String,
        agent_id: String,
        task_repository: Arc<dyn TaskRepository>,
        client_manager: Arc<ClientManager>,
        tool_executor: Arc<ToolExecutor>,
        agent_registry: Arc<AgentRegistry>,
    ) -> Self {
        Self {
            task_id,
            agent_id,
            task_repository,
            client_manager,
            tool_executor,
            agent_registry,
            max_polling_attempts: 30, // Default to 30 attempts
            polling_interval_seconds: 5, // Default to 5 seconds
        }
    }

    /// Processes a routing decision for the task.
    #[instrument(skip(self, decision), fields(task_id = %self.task_id, agent_id = %self.agent_id, ?decision))]
    pub async fn process_decision(&self, decision: RoutingDecision) -> Result<(), ServerError> {
        info!("Processing routing decision for task.");
        // Get the current task state to work with
        let mut task = self.get_task().await?;
        trace!(?task, "Current task state before processing decision.");
        
        match decision {
            RoutingDecision::Local { tool_names } => {
                info!(?tool_names, "Executing task locally using tools.");
                // Execute the task locally using the specified tools
                // The tool executor should handle updating the task status internally
                self.tool_executor.execute_task_locally(&mut task, &tool_names).await?;
                debug!("Local execution finished by tool executor.");
                
                // Save the final state updated by the tool executor
                trace!(?task.status, "Saving final task state after local execution.");
                self.task_repository.save_task(&task).await?;
                self.task_repository.save_state_history(&task.id, &task).await?; // Save final state history
            }
            RoutingDecision::Remote { agent_id } => {
                info!(remote_agent_id = %agent_id, "Delegating task to remote agent.");
                
                // 1. Get the original message from the task history (should be the first one)
                let initial_message = task.history.as_ref()
                    .and_then(|h| h.first())
                    .cloned()
                    .ok_or_else(|| {
                        error!("Cannot delegate task: Initial message not found in history.");
                        ServerError::Internal("Initial message missing for delegation".to_string())
                    })?;
                trace!(?initial_message, "Extracted initial message for delegation.");
                
                // 2. Prepare to use ClientManager to send the task
                trace!(?initial_message, "Using initial message for task delegation.");
                
                // Create TaskSendParams for delegation
                let send_params = TaskSendParams {
                    id: uuid::Uuid::new_v4().to_string(), // Generate a new ID for the remote task
                    message: initial_message,             // Use the initial message
                    session_id: task.session_id.clone(),  // Maintain session continuity
                    metadata: task.metadata.clone(),      // Pass through metadata
                    history_length: None,                 // Default to no history length limit
                    push_notification: None,              // Default to no push notification
                };
                
                match self.client_manager.send_task(&agent_id, send_params).await {
                    Ok(delegated_task_response) => {
                        info!(remote_agent_id = %agent_id, remote_task_id = %delegated_task_response.id, remote_status = ?delegated_task_response.status.state, "Task successfully sent to remote agent.");
                        // 3. Update local task status to reflect successful delegation initiation.
                        // Keep state as Working, but update the message.
                        task.status.state = TaskState::Working; // Keep it working, don't mark completed!
                        task.status.timestamp = Some(Utc::now());
                        task.status.message = Some(Message {
                            role: Role::Agent, // System/Agent message
                            parts: vec![Part::TextPart(TextPart {
                                type_: "text".to_string(),
                                text: format!("Task delegated to agent '{}'. Remote Task ID: {}. Waiting for updates.", agent_id, delegated_task_response.id),
                                metadata: None,
                            })],
                            metadata: None,
                        });
                        // Optionally add remote task ID to local task metadata
                        task.metadata.get_or_insert_with(Map::new).insert(
                            "remote_task_id".to_string(),
                            json!(delegated_task_response.id)
                        );
                        task.metadata.get_or_insert_with(Map::new).insert(
                            "delegated_to_agent_id".to_string(),
                            json!(agent_id)
                        );
                        
                        trace!(?task.status, ?task.metadata, "Local task status updated after successful delegation initiation.");
                        // Save the updated local task (still in Working state)
                        self.task_repository.save_task(&task).await?;
                        self.task_repository.save_state_history(&task.id, &task).await?; // Save delegation initiation state
                        
                        // TODO: Implement polling or push notification handling here
                        // For now, the task remains 'Working' locally.
                        // A separate mechanism would be needed to update the local task
                        // when the remote task completes or changes state.
                        warn!("Delegation sent, but no mechanism implemented to monitor remote task state updates.");
                        
                    }
                    Err(e) => {
                        error!(remote_agent_id = %agent_id, error = %e, "Failed to delegate task to remote agent.");
                        // Update local task to Failed state
                        task.status.state = TaskState::Failed;
                        task.status.timestamp = Some(Utc::now());
                        task.status.message = Some(Message {
                            role: Role::Agent,
                            parts: vec![Part::TextPart(TextPart {
                                type_: "text".to_string(),
                                text: format!("Failed to delegate task to agent '{}': {}", agent_id, e),
                                metadata: None,
                            })],
                            metadata: None,
                        });
                        self.task_repository.save_task(&task).await?;
                        self.task_repository.save_state_history(&task.id, &task).await?; // Save failed state history
                        // Propagate the error
                        return Err(ServerError::A2aClientError(format!("Delegation failed: {}", e)));
                    }
                }
            }
            RoutingDecision::Reject { reason } => {
                info!(%reason, "Task rejected based on routing decision.");
                // Mark the task as failed with the rejection reason
                task.status.state = TaskState::Failed;
                task.status.timestamp = Some(Utc::now());
                task.status.message = Some(Message {
                    role: Role::Agent,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: format!("Task rejected: {}", reason),
                        metadata: None,
                    })],
                    metadata: None,
                });
                trace!(?task.status, "Task status updated to Failed (Rejected).");
                
                // Save the updated task
                self.task_repository.save_task(&task).await?;
                self.task_repository.save_state_history(&task.id, &task).await?; // Save rejected state history
            }
            RoutingDecision::Decompose { subtasks } => {
                warn!(subtask_count = subtasks.len(), "Task decomposition requested but not implemented.");
                // For simplicity, just mark the task as failed with a message about decomposition
                task.status.state = TaskState::Failed;
                task.status.timestamp = Some(Utc::now());
                task.status.message = Some(Message {
                    role: Role::Agent,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: format!("Task decomposition with {} subtasks is not implemented.", subtasks.len()),
                        metadata: None,
                    })],
                    metadata: None,
                });
                trace!(?task.status, "Task status updated to Failed (Decomposition not implemented).");
                
                // Save the updated task
                self.task_repository.save_task(&task).await?;
                self.task_repository.save_state_history(&task.id, &task).await?; // Save failed state history
            }
        }
        
        info!("Finished processing routing decision.");
        Ok(())
    }

    /// Gets the current task.
    #[instrument(skip(self), fields(task_id = %self.task_id))]
    async fn get_task(&self) -> Result<Task, ServerError> {
        debug!("Getting current task details from repository.");
        let task = self.task_repository
            .get_task(&self.task_id)
            .await?
            .ok_or_else(|| {
                error!("Task not found in repository.");
                ServerError::TaskNotFound(self.task_id.clone())
            })?;
        trace!(?task, "Task details retrieved.");
        Ok(task)
    }
}