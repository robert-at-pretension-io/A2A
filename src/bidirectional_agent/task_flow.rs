//! Manages the overall lifecycle and flow of tasks, including delegation.

#![cfg(feature = "bidir-delegate")]

use crate::bidirectional_agent::{
    agent_registry::AgentRegistry,
    client_manager::ClientManager,
    config::BidirectionalAgentConfig,
    error::AgentError,
    task_router::{RoutingDecision, TaskRouter},
    tool_executor::ToolExecutor,
    types::{TaskOrigin, TaskRelationships}, // Import necessary types
};
use crate::server::repositories::task_repository::TaskRepository; // Use the base trait
use crate::types::{Task, TaskSendParams, TaskState, TaskStatus, Message, Role, Part, TextPart};
use std::sync::Arc;
use anyhow::Result;
use tokio::time::{sleep, Duration};

/// Manages the execution flow of a single task.
pub struct TaskFlow {
    task_id: String,
    agent_id: String, // ID of the agent handling this flow
    task_repository: Arc<dyn TaskRepository>,
    client_manager: Arc<ClientManager>,
    tool_executor: Arc<ToolExecutor>,
    agent_registry: Arc<AgentRegistry>,
    // Add other necessary components like config, router if needed directly
}

impl TaskFlow {
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
        }
    }

    /// Starts processing the task based on the routing decision.
    /// This is typically called after the initial routing.
    pub async fn process_decision(&self, decision: RoutingDecision) -> Result<(), AgentError> {
        println!("🚀 Starting task flow for '{}' with decision: {:?}", self.task_id, decision);

        match decision {
            RoutingDecision::Local { tool_names: _ } => {
                self.execute_locally().await?;
            }
            RoutingDecision::Remote { agent_id } => {
                self.delegate_task(&agent_id).await?;
            }
            RoutingDecision::Reject { reason } => {
                self.reject_task(&reason).await?;
            }
            // RoutingDecision::Decompose { subtasks } => {
            //     // Handle decomposition (more complex, maybe later refinement)
            //     println!("⏳ Decomposition not fully implemented yet for task '{}'", self.task_id);
            //     self.reject_task("Decomposition not yet supported").await?;
            // }
        }
        Ok(())
    }

    /// Executes the task locally using the ToolExecutor.
    async fn execute_locally(&self) -> Result<(), AgentError> {
        println!("⚙️ Executing task '{}' locally...", self.task_id);
        let mut task = self.get_task().await?;

        // Mark task origin as Local
        self.set_task_origin(TaskOrigin::Local).await?;

        // Execute using the tool executor
        match self.tool_executor.execute_task_locally(&mut task).await {
            Ok(_) => {
                println!("✅ Local execution successful for task '{}'", self.task_id);
                // Save final state (already done by executor)
                self.task_repository.save_task(&task).await?;
                self.task_repository.save_state_history(&task.id, &task).await?;
                Ok(())
            }
            Err(e) => {
                 println!("❌ Local execution failed for task '{}': {}", self.task_id, e);
                 // Save final state (already done by executor)
                 self.task_repository.save_task(&task).await?;
                 self.task_repository.save_state_history(&task.id, &task).await?;
                 Err(e) // Propagate the error
            }
        }
    }

    /// Delegates the task to a remote agent.
    async fn delegate_task(&self, target_agent_id: &str) -> Result<(), AgentError> {
        println!("✈️ Delegating task '{}' to agent '{}'...", self.task_id, target_agent_id);
        let mut task = self.get_task().await?;

        // --- Prepare TaskSendParams for delegation ---
        // Use the latest message from history as the message to send
        let message_to_send = task.history.as_ref()
            .and_then(|h| h.last())
            .cloned()
            .ok_or_else(|| AgentError::DelegationError("Cannot delegate task with no message history".to_string()))?;

        let delegation_params = TaskSendParams {
            id: task.id.clone(), // Use the same ID for now, remote might change it
            message: message_to_send,
            history_length: None, // Let remote agent decide history length
            metadata: task.metadata.clone(), // Pass along metadata
            push_notification: None, // Don't configure push from here yet
            session_id: task.session_id.clone(),
        };

        // --- Send task via ClientManager ---
        match self.client_manager.send_task(target_agent_id, delegation_params).await {
            Ok(remote_task_status) => {
                println!("  ✅ Task '{}' successfully delegated to '{}'. Remote status: {:?}",
                         self.task_id, target_agent_id, remote_task_status.status.state);

                // Mark task origin as Delegated
                self.set_task_origin(TaskOrigin::Delegated {
                    agent_id: target_agent_id.to_string(),
                    remote_task_id: remote_task_status.id.clone(), // Store remote ID
                }).await?;

                // Update local task status to reflect delegation (e.g., Working/Delegated)
                task.status = TaskStatus {
                    state: TaskState::Working, // Or a new 'Delegated' state
                    timestamp: Some(chrono::Utc::now()),
                    message: Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Task delegated to agent '{}'. Remote Task ID: {}", target_agent_id, remote_task_status.id),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
                self.task_repository.save_task(&task).await?;
                self.task_repository.save_state_history(&task.id, &task).await?;

                // --- Start Polling for Remote Task Status (Simplified) ---
                // In a real system, this would be more robust, potentially using webhooks
                // or a dedicated background poller managed by ClientManager.
                self.poll_delegated_task(target_agent_id, &remote_task_status.id).await?;

                Ok(())
            }
            Err(e) => {
                 println!("  ❌ Delegation failed for task '{}' to agent '{}': {}", self.task_id, target_agent_id, e);
                 // Update local task status to Failed
                 task.status = TaskStatus {
                    state: TaskState::Failed,
                    timestamp: Some(chrono::Utc::now()),
                    message: Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Delegation to agent '{}' failed: {}", target_agent_id, e),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                 };
                 self.task_repository.save_task(&task).await?;
                 self.task_repository.save_state_history(&task.id, &task).await?;
                 Err(AgentError::DelegationError(format!("Failed to delegate task: {}", e)))
            }
        }
    }

    /// Simplified polling logic for a delegated task.
    async fn poll_delegated_task(&self, agent_id: &str, remote_task_id: &str) -> Result<(), AgentError> {
        println!("⏳ Starting polling for remote task '{}' on agent '{}'", remote_task_id, agent_id);
        let max_polls = 10;
        let poll_interval = Duration::from_secs(5);

        for i in 0..max_polls {
            sleep(poll_interval).await;
            println!("  Polling attempt {} for task '{}'...", i + 1, remote_task_id);

            match self.client_manager.get_task_status(agent_id, remote_task_id).await {
                Ok(remote_task) => {
                    println!("  Remote status for task '{}': {:?}", remote_task_id, remote_task.status.state);
                    // Update local task based on remote status
                    let mut local_task = self.get_task().await?;
                    local_task.status = remote_task.status.clone(); // Mirror remote status
                    local_task.artifacts = remote_task.artifacts.clone(); // Mirror artifacts

                    self.task_repository.save_task(&local_task).await?;
                    self.task_repository.save_state_history(&local_task.id, &local_task).await?;

                    // Check if remote task is in a final state
                    if matches!(remote_task.status.state, TaskState::Completed | TaskState::Failed | TaskState::Canceled) {
                        println!("✅ Remote task '{}' reached final state: {:?}", remote_task_id, remote_task.status.state);
                        return Ok(());
                    }
                }
                Err(e) => {
                    println!("  ⚠️ Polling failed for task '{}': {}", remote_task_id, e);
                    // Decide how to handle polling errors (retry, mark local as failed, etc.)
                    // For simplicity, we'll stop polling on error here.
                    // A more robust implementation might retry or have different error handling.
                    let mut local_task = self.get_task().await?;
                    local_task.status = TaskStatus {
                        state: TaskState::Failed,
                        timestamp: Some(chrono::Utc::now()),
                        message: Some(Message {
                            role: Role::Agent,
                            parts: vec![Part::TextPart(TextPart {
                                type_: "text".to_string(),
                                text: format!("Polling remote task '{}' failed: {}", remote_task_id, e),
                                metadata: None,
                            })],
                            metadata: None,
                        }),
                    };
                    self.task_repository.save_task(&local_task).await?;
                    self.task_repository.save_state_history(&local_task.id, &local_task).await?;
                    return Err(AgentError::DelegationError(format!("Polling failed: {}", e)));
                }
            }
        }

        println!("⚠️ Polling timed out for remote task '{}'", remote_task_id);
        // Mark local task as failed due to timeout
        let mut local_task = self.get_task().await?;
        local_task.status = TaskStatus {
            state: TaskState::Failed,
            timestamp: Some(chrono::Utc::now()),
            message: Some(Message {
                role: Role::Agent,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: format!("Polling remote task '{}' timed out after {} attempts.", remote_task_id, max_polls),
                    metadata: None,
                })],
                metadata: None,
            }),
        };
        self.task_repository.save_task(&local_task).await?;
        self.task_repository.save_state_history(&local_task.id, &local_task).await?;
        Err(AgentError::DelegationError("Polling timed out".to_string()))
    }


    /// Marks the task as rejected.
    async fn reject_task(&self, reason: &str) -> Result<(), AgentError> {
        println!("❌ Rejecting task '{}': {}", self.task_id, reason);
        let mut task = self.get_task().await?;
        task.status = TaskStatus {
            state: TaskState::Failed, // Or a specific 'Rejected' state if added
            timestamp: Some(chrono::Utc::now()),
            message: Some(Message {
                role: Role::Agent,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: format!("Task rejected: {}", reason),
                    metadata: None,
                })],
                metadata: None,
            }),
        };
        self.task_repository.save_task(&task).await?;
        self.task_repository.save_state_history(&task.id, &task).await?;
        Ok(())
    }

    /// Helper to get the current task state from the repository.
    async fn get_task(&self) -> Result<Task, AgentError> {
        self.task_repository
            .get_task(&self.task_id)
            .await?
            .ok_or_else(|| AgentError::TaskFlowError(format!("Task '{}' not found during flow processing.", self.task_id)))
    }

     /// Helper to set the task origin (requires TaskRepositoryExt).
     async fn set_task_origin(&self, origin: TaskOrigin) -> Result<(), AgentError> {
         // This requires the TaskRepositoryExt trait to be implemented
         // on the concrete repository type.
         // For now, this is a placeholder.
         println!("ℹ️ Setting task origin for '{}' to {:?}", self.task_id, origin);
         // Example (requires modifying TaskRepositoryExt and InMemoryTaskRepository):
         // self.task_repository.set_task_origin(&self.task_id, origin).await?;
         Ok(())
     }
}
