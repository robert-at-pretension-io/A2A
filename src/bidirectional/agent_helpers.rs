//! Helper functions for the Bidirectional Agent, related to session management and utilities.

use anyhow::{anyhow, Result};
use dashmap::DashMap;
use serde_json;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, trace, warn};
use uuid::Uuid;

use super::bidirectional_agent::{BidirectionalAgent, AGENT_NAME, AGENT_VERSION};
use crate::{
    server::ServerError, // Import ServerError for error mapping in get_current_session_tasks
    server,
    types::{Part, Role, Task, TaskQueryParams, TaskState, AgentCard, AgentCapabilities, AgentSkill},
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

/// Run the agent server
#[instrument(skip(agent), fields(agent_id = %agent.agent_id, port = %agent.port, bind_address = %agent.bind_address))]
pub async fn run_server(agent: &BidirectionalAgent) -> Result<()> {
    info!("Starting agent server run sequence."); // Keep info for server start sequence
    // Create a cancellation token for graceful shutdown
    debug!("Creating server shutdown token.");
    let shutdown_token = CancellationToken::new();
    let shutdown_token_clone = shutdown_token.clone();

    // Set up signal handlers for graceful shutdown
    debug!("Setting up signal handlers (Ctrl+C).");
    let agent_id_clone = agent.agent_id.clone(); // Clone for the signal handler task
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to install CTRL+C handler");
        // Use tracing within the signal handler task
        let span = tracing::info_span!("signal_handler", agent_id = %agent_id_clone);
        let _enter = span.enter();
        info!("Received Ctrl+C shutdown signal."); // Keep info for signal
        println!("\n🛑 Received shutdown signal, stopping server..."); // Added emoji
        shutdown_token_clone.cancel();
        debug!("Cancellation token cancelled by signal handler.");
    });

    // Create the agent card for this server
    debug!("Creating agent card for server endpoint.");
    let agent_card = create_agent_card(agent); // Logs internally
    debug!(agent_name = %agent_card.name, "Generated agent card for server."); // Changed to debug

    // Convert to JSON value for the server
    trace!("Serializing agent card to JSON for server.");
    let agent_card_json = match serde_json::to_value(&agent_card) {
         Ok(json) => {
             trace!(?json, "Agent card serialized successfully.");
             json
         }
         Err(e) => {
             error!(error = %e, "Failed to serialize agent card to JSON. Using empty object.");
             serde_json::json!({})
         }
    };

    info!(bind_address = %agent.bind_address, port = %agent.port, "Attempting to start server via run_server.");
    // Start the server - handle the Box<dyn Error> specially
    let server_handle = match server::run_server(
        agent.port,
        &agent.bind_address,
        agent.task_service.clone(),
        agent.streaming_service.clone(),
        agent.notification_service.clone(),
        shutdown_token.clone(),
        Some(agent_card_json), // Pass our custom agent card
    ).await {
        Ok(handle) => {
            info!(bind_address = %agent.bind_address, port = %agent.port, "Server started successfully."); // Keep info for server start success
            handle
        },
        Err(e) => {
            error!(bind_address = %agent.bind_address, port = %agent.port, error = %e, "Failed to start server.");
            // Return an error that can be handled by the caller (e.g., main)
            return Err(anyhow!("Failed to start server: {}", e));
        }
    };

    println!("🔌 Server running on http://{}:{}", agent.bind_address, agent.port); // Added emoji

    // Wait for the server to complete or be cancelled
    debug!("Waiting for server task to complete or be cancelled."); // Changed to debug
    match server_handle.await {
        Ok(()) => {
            info!("Server shut down gracefully."); // Keep info for graceful shutdown
            Ok(())
        }
        Err(join_err) => {
            error!(error = %join_err, "Server join error.");
            Err(anyhow!("Server join error: {}", join_err))
        }
    }
}

/// Create an agent card for this agent instance.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub fn create_agent_card(agent: &BidirectionalAgent) -> AgentCard {
    debug!(agent_id = %agent.agent_id, "Creating agent card for this instance.");
    // Construct AgentCapabilities based on actual capabilities
    // TODO: Make these capabilities configurable or dynamically determined
    trace!("Determining agent capabilities.");
    let capabilities = AgentCapabilities {
        push_notifications: true, // Example: Assuming supported
        state_transition_history: true, // Example: Assuming supported
        streaming: false, // Set to false to match the test expectations
        // Add other fields from AgentCapabilities if they exist
    };
    trace!(?capabilities, "Agent capabilities determined.");

    // Convert tools to agent skills
    let mut skills = Vec::new();
    
    // Get tools from the task service's tool executor if available
    // self.task_service is an Arc<TaskService>, not an Option
    let task_service = &agent.task_service;
    if let Some(tool_executor) = &task_service.tool_executor {
        for (tool_name, tool) in tool_executor.tools.iter() {
            // Create an AgentSkill for each tool
            let skill = AgentSkill {
                id: tool_name.clone(),
                name: tool_name.clone(),
                description: Some(tool.description().to_string()),
                examples: None,
                input_modes: Some(vec!["text".to_string()]),
                output_modes: Some(vec!["text".to_string()]),
                tags: Some(tool.capabilities().iter().map(|&s| s.to_string()).collect()),
            };
            skills.push(skill);
        }
    } else {
        debug!("No tool executor found in task service, skills list will be empty");
    }
    
    trace!(skills_count = skills.len(), "Skills for agent card");

    let card = AgentCard {
        // id field does not exist on AgentCard in types.rs
        name: agent.agent_name.clone(), // Use the configured agent name (falls back to agent_id if None)
        description: Some("A bidirectional A2A agent that can process tasks and delegate to other agents".to_string()),
        version: AGENT_VERSION.to_string(), // version is String
        url: format!("http://{}:{}", agent.bind_address, agent.port), // url is String
        capabilities, // Use the capabilities struct
        authentication: None, // Set authentication if needed, otherwise None
        default_input_modes: vec!["text".to_string()], // Example
        default_output_modes: vec!["text".to_string()], // Example
        documentation_url: None,
        provider: None,
        skills, // Use the skills we collected from tools
        // Add other fields from AgentCard if they exist
    };
    trace!(?card, "Agent card created.");
    card // Return the created card
}

