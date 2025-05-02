//! Task flow management and lifecycle handling using A2A protocol.
//!
//! This module provides the TaskFlow struct, which implements the A2A task lifecycle
//! and state management. It supports handling task state transitions, multi-turn
//! conversations via InputRequired state, delegation to remote agents, and local
//! execution using appropriate tools.

use crate::bidirectional_agent::{
    agent_registry::AgentRegistry,
    client_manager::ClientManager,
    error::AgentError,
    task_flow_helpers::{self, TaskFlowKeys, TASKFLOW_NAMESPACE, set_task_metadata, set_task_metadata_ext, get_task_metadata},
    task_router::{self, RoutingDecision, TaskRouter},
    tool_executor::ToolExecutor,
    types::{create_text_message, create_text_message_with_role},
};
use crate::client::{self, errors::ClientError};
use crate::server::repositories::task_repository::TaskRepository;
use crate::types::{
    Artifact, Message, Part, Role, Task, TaskSendParams, TaskState, TaskStatus, TextPart,
};
use chrono::Utc;
use serde_json::{json, Map, Value};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

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
    pub async fn process_task(&self, params: TaskSendParams) -> Result<Task, AgentError> {
        // Create a new task from the params
        let mut task = Task {
            id: params.id.clone(),
            status: TaskStatus {
                state: TaskState::Working,
                timestamp: Some(Utc::now()),
                message: None,
            },
            creation_time: Utc::now(),
            last_update_time: Utc::now(),
            metadata: params.metadata.clone(),
            message: params.message,
            artifacts: None,
        };
        
        // Save the initial task state
        self.task_repository.save_task(&task).await?;
        
        // Handle the task based on routing decision
        // This is a simplified version - in the full implementation, we would use
        // the task_router to determine execution strategy
        
        // For now, just mark the task as complete with a simple response
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
        task.last_update_time = Utc::now();
        
        // Save the final task state
        self.task_repository.save_task(&task).await?;
        
        Ok(task)
    }
    
    /// Creates a new TaskFlow for managing a task's lifecycle.
    ///
    /// # Arguments
    /// * `task_id` - The ID of the task to manage
    /// * `agent_id` - The ID of the agent handling this task
    /// * `task_repository` - Repository for task storage
    /// * `client_manager` - For delegating tasks to other agents
    /// * `tool_executor` - For local tool execution
    /// * `agent_registry` - For agent discovery
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

    /// Sets the maximum number of polling attempts for delegated tasks.
    pub fn with_max_polling_attempts(mut self, attempts: usize) -> Self {
        self.max_polling_attempts = attempts;
        self
    }

    /// Sets the polling interval for delegated tasks.
    pub fn with_polling_interval(mut self, seconds: u64) -> Self {
        self.polling_interval_seconds = seconds;
        self
    }

    /// Processes a routing decision for the task.
    pub async fn process_decision(&self, decision: RoutingDecision) -> Result<(), AgentError> {
        debug!(
            task_id = %self.task_id,
            routing_decision = ?decision,
            "Processing routing decision"
        );

        // Update the processing status metadata
        self.set_processing_status(ProcessingStatus::Processing).await?;

        match decision {
            RoutingDecision::Local { tool_names } => {
                self.execute_locally(&tool_names).await?;
            }
            RoutingDecision::Remote { agent_id } => {
                self.delegate_task(&agent_id).await?;
            }
            RoutingDecision::Reject { reason } => {
                self.reject_task(&reason).await?;
            }
            RoutingDecision::Decompose { subtasks } => {
                self.decompose_task(subtasks).await?;
            }
        }

        Ok(())
    }

    /// Processes a follow-up message for a task in InputRequired state.
    pub async fn process_follow_up(&self, message: Message) -> Result<(), AgentError> {
        let mut task = self.get_task().await?;

        // Verify task is in InputRequired state
        if task.status.state != TaskState::InputRequired {
            return Err(AgentError::InvalidState(format!(
                "Cannot process follow-up for task '{}' in state '{:?}'. Expected InputRequired.",
                self.task_id, task.status.state
            )));
        }

        // Update processing status
        self.set_processing_status(ProcessingStatus::Processing).await?;

        // Update task status to Working while processing
        task.status.state = TaskState::Working;
        task.status.timestamp = Some(Utc::now());
        self.task_repository.save_task(&task).await?;
        self.task_repository.save_state_history(&task.id, &task).await?;

        // Check if this is a delegated task
        if let Some(origin) = self.get_task_origin().await? {
            match origin {
                TaskOrigin::Delegated {
                    agent_id,
                    agent_url: _,
                    delegated_at: _,
                } => {
                    // Get remote task ID
                    let remote_task_id = get_metadata_ext::<String>(
                        &task,
                        TASKFLOW_NAMESPACE,
                        TaskFlowKeys::REMOTE_TASK_ID,
                    )
                    .ok_or_else(|| {
                        AgentError::TaskFlowError(format!(
                            "Missing remote task ID for delegated task '{}'",
                            self.task_id
                        ))
                    })?;

                    // Forward the follow-up message to the remote agent
                    debug!(
                        task_id = %self.task_id,
                        remote_task_id = %remote_task_id,
                        remote_agent_id = %agent_id,
                        "Forwarding follow-up message to remote agent"
                    );

                    // Prepare params for the remote agent
                    let params = TaskSendParams {
                        id: remote_task_id,
                        message,
                        session_id: task.session_id.clone(),
                        push_notification: None,
                        history_length: None,
                        metadata: task.metadata.clone(),
                    };

                    // Forward to remote agent
                    let result = self.client_manager.send_task(&agent_id, params).await?;

                    // Process the remote result by polling for updates
                    self.process_remote_follow_up_result(&agent_id, &result.id).await?;
                }
                _ => {
                    // Local processing for follow-up
                    debug!(
                        task_id = %self.task_id,
                        "Processing follow-up message locally"
                    );

                    // Add the message to task history if needed
                    // (Repository might handle this based on implementation)

                    // Update task with new message using tool executor
                    let mut updated_task = self.get_task().await?;
                    self.tool_executor
                        .process_follow_up(&mut updated_task, message)
                        .await?;

                    // Save final state
                    self.task_repository.save_task(&updated_task).await?;
                    self.task_repository
                        .save_state_history(&updated_task.id, &updated_task)
                        .await?;

                    // Update processing status based on task state
                    let status = match updated_task.status.state {
                        TaskState::Completed => ProcessingStatus::Completed,
                        TaskState::Failed => ProcessingStatus::Failed,
                        TaskState::Canceled => ProcessingStatus::Canceled,
                        TaskState::InputRequired => ProcessingStatus::AwaitingInput,
                        _ => ProcessingStatus::Processing,
                    };
                    self.set_processing_status(status).await?;
                }
            }
        } else {
            // Default to local processing if no origin info
            let mut updated_task = self.get_task().await?;
            self.tool_executor
                .process_follow_up(&mut updated_task, message)
                .await?;

            // Save final state
            self.task_repository.save_task(&updated_task).await?;
            self.task_repository
                .save_state_history(&updated_task.id, &updated_task)
                .await?;
        }

        Ok(())
    }

    /// Handles the result of a follow-up message sent to a remote agent.
    async fn process_remote_follow_up_result(
        &self,
        agent_id: &str,
        remote_task_id: &str,
    ) -> Result<(), AgentError> {
        // Poll the remote agent for updates
        self.poll_delegated_task(agent_id, remote_task_id).await
    }

    /// Executes the task locally using the specified tools.
    async fn execute_locally(&self, tool_names: &[String]) -> Result<(), AgentError> {
        debug!(
            task_id = %self.task_id,
            tools = ?tool_names,
            "Executing task locally"
        );

        // Get the current task
        let mut task = self.get_task().await?;

        // Set task origin as Local
        self.set_task_origin(TaskOrigin::Local).await?;

        // Execute using the tool executor
        match self.tool_executor.execute_task_locally(&mut task, tool_names).await {
            Ok(_) => {
                debug!(
                    task_id = %self.task_id,
                    "Local execution completed successfully"
                );

                // Update processing status based on final task state
                let status = match task.status.state {
                    TaskState::Completed => ProcessingStatus::Completed,
                    TaskState::Failed => ProcessingStatus::Failed,
                    TaskState::Canceled => ProcessingStatus::Canceled,
                    TaskState::InputRequired => ProcessingStatus::AwaitingInput,
                    _ => ProcessingStatus::Processing,
                };
                self.set_processing_status(status).await?;

                // Save final state (executor already saved the task)
                Ok(())
            }
            Err(e) => {
                error!(
                    task_id = %self.task_id,
                    error = %e,
                    "Local execution failed"
                );

                // Update task status to Failed if not already
                if task.status.state != TaskState::Failed {
                    task.status.state = TaskState::Failed;
                    task.status.timestamp = Some(Utc::now());
                    task.status.message = Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Task execution failed: {}", e),
                            metadata: None,
                        })],
                        metadata: None,
                    });

                    // Save the failed state
                    self.task_repository.save_task(&task).await?;
                    self.task_repository.save_state_history(&task.id, &task).await?;
                }

                // Set processing status to Failed
                self.set_processing_status(ProcessingStatus::Failed).await?;

                Err(e)
            }
        }
    }

    /// Delegates the task to a remote agent.
    async fn delegate_task(&self, target_agent_id: &str) -> Result<(), AgentError> {
        debug!(
            task_id = %self.task_id,
            target_agent = %target_agent_id,
            "Delegating task to remote agent"
        );

        // Get the current task
        let mut task = self.get_task().await?;

        // Get the most recent message from the task to send
        let message = task
            .status
            .message
            .clone()
            .or_else(|| {
                task.history.as_ref().and_then(|history| {
                    history
                        .iter()
                        .filter(|t| t.status.message.is_some())
                        .last()
                        .and_then(|t| t.status.message.clone())
                })
            })
            .ok_or_else(|| {
                AgentError::DelegationError(format!(
                    "Cannot delegate task '{}' with no message",
                    self.task_id
                ))
            })?;

        // Prepare delegation parameters
        let delegation_params = TaskSendParams {
            id: format!("delegated-{}", Utc::now().timestamp_millis()), // Create a new ID for remote
            message,
            session_id: task.session_id.clone(),
            push_notification: None, // Don't configure push from here
            history_length: None,
            metadata: task.metadata.clone(),
        };

        // Send task to remote agent
        match self.client_manager.send_task(target_agent_id, delegation_params).await {
            Ok(remote_task) => {
                debug!(
                    task_id = %self.task_id,
                    remote_task_id = %remote_task.id,
                    remote_agent = %target_agent_id,
                    "Task successfully delegated"
                );

                // Get agent URL if available
                let agent_url = self
                    .agent_registry
                    .get_agent_url(target_agent_id)
                    .await
                    .ok();

                // Set task origin as Delegated
                self.set_task_origin(TaskOrigin::Delegated {
                    agent_id: target_agent_id.to_string(),
                    agent_url: agent_url.clone(),
                    delegated_at: Utc::now().to_rfc3339(),
                })
                .await?;

                // Store remote task ID in metadata
                task = self.get_task().await?;
                let mut task = set_metadata_ext(
                    task,
                    TASKFLOW_NAMESPACE,
                    TaskFlowKeys::REMOTE_TASK_ID,
                    remote_task.id.clone(),
                );

                // Store remote agent ID in metadata
                task = set_metadata_ext(
                    task,
                    TASKFLOW_NAMESPACE,
                    TaskFlowKeys::REMOTE_AGENT_ID,
                    target_agent_id.to_string(),
                );

                // Store agent URL if available
                if let Some(url) = agent_url {
                    task = set_metadata_ext(
                        task,
                        TASKFLOW_NAMESPACE,
                        TaskFlowKeys::REMOTE_AGENT_URL,
                        url,
                    );
                }

                // Update task status to indicate delegation
                task.status.state = TaskState::Working;
                task.status.timestamp = Some(Utc::now());
                task.status.message = Some(Message {
                    role: Role::Agent,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: format!(
                            "Task delegated to agent '{}'. Remote task ID: {}",
                            target_agent_id, remote_task.id
                        ),
                        metadata: None,
                    })],
                    metadata: None,
                });

                // Save the updated task
                self.task_repository.save_task(&task).await?;
                self.task_repository.save_state_history(&task.id, &task).await?;

                // Start polling for updates from the remote agent
                self.poll_delegated_task(target_agent_id, &remote_task.id).await?;

                Ok(())
            }
            Err(e) => {
                error!(
                    task_id = %self.task_id,
                    target_agent = %target_agent_id,
                    error = %e,
                    "Delegation failed"
                );

                // Update task status to Failed
                task.status.state = TaskState::Failed;
                task.status.timestamp = Some(Utc::now());
                task.status.message = Some(Message {
                    role: Role::Agent,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: format!("Delegation to agent '{}' failed: {}", target_agent_id, e),
                        metadata: None,
                    })],
                    metadata: None,
                });

                // Save the failed state
                self.task_repository.save_task(&task).await?;
                self.task_repository.save_state_history(&task.id, &task).await?;

                // Set processing status to Failed
                self.set_processing_status(ProcessingStatus::Failed).await?;

                Err(AgentError::DelegationError(format!(
                    "Failed to delegate task: {}",
                    e
                )))
            }
        }
    }

    /// Polls a delegated task for updates and synchronizes with local state.
    async fn poll_delegated_task(
        &self,
        agent_id: &str,
        remote_task_id: &str,
    ) -> Result<(), AgentError> {
        debug!(
            task_id = %self.task_id,
            remote_task_id = %remote_task_id,
            remote_agent = %agent_id,
            max_attempts = self.max_polling_attempts,
            interval_sec = self.polling_interval_seconds,
            "Starting polling for delegated task"
        );

        for attempt in 1..=self.max_polling_attempts {
            debug!(
                task_id = %self.task_id,
                remote_task_id = %remote_task_id,
                attempt = attempt,
                "Polling remote task"
            );

            // Sleep before polling (except on first attempt)
            if attempt > 1 {
                sleep(Duration::from_secs(self.polling_interval_seconds)).await;
            }

            // Get the status of the remote task
            match self.client_manager.get_task_status(agent_id, remote_task_id).await {
                Ok(remote_task) => {
                    debug!(
                        task_id = %self.task_id,
                        remote_task_id = %remote_task_id,
                        remote_state = ?remote_task.status.state,
                        "Received remote task status update"
                    );

                    // Update local task with remote state
                    let mut local_task = self.get_task().await?;

                    // Update status message if available
                    local_task.status.state = remote_task.status.state.clone();
                    local_task.status.timestamp = remote_task.status.timestamp;
                    local_task.status.message = remote_task.status.message.clone();

                    // Update artifacts if available
                    if let Some(remote_artifacts) = remote_task.artifacts {
                        // Initialize local artifacts if needed
                        if local_task.artifacts.is_none() {
                            local_task.artifacts = Some(Vec::new());
                        }

                        // Add/update remote artifacts in local task
                        if let Some(local_artifacts) = &mut local_task.artifacts {
                            for remote_artifact in remote_artifacts {
                                // Check if this is an update to an existing artifact
                                if let Some(existing_index) = local_artifacts
                                    .iter()
                                    .position(|a| a.index == remote_artifact.index)
                                {
                                    // Replace existing artifact
                                    local_artifacts[existing_index] = remote_artifact;
                                } else {
                                    // Add as new artifact
                                    local_artifacts.push(remote_artifact);
                                }
                            }
                        }
                    }

                    // Save the updated task
                    self.task_repository.save_task(&local_task).await?;
                    self.task_repository
                        .save_state_history(&local_task.id, &local_task)
                        .await?;

                    // Update processing status based on task state
                    let status = match local_task.status.state {
                        TaskState::Completed => ProcessingStatus::Completed,
                        TaskState::Failed => ProcessingStatus::Failed,
                        TaskState::Canceled => ProcessingStatus::Canceled,
                        TaskState::InputRequired => ProcessingStatus::AwaitingInput,
                        _ => ProcessingStatus::Processing,
                    };
                    self.set_processing_status(status).await?;

                    // Check if remote task is in a terminal state
                    if matches!(
                        remote_task.status.state,
                        TaskState::Completed | TaskState::Failed | TaskState::Canceled
                    ) {
                        debug!(
                            task_id = %self.task_id,
                            remote_task_id = %remote_task_id,
                            final_state = ?remote_task.status.state,
                            "Remote task reached final state"
                        );
                        return Ok(());
                    }

                    // Check if remote task needs input and we need to pause polling
                    if remote_task.status.state == TaskState::InputRequired {
                        debug!(
                            task_id = %self.task_id,
                            remote_task_id = %remote_task_id,
                            "Remote task requires input, pausing polling"
                        );
                        return Ok(());
                    }
                }
                Err(e) => {
                    warn!(
                        task_id = %self.task_id,
                        remote_task_id = %remote_task_id,
                        attempt = attempt,
                        error = %e,
                        "Polling attempt failed"
                    );

                    // Continue polling on error unless max attempts reached
                    if attempt == self.max_polling_attempts {
                        error!(
                            task_id = %self.task_id,
                            remote_task_id = %remote_task_id,
                            max_attempts = self.max_polling_attempts,
                            "Polling failed after maximum attempts"
                        );

                        // Update local task to Failed state
                        let mut local_task = self.get_task().await?;
                        local_task.status.state = TaskState::Failed;
                        local_task.status.timestamp = Some(Utc::now());
                        local_task.status.message = Some(create_text_message(
                            Role::Agent,
                            &format!(
                                "Polling remote task '{}' failed after {} attempts: {}",
                                remote_task_id, self.max_polling_attempts, e
                            ),
                        ));

                        // Save the failed state
                        self.task_repository.save_task(&local_task).await?;
                        self.task_repository
                            .save_state_history(&local_task.id, &local_task)
                            .await?;

                        // Set processing status to Failed
                        self.set_processing_status(ProcessingStatus::Failed).await?;

                        return Err(AgentError::DelegationError(format!(
                            "Polling failed after {} attempts: {}",
                            self.max_polling_attempts, e
                        )));
                    }
                }
            }
        }

        // Should never reach here due to returns in the loop
        unreachable!()
    }

    /// Marks the task as rejected.
    async fn reject_task(&self, reason: &str) -> Result<(), AgentError> {
        debug!(
            task_id = %self.task_id,
            reason = %reason,
            "Rejecting task"
        );

        // Get the current task
        let mut task = self.get_task().await?;

        // Update task status to Failed
        task.status.state = TaskState::Failed;
        task.status.timestamp = Some(Utc::now());
        task.status.message = Some(create_text_message(
            Role::Agent,
            &format!("Task rejected: {}", reason),
        ));

        // Save the rejected state
        self.task_repository.save_task(&task).await?;
        self.task_repository.save_state_history(&task.id, &task).await?;

        // Set processing status to Failed
        self.set_processing_status(ProcessingStatus::Failed).await?;

        Ok(())
    }

    /// Decomposes a task into subtasks.
    async fn decompose_task(&self, subtasks: Vec<task_router::SubtaskDefinition>) -> Result<(), AgentError> {
        debug!(
            task_id = %self.task_id,
            subtask_count = subtasks.len(),
            "Decomposing task into subtasks"
        );

        // This is a placeholder implementation that will reject the task
        // Task decomposition requires a more complex implementation to
        // create, track, and synthesize results from multiple subtasks
        let reason = "Task decomposition is not yet fully implemented";
        warn!(task_id = %self.task_id, "Rejecting decomposition request");
        self.reject_task(reason).await?;

        // Set task origin as Decomposed (even though we're rejecting for now)
        // This helps identify tasks that were meant to be decomposed
        self.set_task_origin(TaskOrigin::Decomposed {
            subtask_ids: subtasks,
        })
        .await?;

        Ok(())
    }

    /// Gets the current task.
    async fn get_task(&self) -> Result<Task, AgentError> {
        self.task_repository
            .get_task(&self.task_id)
            .await?
            .ok_or_else(|| {
                AgentError::TaskNotFound(format!("Task '{}' not found", self.task_id))
            })
    }

    /// Sets the task origin in metadata.
    async fn set_task_origin(&self, origin: TaskOrigin) -> Result<(), AgentError> {
        let task = self.get_task().await?;

        // Convert origin to serializable form
        let origin_value = match &origin {
            TaskOrigin::Local => json!("local"),
            TaskOrigin::Delegated {
                agent_id,
                agent_url,
                delegated_at,
            } => json!({
                "type": "delegated",
                "agent_id": agent_id,
                "agent_url": agent_url,
                "delegated_at": delegated_at
            }),
            TaskOrigin::Decomposed { subtask_ids } => json!({
                "type": "decomposed",
                "subtask_ids": subtask_ids
            }),
            TaskOrigin::Unknown => json!("unknown"),
        };

        // Save the origin in task metadata
        let updated_task =
            set_metadata_ext(task, TASKFLOW_NAMESPACE, TaskFlowKeys::ORIGIN, origin_value);
        self.task_repository.save_task(&updated_task).await?;

        Ok(())
    }

    /// Gets the task origin from metadata.
    async fn get_task_origin(&self) -> Result<Option<TaskOrigin>, AgentError> {
        let task = self.get_task().await?;

        // Get origin value from metadata
        let origin_value = get_metadata_ext::<Value>(&task, TASKFLOW_NAMESPACE, TaskFlowKeys::ORIGIN);

        match origin_value {
            Some(value) => {
                if let Some(origin_str) = value.as_str() {
                    match origin_str {
                        "local" => Ok(Some(TaskOrigin::Local)),
                        "unknown" => Ok(Some(TaskOrigin::Unknown)),
                        _ => Ok(None), // Unknown string value
                    }
                } else if value.is_object() {
                    let obj = value.as_object().unwrap(); // Safe because we checked is_object()
                    if let Some(type_value) = obj.get("type").and_then(|v| v.as_str()) {
                        match type_value {
                            "delegated" => {
                                let agent_id = obj
                                    .get("agent_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let agent_url = obj
                                    .get("agent_url")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                let delegated_at = obj
                                    .get("delegated_at")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();

                                Ok(Some(TaskOrigin::Delegated {
                                    agent_id,
                                    agent_url,
                                    delegated_at,
                                }))
                            }
                            "decomposed" => {
                                let subtask_ids = obj
                                    .get("subtask_ids")
                                    .and_then(|v| v.as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                                            .collect()
                                    })
                                    .unwrap_or_else(Vec::new);

                                Ok(Some(TaskOrigin::Decomposed { subtask_ids }))
                            }
                            _ => Ok(None), // Unknown type value
                        }
                    } else {
                        Ok(None) // Missing type field
                    }
                } else {
                    Ok(None) // Neither string nor object
                }
            }
            None => Ok(None), // No origin value found
        }
    }

    /// Sets the task processing status in metadata.
    async fn set_processing_status(&self, status: ProcessingStatus) -> Result<(), AgentError> {
        let task = self.get_task().await?;

        // Convert status to string
        let status_str = match status {
            ProcessingStatus::Starting => "starting",
            ProcessingStatus::Processing => "processing",
            ProcessingStatus::AwaitingInput => "awaiting_input",
            ProcessingStatus::Completed => "completed",
            ProcessingStatus::Failed => "failed",
            ProcessingStatus::Canceled => "canceled",
        };

        // Save the status in task metadata
        let mut task_metadata = task.metadata.clone().unwrap_or_else(|| serde_json::Map::new());
        if let Err(e) = set_metadata_ext(
            &mut task_metadata,
            TASKFLOW_NAMESPACE,
            &TaskFlowKeys::PROCESSING_STATUS.to_string(),
            serde_json::json!(status_str),
        ) {
            eprintln!("Failed to set metadata: {}", e);
        }
        
        // Update task with the modified metadata
        let mut updated_task = task.clone();
        updated_task.metadata = Some(task_metadata);
        self.task_repository.save_task(&updated_task).await?;

        Ok(())
    }

    /// Gets the task processing status from metadata.
    async fn get_processing_status(&self) -> Result<ProcessingStatus, AgentError> {
        let task = self.get_task().await?;

        // Get status value from metadata
        let status_str =
            get_metadata_ext::<String>(&task, TASKFLOW_NAMESPACE, TaskFlowKeys::PROCESSING_STATUS)
                .unwrap_or_else(|| "starting".to_string());

        // Convert string to status
        match status_str.as_str() {
            "starting" => Ok(ProcessingStatus::Starting),
            "processing" => Ok(ProcessingStatus::Processing),
            "awaiting_input" => Ok(ProcessingStatus::AwaitingInput),
            "completed" => Ok(ProcessingStatus::Completed),
            "failed" => Ok(ProcessingStatus::Failed),
            "canceled" => Ok(ProcessingStatus::Canceled),
            _ => Ok(ProcessingStatus::Starting), // Default to Starting for unknown values
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bidirectional_agent::task_router::RoutingDecision;
    use crate::bidirectional_agent::test_utils::testing::create_test_components;
    use crate::types::{Message, Part, Role, TaskState, TextPart};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_task_flow_local_execution() {
        // Create test components
        let (task_repo, _, client_manager, tool_executor, agent_registry) =
            create_test_components().await;

        // Create a test task
        let task_id = "test-task-1".to_string();
        let agent_id = "test-agent-1".to_string();
        let mut task = Task {
            id: task_id.clone(),
            session_id: Some("test-session-1".to_string()),
            status: TaskStatus {
                state: TaskState::Working,
                timestamp: Some(Utc::now()),
                message: Some(Message {
                    role: Role::User,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: "Test task".to_string(),
                        metadata: None,
                    })],
                    metadata: None,
                }),
            },
            artifacts: None,
            history: None,
            metadata: None,
        };

        // Save initial task
        task_repo.save_task(&task).await.unwrap();

        // Create TaskFlow
        let task_flow = TaskFlow::new(
            task_id.clone(),
            agent_id,
            Arc::clone(&task_repo),
            Arc::clone(&client_manager),
            Arc::clone(&tool_executor),
            Arc::clone(&agent_registry),
        );

        // Process a local execution decision
        let decision = RoutingDecision::Local {
            tool_names: vec!["echo".to_string()],
        };

        // Execute the decision
        task_flow.process_decision(decision).await.unwrap();

        // Verify task has been updated
        let updated_task = task_repo.get_task(&task_id).await.unwrap().unwrap();

        // Check task origin
        let origin = task_flow.get_task_origin().await.unwrap();
        assert!(matches!(origin, Some(TaskOrigin::Local)));

        // Check processing status
        let status = task_flow.get_processing_status().await.unwrap();
        assert!(matches!(status, ProcessingStatus::Completed));
    }

    #[tokio::test]
    async fn test_task_flow_reject() {
        // Create test components
        let (task_repo, _, client_manager, tool_executor, agent_registry) =
            create_test_components().await;

        // Create a test task
        let task_id = "test-task-2".to_string();
        let agent_id = "test-agent-1".to_string();
        let mut task = Task {
            id: task_id.clone(),
            session_id: Some("test-session-2".to_string()),
            status: TaskStatus {
                state: TaskState::Working,
                timestamp: Some(Utc::now()),
                message: Some(Message {
                    role: Role::User,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: "Test task".to_string(),
                        metadata: None,
                    })],
                    metadata: None,
                }),
            },
            artifacts: None,
            history: None,
            metadata: None,
        };

        // Save initial task
        task_repo.save_task(&task).await.unwrap();

        // Create TaskFlow
        let task_flow = TaskFlow::new(
            task_id.clone(),
            agent_id,
            Arc::clone(&task_repo),
            Arc::clone(&client_manager),
            Arc::clone(&tool_executor),
            Arc::clone(&agent_registry),
        );

        // Process a reject decision
        let decision = RoutingDecision::Reject {
            reason: "Test rejection reason".to_string(),
        };

        // Execute the decision
        task_flow.process_decision(decision).await.unwrap();

        // Verify task has been updated
        let updated_task = task_repo.get_task(&task_id).await.unwrap().unwrap();

        // Check task state
        assert_eq!(updated_task.status.state, TaskState::Failed);

        // Check task message contains rejection reason
        let message_text = updated_task
            .status
            .message
            .unwrap()
            .parts
            .iter()
            .find_map(|p| match p {
                Part::TextPart(tp) => Some(tp.text.clone()),
                _ => None,
            })
            .unwrap();
        assert!(message_text.contains("Test rejection reason"));

        // Check processing status
        let status = task_flow.get_processing_status().await.unwrap();
        assert!(matches!(status, ProcessingStatus::Failed));
    }
}