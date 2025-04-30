//! Combines results from multiple delegated or decomposed tasks.

#![cfg(feature = "bidir-delegate")]

use crate::types::{Task, Artifact, Message, Part, TextPart, Role, TaskState}; // Import TaskState
use crate::server::repositories::task_repository::TaskRepository;
use crate::bidirectional_agent::error::AgentError;
use std::sync::Arc;
use anyhow::Result;

/// Synthesizes results from completed child tasks into a parent task.
pub struct ResultSynthesizer {
    parent_task_id: String,
    task_repository: Arc<dyn TaskRepository>,
    // Add configuration or context if needed
}

impl ResultSynthesizer {
    pub fn new(parent_task_id: String, task_repository: Arc<dyn TaskRepository>) -> Self {
        Self { parent_task_id, task_repository }
    }

    /// Performs the synthesis process.
    /// Fetches child tasks, combines their artifacts/messages, and updates the parent.
    pub async fn synthesize(&self) -> Result<(), AgentError> {
        println!("🔄 Synthesizing results for parent task '{}'...", self.parent_task_id);

        // --- 1. Fetch Child Tasks (Requires TaskRepositoryExt) ---
        // Placeholder: Assume we have a way to get child task IDs
        let child_task_ids = self.get_child_task_ids().await?;
        if child_task_ids.is_empty() {
            println!("  ⚠️ No child tasks found for parent '{}'. Synthesis skipped.", self.parent_task_id);
            return Ok(());
        }
        println!("  Found child tasks: {:?}", child_task_ids);

        // --- 2. Collect Results from Completed Children ---
        let mut combined_artifacts: Vec<Artifact> = Vec::new();
        let mut combined_messages: Vec<String> = Vec::new();
        let mut all_children_completed = true;

        for child_id in &child_task_ids {
            match self.task_repository.get_task(child_id).await? {
                Some(child_task) => {
                    match child_task.status.state {
                        TaskState::Completed => {
                            println!("  ✅ Child task '{}' completed.", child_id);
                            // Collect artifacts
                            if let Some(artifacts) = child_task.artifacts {
                                combined_artifacts.extend(artifacts);
                            }
                            // Collect final message text
                            if let Some(msg) = child_task.status.message {
                                if let Some(Part::TextPart(tp)) = msg.parts.first() {
                                    combined_messages.push(format!("Result from {}: {}", child_id, tp.text));
                                }
                            }
                        }
                        TaskState::Failed | TaskState::Canceled => {
                             println!("  ❌ Child task '{}' failed or was canceled.", child_id);
                             all_children_completed = false;
                             combined_messages.push(format!("Task {} failed or was canceled.", child_id));
                             // Decide if parent should fail if any child fails
                             // For now, we'll let it complete but include failure info.
                        }
                        _ => {
                            println!("  ⏳ Child task '{}' is still in progress ({:?}).", child_id, child_task.status.state);
                            all_children_completed = false;
                            // Parent task cannot be completed yet.
                            // In a real system, this synthesis might be triggered only when all children are final.
                            return Ok(()); // Exit synthesis for now
                        }
                    }
                }
                None => {
                    println!("  ⚠️ Child task '{}' not found.", child_id);
                    all_children_completed = false; // Treat missing child as incomplete/failed
                }
            }
        }

        // --- 3. Update Parent Task ---
        let mut parent_task = self.task_repository.get_task(&self.parent_task_id).await?
            .ok_or_else(|| AgentError::SynthesisError(format!("Parent task '{}' not found.", self.parent_task_id)))?;

        // Combine artifacts (simple concatenation for now)
        parent_task.artifacts = Some(combined_artifacts);

        // Create synthesized message
        let final_state = if all_children_completed { TaskState::Completed } else { TaskState::Failed }; // Or maybe a 'PartiallyCompleted' state
        let synthesis_text = if all_children_completed {
            format!("All subtasks completed. Combined results:\n{}", combined_messages.join("\n"))
        } else {
             format!("Some subtasks failed or were canceled. Partial results:\n{}", combined_messages.join("\n"))
        };

        parent_task.status = crate::types::TaskStatus {
            state: final_state,
            timestamp: Some(chrono::Utc::now()),
            message: Some(Message {
                role: Role::Agent,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: synthesis_text,
                    metadata: None,
                })],
                metadata: None,
            }),
        };

        // --- 4. Save Updated Parent Task ---
        self.task_repository.save_task(&parent_task).await?;
        self.task_repository.save_state_history(&parent_task.id, &parent_task).await?;

        println!("✅ Synthesis complete for parent task '{}'. Final state: {:?}", self.parent_task_id, final_state);
        Ok(())
    }

     /// Placeholder for fetching child task IDs. Requires TaskRepositoryExt.
     async fn get_child_task_ids(&self) -> Result<Vec<String>, AgentError> {
         // In a real implementation, this would use TaskRepositoryExt::get_related_tasks
         println!("⚠️ Placeholder: Fetching child task IDs for '{}'", self.parent_task_id);
         // Example:
         // let relationships = self.task_repository.get_related_tasks(&self.parent_task_id).await?;
         // Ok(relationships.children)
         Ok(vec![]) // Return empty for now
     }
}