/// Send a task to a remote agent using the A2A client, ensuring session consistency.
#[instrument(skip(agent, message), fields(agent_id = %agent.agent_id, message_len = message.len()))]
pub async fn send_task_to_remote(agent: &mut BidirectionalAgent, message: &str) -> Result<Task> {
    debug!("Attempting to send task to remote agent."); // Changed to debug
    trace!(message = %message, "Message content for remote task.");
    // ① Ensure a session exists locally
    debug!("Ensuring local session exists.");
    ensure_session(agent).await; // Call helper function
    let session_id = agent.current_session_id.clone(); // Clone session_id for sending
    trace!(?session_id, "Using session ID for remote task.");

    let remote_url = client_url(agent).unwrap_or_else(|| "unknown".to_string());
    info!(remote_url = %remote_url, session_id = ?session_id, "Attempting to send task to remote agent."); // Keep info for sending attempt

    // Get mutable reference to the client
    debug!("Getting A2A client instance.");
    let client = match agent.client.as_mut() {
         Some(c) => c,
         None => {
             error!("No remote client configured. Use :connect first.");
             return Err(anyhow!("No remote client configured. Use :connect first."));
         }
    };
    trace!("A2A client obtained.");

    // ② Send the task with the current session ID
    debug!("Calling client.send_task.");
    let task_result = client.send_task(message, session_id.clone()).await; // Pass cloned session_id
    trace!(?task_result, "Result from client.send_task.");

    let task = match task_result {
        Ok(t) => {
            info!(remote_url = %remote_url, task_id = %t.id, status = ?t.status.state, "Successfully sent task and received response."); // Keep info for success
            trace!(?t, "Received task object from remote.");
            t
        }
        Err(e) => {
            error!(remote_url = %remote_url, error = %e, "Error sending task to remote agent.");
            return Err(anyhow!("Error sending task to {}: {}", remote_url, e));
        }
    };

    // ③ Mirror the returned task locally (read-only copy)
    let task_id_clone = task.id.clone(); // Clone ID for logging messages
    debug!(task_id = %task_id_clone, "Importing remote task locally for tracking.");
    match agent.task_service.import_task(task.clone()).await {
        Ok(()) => {
            debug!(task_id = %task_id_clone, "Cached remote task locally."); // Changed to debug
        }
        Err(e) => {
            // Log warning if caching fails, but don't fail the whole operation
            warn!(task_id = %task_id_clone, error = %e, "Could not cache remote task locally.");
        }
    }

    // ④ Add the task (local mirror) to the current session history
    debug!(task_id = %task.id, "Saving imported remote task to session history.");
    save_task_to_history(agent, task.clone()).await?; // Call helper

    debug!(task_id = %task.id, session_id = ?session_id, "Remote task processing initiated and linked to session."); // Changed to debug
    Ok(task) // Return the original task received from the remote agent
}

/// Get capabilities of a remote agent (using A2A client)
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub async fn get_remote_agent_card(agent: &mut BidirectionalAgent) -> Result<AgentCard> {
    let remote_url = client_url(agent).unwrap_or_else(|| "unknown".to_string());
    info!(remote_url = %remote_url, "Attempting to get remote agent card."); // Keep info for attempt

    // Check if we have a client configured
    debug!("Checking for A2A client configuration.");
    if let Some(client) = &mut agent.client {
        trace!("A2A client found, calling get_agent_card.");
        match client.get_agent_card().await {
             Ok(agent_card) => {
                info!(remote_agent_name = %agent_card.name, remote_url = %remote_url, "Retrieved remote agent card successfully."); // Keep info for success
                trace!(?agent_card, "Retrieved agent card details.");
                Ok(agent_card)
             }
             Err(e) => {
                error!(remote_url = %remote_url, error = %e, "Error retrieving remote agent card.");
                Err(anyhow!("Error retrieving agent card from {}: {}", remote_url, e))
             }
        }
    } else {
        error!("Cannot get remote agent card: No A2A client configured.");
        Err(anyhow!("No A2A client configured. Use --target-url or :connect first."))
    }
}

/// Create a session on demand so remote tasks can be grouped
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub async fn ensure_session(agent: &mut BidirectionalAgent) {
    trace!("Ensuring session exists.");
    if agent.current_session_id.is_none() {
        debug!("No current session ID found. Creating one.");
        let session_id = create_new_session(agent); // Call helper
        debug!(session_id = %session_id, "Created new session on demand."); // Changed to debug
    } else {
        trace!(session_id = %agent.current_session_id.as_deref().unwrap_or("None"), "Session already exists.");
    }
}
