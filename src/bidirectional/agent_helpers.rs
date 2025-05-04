//! Helper functions for the Bidirectional Agent, related to session management and utilities.

use anyhow::Result;
use dashmap::DashMap;
use std::sync::Arc;
use tracing::{debug, info, instrument, trace, warn};
use uuid::Uuid;

use super::bidirectional_agent::BidirectionalAgent;
use crate::{
    server::ServerError, // Import ServerError for error mapping in get_current_session_tasks
    types::{Part, Role, Task, TaskQueryParams, TaskState},
};

/// Create a new session ID and store it in the agent's state.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub fn create_new_session(agent: &mut BidirectionalAgent) -> String {
    let session_id = format!("session-{}", Uuid::new_v4());
    info!(new_session_id = %session_id, "Creating new session."); // Keep info for new session
    agent.current_session_id = Some(session_id.clone());
    trace!("Inserting new session ID into session_tasks map.");
    agent.session_tasks.insert(session_id.clone(), Vec::new());
    session_id
}

/// Add task ID to the current session's task list.
#[instrument(skip(agent, task), fields(agent_id = %agent.agent_id, task_id = %task.id))]
pub async fn save_task_to_history(agent: &BidirectionalAgent, task: Task) -> Result<(), ServerError> {
    debug!("Attempting to save task to session history.");
    if let Some(session_id) = &agent.current_session_id {
        trace!(%session_id, "Current session ID found.");
        if let Some(mut tasks) = agent.session_tasks.get_mut(session_id) {
            debug!(session_id = %session_id, "Adding task to session history."); // Changed to debug
            tasks.push(task.id.clone());
            trace!(task_count = tasks.len(), "Task added to session map.");
        } else {
            // This case should ideally not happen if ensure_session is called correctly
            warn!(session_id = %session_id, "Session not found in map while trying to save task.");
        }
    } else {
        // This might happen if a task completes outside of an active REPL session context
        warn!("No active session while trying to save task to history.");
    }
    Ok(())
}

/// Get full Task objects for all tasks in the current session.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub async fn get_current_session_tasks(agent: &BidirectionalAgent) -> Result<Vec<Task>, ServerError> {
    let mut tasks = Vec::new();
    if let Some(session_id) = &agent.current_session_id {
        debug!(session_id = %session_id, "Fetching full tasks for current session."); // Changed to debug
        if let Some(task_ids) = agent.session_tasks.get(session_id) {
            debug!(session_id = %session_id, count = %task_ids.len(), "Found task IDs in session map."); // Changed to debug
            trace!(?task_ids, "Task IDs to fetch.");
            for task_id in task_ids.iter() {
                debug!(task_id = %task_id, "Fetching details for task.");
                // Use TaskQueryParams to get task with history
                let params = TaskQueryParams {
                    id: task_id.clone(),
                    history_length: None, // Get full history
                    metadata: None,
                };
                trace!(?params, "Params for fetching task details.");

                match agent.task_service.get_task(params).await {
                    Ok(task) => {
                        trace!(task_id = %task.id, "Successfully fetched task details.");
                        tasks.push(task);
                    }
                    Err(e) => {
                        error!(task_id = %task_id, session_id = %session_id, error = %e, "Failed to get task details for session history.");
                        // Continue trying to get other tasks, but log the error
                    }
                }
            }
            debug!(fetched_count = tasks.len(), "Finished fetching tasks for session.");
        } else {
            warn!(session_id = %session_id, "Session ID not found in task map while fetching tasks.");
        }
        Ok(tasks) // Return the collected tasks
    } else {
        warn!("Cannot get session tasks: No active session.");
        // Return empty vec if no session
        Ok(Vec::new())
    }
}

