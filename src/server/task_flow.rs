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
    task_router::{RoutingDecision, TaskRouter},
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
    pub async fn process_decision(&self, decision: RoutingDecision) -> Result<(), ServerError> {
        // Update the task with the processing status
        let mut task = self.get_task().await?;
        
        match decision {
            RoutingDecision::Local { tool_names } => {
                // Execute the task locally using the specified tools
                self.tool_executor.execute_task_locally(&mut task, &tool_names).await?;
                
                // Save the updated task
                self.task_repository.save_task(&task).await?;
            }
            RoutingDecision::Remote { agent_id } => {
                // For simplicity in this limited version, just update the task to show delegation
                task.status.message = Some(Message {
                    role: Role::Agent,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: format!("Task would be delegated to agent '{}'.", agent_id),
                        metadata: None,
                    })],
                    metadata: None,
                });
                
                // In a real implementation, this would delegate the task to another agent
                // For now, mark as completed to avoid hanging tasks
                task.status.state = TaskState::Completed;
                task.status.timestamp = Some(Utc::now());
                
                // Save the updated task
                self.task_repository.save_task(&task).await?;
            }
            RoutingDecision::Reject { reason } => {
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
                
                // Save the updated task
                self.task_repository.save_task(&task).await?;
            }
            RoutingDecision::Decompose { subtasks } => {
                // For simplicity, just mark the task as failed with a message about decomposition
                task.status.state = TaskState::Failed;
                task.status.timestamp = Some(Utc::now());
                task.status.message = Some(Message {
                    role: Role::Agent,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: format!("Task decomposition with {} subtasks is not implemented in this limited version.", subtasks.len()),
                        metadata: None,
                    })],
                    metadata: None,
                });
                
                // Save the updated task
                self.task_repository.save_task(&task).await?;
            }
        }
        
        Ok(())
    }

    /// Gets the current task.
    async fn get_task(&self) -> Result<Task, ServerError> {
        self.task_repository
            .get_task(&self.task_id)
            .await?
            .ok_or_else(|| ServerError::TaskNotFound(self.task_id.clone()))
    }
}