/// Helper to extract text from task artifacts, status message, or history.
#[instrument(skip(agent, task), fields(task_id = %task.id))]
pub fn extract_text_from_task(agent: &BidirectionalAgent, task: &Task) -> String {
    // Note: agent parameter is not strictly needed here but kept for consistency
    // with the pattern of moving helpers.
    debug!("Extracting text response from task.");
    trace!(?task, "Task details for text extraction.");

    // First check if we have artifacts that contain results
    trace!("Checking task artifacts for text.");
    if let Some(ref artifacts) = task.artifacts {
        // Get the latest artifact
        if let Some(latest_artifact) = artifacts.iter().last() {
            trace!(artifact_index = latest_artifact.index, "Found latest artifact.");
            // Extract text from the artifact
            let text = latest_artifact
                .parts
                .iter()
                .filter_map(|p| match p {
                    Part::TextPart(tp) => {
                        trace!("Found TextPart in artifact.");
                        Some(tp.text.clone())
                    }
                    _ => {
                        trace!("Non-TextPart found in artifact, skipping.");
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");

            if !text.is_empty() {
                trace!(text_len = text.len(), "Extracted text from artifact.");
                // Extract proper content from the text
                // If it's in quotes or just a JSON string, clean it up
                let cleaned_text = text.trim_matches('"');
                if cleaned_text.starts_with('{') && cleaned_text.ends_with('}') {
                    trace!("Text looks like JSON, attempting to pretty-print.");
                    // Try to parse and pretty-format the JSON
                    if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(cleaned_text)
                    {
                        let pretty_json = serde_json::to_string_pretty(&json_value)
                            .unwrap_or(cleaned_text.to_string());
                        debug!("Returning pretty-printed JSON from artifact.");
                        return pretty_json;
                    } else {
                        trace!("Failed to parse as JSON, returning cleaned text.");
                    }
                }
                debug!("Returning cleaned text from artifact.");
                return cleaned_text.to_string();
            } else {
                trace!("No text found in latest artifact parts.");
            }
        } else {
            trace!("Artifacts list is empty.");
        }
    } else {
        trace!("Task has no artifacts field.");
    }

    // If no artifacts, check the status message
    trace!("Checking task status message for text.");
    if let Some(ref message) = task.status.message {
        trace!("Found status message.");
        let text = message
            .parts
            .iter()
            .filter_map(|p| match p {
                Part::TextPart(tp) => {
                    trace!("Found TextPart in status message.");
                    Some(tp.text.clone())
                }
                _ => {
                    trace!("Non-TextPart found in status message, skipping.");
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        if !text.is_empty() {
            trace!(text_len = text.len(), "Extracted text from status message.");
            debug!("Returning text from status message.");
            return text;
        } else {
            trace!("No text found in status message parts.");
        }
    } else {
        trace!("Task has no status message.");
    }

    // Then check history if available
    trace!("Checking task history for agent messages.");
    if let Some(history) = &task.history {
        trace!(history_len = history.len(), "Found history.");
        let agent_messages = history
            .iter()
            .filter(|m| m.role == Role::Agent)
            .flat_map(|m| {
                trace!(message_role = ?m.role, "Processing message in history.");
                m.parts.iter()
            })
            .filter_map(|p| match p {
                Part::TextPart(tp) => {
                    trace!("Found TextPart in history agent message.");
                    Some(tp.text.clone())
                }
                _ => {
                    trace!("Non-TextPart found in history agent message, skipping.");
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        if !agent_messages.is_empty() {
            trace!(text_len = agent_messages.len(), "Extracted text from history agent messages.");
            debug!("Returning text from history agent messages.");
            return agent_messages;
        } else {
            trace!("No text found in history agent messages.");
        }
    } else {
        trace!("Task has no history field.");
    }

    // Fallback
    warn!("No response text found in artifacts, status message, or history.");
    "No response text available.".to_string() // Add fallback return
}

/// Get the URL of the currently configured client from the agent's config
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub fn client_url(agent: &BidirectionalAgent) -> Option<String> {
    // Use the target_url stored in the agent's own configuration
    let url = agent.client_config.target_url.clone();
    trace!(?url, "Retrieved client URL from config.");
    url
}
