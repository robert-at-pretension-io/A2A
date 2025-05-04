//! REPL command handlers for the Bidirectional Agent.

use anyhow::{anyhow, Result};
use serde_json::{/* json, */ Value}; // Removed json
// use std::io; // Unused
use tracing::{debug, error, info, instrument, /* trace, */ warn}; // Removed trace
// use uuid::Uuid; // Unused

use crate::{
    bidirectional::bidirectional_agent::BidirectionalAgent,
    client::A2aClient,
    types::{
        /* AgentCard, AgentCapabilities, */ Part, Role, /* Task, */ TaskIdParams, TaskQueryParams, TaskState, // Removed unused
        /* TextPart, */ // Unused
    },
};

/// Prints the REPL help message to the console.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub(super) fn print_repl_help(agent: &BidirectionalAgent) {
    // Note: agent isn't strictly needed here, but kept for consistency
    debug!("Printing REPL help message.");
    println!("\n========================================");
    println!("⚡ Bidirectional A2A Agent REPL Commands ⚡");
    println!("========================================");
    println!("  :help            - Show this help message");
    println!("  :card            - Show agent card");
    println!("  :servers         - List known remote servers");
    println!("  :connect URL     - Connect to a remote agent at URL");
    println!("  :connect HOST:PORT - Connect to a remote agent by host and port");
    println!("  :connect N       - Connect to Nth server in the server list");
    println!("  :disconnect      - Disconnect from current remote agent");
    println!("  :remote MSG      - Send message as task to connected agent");
    println!("  :listen PORT     - Start listening server on specified port");
    println!("  :stop            - Stop the currently running server");
    println!("  :session new     - Create a new conversation session");
    println!("  :session show    - Show the current session ID");
    println!("  :history         - Show message history for current session");
    println!("  :tasks           - List all tasks in the current session");
    println!("  :task ID         - Show details for a specific task");
    println!("  :artifacts ID    - Show artifacts for a specific task");
    println!("  :cancelTask ID   - Cancel a running task");
    println!("  :file PATH MSG   - Send message with a file attachment (Not Implemented)"); // Keep note
    println!("  :data JSON MSG   - Send message with JSON data (Not Implemented)"); // Keep note
    println!("  :tool NAME [PARAMS] - Execute local tool with optional JSON parameters");
    println!("  :quit            - Exit the REPL");
    println!("========================================\n");
}

/// Handles the ':connect' REPL command logic.
#[instrument(skip(agent, target), fields(agent_id = %agent.agent_id))]
pub(super) async fn handle_connect(agent: &mut BidirectionalAgent, target: &str) -> Result<String> {
    debug!(target = %target, "Handling connect command.");

    // Check if it's a number (referring to a server in the list)
    if let Ok(server_idx) = target.parse::<usize>() {
        // Read from shared known_servers map
        let mut server_list: Vec<(String, String)> = agent
            .known_servers
            .iter()
            .map(|entry| (entry.value().clone(), entry.key().clone())) // (Name, URL)
            .collect();
        server_list.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by name to match display order

        if server_idx > 0 && server_idx <= server_list.len() {
            let (name, url) = &server_list[server_idx - 1];
            let url_clone = url.clone(); // Clone for async block

            // Create a new client with the selected URL
            agent.client = Some(A2aClient::new(&url_clone));
            info!(server_name = %name, server_url = %url_clone, "Connecting to known server by number.");
            let connect_msg = format!("🔗 Connecting to {}: {}", name, url_clone);

            // Attempt to get card after connecting (spawn to avoid blocking REPL)
            let mut agent_client = agent.client.as_mut().unwrap().clone(); // Clone client for task
            let agent_registry_clone = agent.agent_registry.clone(); // Clone registry for update
            let known_servers_clone = agent.known_servers.clone(); // Clone known_servers
            let agent_id_clone = agent.agent_id.clone(); // For tracing span

            tokio::spawn(async move {
                let span = tracing::info_span!(
                    "connect_get_card",
                    agent_id = %agent_id_clone,
                    remote_url = %url_clone
                );
                let _enter = span.enter();
                match agent_client.get_agent_card().await {
                    Ok(card) => {
                        let remote_agent_name = card.name.clone();
                        debug!(%remote_agent_name, "Successfully got card after connecting.");
                        // Update known servers
                        known_servers_clone
                            .insert(url_clone.clone(), remote_agent_name.clone());
                        // Update canonical registry using discover
                        match agent_registry_clone.discover(&url_clone).await {
                            Ok(discovered_agent_id) => {
                                debug!(url = %url_clone, %discovered_agent_id, "Successfully updated canonical registry after connecting by number.");
                                if discovered_agent_id != remote_agent_name {
                                    debug!(url = %url_clone, old_name = %remote_agent_name, new_id = %discovered_agent_id, "Updating known_servers with discovered ID.");
                                    known_servers_clone
                                        .insert(url_clone.clone(), discovered_agent_id);
                                }
                            }
                            Err(e) => {
                                warn!(error = %e, url = %url_clone, "Failed to update canonical registry after connecting by number.");
                            }
                        }
                        println!("📇 Remote agent verified: {}", remote_agent_name); // Still print verification
                    }
                    Err(e) => {
                        warn!(error = %e, "Connected, but failed to get card.");
                        println!("⚠️ Could not retrieve agent card after connecting: {}", e); // Still print warning
                    }
                }
            });
            Ok(connect_msg) // Return the initial connection message
        } else {
            error!(index = %server_idx, "Invalid server number provided.");
            Err(anyhow!(
                "Invalid server number. Use :servers to see available servers."
            ))
        }
    } else {
        // Treat as URL - Extract URL from potentially noisy args string
        debug!(args = %target, "Attempting to extract URL from connect arguments.");
        // Find the first word that looks like a URL
        let extracted_url = target
            .split_whitespace()
            .find(|s| s.starts_with("http://") || s.starts_with("https://"))
            .map(|s| {
                s.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/')
                    .to_string()
            }); // Basic cleanup

        if extracted_url.is_none() {
            error!(
                "Could not extract a valid URL from connect arguments: '{}'",
                target
            );
            return Err(anyhow!(
                "No valid URL found in arguments. Use 'connect URL' or 'connect N'."
            ));
        }

        let url_to_connect = extracted_url.unwrap();
        if url_to_connect.is_empty() {
            error!("Extracted URL is empty from arguments: '{}'", target);
            return Err(anyhow!(
                "Extracted URL is empty. Use 'connect URL' or 'connect N'."
            ));
        }
        debug!(url = %url_to_connect, "Extracted URL. Attempting connection.");

        let mut client = A2aClient::new(&url_to_connect); // Use the extracted URL

        match client.get_agent_card().await {
            Ok(card) => {
                let remote_agent_name = card.name.clone();
                info!(remote_agent_name = %remote_agent_name, url = %url_to_connect, "Successfully connected to agent.");
                let success_msg =
                    format!("✅ Successfully connected to agent: {}", remote_agent_name);

                agent
                    .known_servers
                    .insert(url_to_connect.clone(), remote_agent_name.clone());
                agent.client = Some(client);

                // Update registry in background
                let registry_clone = agent.agent_registry.clone();
                let url_clone = url_to_connect.clone(); // Use url_to_connect
                let known_servers_clone = agent.known_servers.clone();
                let remote_agent_name_clone = remote_agent_name.clone();
                tokio::spawn(async move {
                    match registry_clone.discover(&url_clone).await {
                        Ok(discovered_agent_id) => {
                            debug!(url = %url_clone, %discovered_agent_id, "Successfully updated canonical registry after connecting by URL.");
                            if discovered_agent_id != remote_agent_name_clone {
                                debug!(url = %url_clone, old_name = %remote_agent_name_clone, new_id = %discovered_agent_id, "Updating known_servers with discovered ID.");
                                known_servers_clone
                                    .insert(url_clone.clone(), discovered_agent_id);
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, url = %url_clone, "Failed to update canonical registry after connecting by URL.");
                        }
                    }
                });
                Ok(success_msg)
            }
            Err(e) => {
                error!(url = %url_to_connect, error = %e, "Failed to connect to agent via URL.");
                // Don't ask y/n here, just return error
                if !agent.known_servers.contains_key(&url_to_connect) {
                    let unknown_name = "Unknown Agent".to_string();
                    agent
                        .known_servers
                        .insert(url_to_connect.clone(), unknown_name.clone());
                    info!(url = %url_to_connect, "Added URL to known servers as 'Unknown Agent' after failed connection attempt.");
                }
                Err(anyhow!("Failed to connect to agent at {}: {}. Please check the server is running and the URL is correct. URL added to known servers as 'Unknown Agent'.", url_to_connect, e))
            }
        }
    }
}

/// Handles the ':disconnect' REPL command logic.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub(super) fn handle_disconnect(agent: &mut BidirectionalAgent) -> Result<String> {
    debug!("Handling disconnect command.");
    if agent.client.is_some() {
        let url = agent.client_url().unwrap_or_else(|| "unknown".to_string());
        agent.client = None;
        info!(disconnected_from = %url, "Disconnected from remote agent.");
        Ok(format!("🔌 Disconnected from {}", url))
    } else {
        debug!("Attempted disconnect but not connected.");
        Ok("⚠️ Not connected to any server".to_string()) // Return as Ok string
    }
}

/// Handles the ':servers' REPL command logic.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub(super) fn handle_list_servers(agent: &BidirectionalAgent) -> Result<String> {
    debug!("Handling list_servers command.");
    if agent.known_servers.is_empty() {
        info!("No known servers found.");
        Ok(
            "📡 No known servers. Connect to a server or wait for background connection attempt."
                .to_string(),
        )
    } else {
        info!(count = %agent.known_servers.len(), "Formatting known servers list.");
        let mut output = String::from("\n📡 Known Servers:\n");
        let mut server_list: Vec<(String, String)> = agent
            .known_servers
            .iter()
            .map(|entry| (entry.value().clone(), entry.key().clone())) // (Name, URL)
            .collect();
        server_list.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by name

        for (i, (name, url)) in server_list.iter().enumerate() {
            let marker = if Some(url) == agent.client_url().as_ref() {
                "*"
            } else {
                " "
            };
            output.push_str(&format!("  {}{}: {} - {}\n", marker, i + 1, name, url));
        }
        output.push_str("\nUse :connect N to connect to a server by number\n");
        Ok(output)
    }
}

/// Handles the ':remote' REPL command logic.
#[instrument(skip(agent, message), fields(agent_id = %agent.agent_id))]
pub(super) async fn handle_remote_message(
    agent: &mut BidirectionalAgent,
    message: &str,
) -> Result<String> {
    debug!(message = %message, "Handling remote message command.");
    if message.is_empty() {
        error!("No message provided for :remote command.");
        return Err(anyhow!("No message provided. Use :remote MESSAGE"));
    }
    if agent.client.is_none() {
        error!("Cannot send remote task: Not connected to a remote agent.");
        return Err(anyhow!(
            "Not connected to a remote agent. Use :connect URL first."
        ));
    }

    info!(message = %message, "Sending task to remote agent.");
    let task = agent.send_task_to_remote(message).await?; // send_task_to_remote logs internally now
    info!(task_id = %task.id, status = ?task.status.state, "Remote task sent successfully.");

    let mut response = format!(
        "✅ Task sent successfully!\n   Task ID: {}\n   Initial state reported by remote: {:?}",
        task.id, task.status.state
    );

    if task.status.state == TaskState::Completed {
        let task_response = agent.extract_text_from_task(&task);
        if task_response != "No response text available." {
            debug!(task_id = %task.id, "Appending immediate response from completed remote task.");
            response.push_str("\n\n📥 Response from remote agent:\n");
            response.push_str(&task_response);
        }
    }
    Ok(response)
}

/// Handles the ':session new' REPL command logic.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub(super) fn handle_new_session(agent: &mut BidirectionalAgent) -> Result<String> {
    let session_id = agent.create_new_session(); // create_new_session logs internally now
    info!("Created new session: {}", session_id);
    Ok(format!("✅ Created new session: {}", session_id))
}

/// Handles the ':session show' REPL command logic.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub(super) fn handle_show_session(agent: &BidirectionalAgent) -> Result<String> {
    if let Some(session_id) = &agent.current_session_id {
        debug!(session_id = %session_id, "Displaying current session ID.");
        Ok(format!("🔍 Current session: {}", session_id))
    } else {
        info!("No active session.");
        Ok("⚠️ No active session. Use :session new to create one.".to_string())
    }
}

/// Handles the ':history' REPL command logic.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub(super) async fn handle_history(agent: &BidirectionalAgent) -> Result<String> {
    if let Some(session_id) = &agent.current_session_id {
        debug!(session_id = %session_id, "Fetching history for current session.");
        let tasks = agent.get_current_session_tasks().await?; // get_current_session_tasks logs internally
        if tasks.is_empty() {
            info!(session_id = %session_id, "No tasks found in current session history.");
            Ok("📭 No messages in current session.".to_string())
        } else {
            info!(session_id = %session_id, task_count = %tasks.len(), "Formatting session history.");
            let mut output = format!("\n📝 Session History ({} Tasks):\n", tasks.len());
            for task in tasks.iter() {
                output.push_str(&format!(
                    "--- Task ID: {} (Status: {:?}) ---\n",
                    task.id, task.status.state
                ));
                if let Some(history) = &task.history {
                    for message in history {
                        let role_icon = match message.role {
                            Role::User => "👤",
                            Role::Agent => "🤖",
                            // _ => "➡️", // Role is enum, no need for wildcard
                        };
                        let text = message
                            .parts
                            .iter()
                            .filter_map(|p| match p {
                                Part::TextPart(tp) => Some(tp.text.as_str()),
                                Part::FilePart(_) => Some("[File Part]"),
                                Part::DataPart(_) => Some("[Data Part]"),
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        let display_text = if text.len() > 100 {
                            format!("{}...", &text[..97])
                        } else {
                            text.to_string()
                        };
                        output.push_str(&format!(
                            "  {} {}: {}\n",
                            role_icon, message.role, display_text
                        ));
                    }
                } else {
                    output.push_str("  (No message history available for this task)\n");
                }
                output.push_str("--------------------------------------\n");
            }
            Ok(output)
        }
    } else {
        info!("Cannot show history: No active session.");
        Err(anyhow!(
            "No active session. Use :session new to create one."
        ))
    }
}

/// Handles the ':tasks' REPL command logic.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub(super) async fn handle_list_tasks(agent: &BidirectionalAgent) -> Result<String> {
    if let Some(session_id) = &agent.current_session_id {
        debug!(session_id = %session_id, "Fetching task list for current session.");
        if let Some(task_ids) = agent.session_tasks.get(session_id) {
            if task_ids.is_empty() {
                info!(session_id = %session_id, "No tasks found in current session map.");
                Ok("📭 No tasks recorded in current session.".to_string())
            } else {
                info!(session_id = %session_id, task_count = %task_ids.len(), "Formatting task list.");
                let mut output =
                    format!("\n📋 Tasks Recorded in Current Session ({}):\n", task_ids.len());
                for (i, task_id) in task_ids.iter().enumerate() {
                    let params = TaskQueryParams {
                        id: task_id.clone(),
                        history_length: Some(0),
                        metadata: None,
                    };
                    let status_str = match agent.task_service.get_task(params).await {
                        Ok(task) => format!("{:?}", task.status.state),
                        Err(_) => "Status Unknown".to_string(),
                    };
                    output.push_str(&format!(
                        "  {}. {} - Status: {}\n",
                        i + 1,
                        task_id,
                        status_str
                    ));
                }
                Ok(output)
            }
        } else {
            info!(session_id = %session_id, "Session ID not found in task map.");
            Ok("📭 No tasks recorded for current session (session ID not found).".to_string())
        }
    } else {
        info!("Cannot list tasks: No active session.");
        Err(anyhow!(
            "No active session. Use :session new to create one."
        ))
    }
}

/// Handles the ':task ID' REPL command logic.
#[instrument(skip(agent, task_id), fields(agent_id = %agent.agent_id))]
pub(super) async fn handle_show_task(agent: &BidirectionalAgent, task_id: &str) -> Result<String> {
    debug!(task_id = %task_id, "Handling show task command.");
    if task_id.is_empty() {
        error!("No task ID provided for :task command.");
        return Err(anyhow!("No task ID provided. Use :task TASK_ID"));
    }

    info!(task_id = %task_id, "Fetching details for task.");
    let params = TaskQueryParams {
        id: task_id.to_string(),
        history_length: None,
        metadata: None,
    };
    match agent.task_service.get_task(params).await {
        Ok(task) => {
            debug!(task_id = %task.id, "Formatting details for task.");
            let mut output = String::from("\n🔍 Task Details:\n");
            output.push_str(&format!("  ID: {}\n", task.id));
            output.push_str(&format!("  Status: {:?}\n", task.status.state));
            output.push_str(&format!(
                "  Session: {}\n",
                task.session_id.as_deref().unwrap_or("None")
            ));
            output.push_str(&format!(
                "  Timestamp: {}\n",
                task.status
                    .timestamp
                    .map(|t| t.to_rfc3339())
                    .as_deref()
                    .unwrap_or("None")
            ));
            let artifact_count = task.artifacts.as_ref().map_or(0, |a| a.len());
            output.push_str(&format!(
                "  Artifacts: {} (use :artifacts {} to view)\n",
                artifact_count, task.id
            ));

            if let Some(message) = &task.status.message {
                let text = message
                    .parts
                    .iter()
                    .filter_map(|p| match p {
                        Part::TextPart(tp) => Some(tp.text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                output.push_str(&format!("\n  Status Message: {}\n", text));
            }
            if let Some(history) = &task.history {
                output.push_str(&format!("\n  History Preview ({} messages):\n", history.len()));
                for msg in history.iter().take(5) {
                    let role_icon = match msg.role {
                        Role::User => "👤",
                        Role::Agent => "🤖",
                        // _ => "➡️",
                    };
                    let text = msg
                        .parts
                        .iter()
                        .filter_map(|p| match p {
                            Part::TextPart(tp) => Some(tp.text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    let display_text = if text.len() > 60 {
                        format!("{}...", &text[..57])
                    } else {
                        text.to_string()
                    };
                    output.push_str(&format!(
                        "    {} {}: {}\n",
                        role_icon, msg.role, display_text
                    ));
                }
                if history.len() > 5 {
                    output.push_str("    ...\n");
                }
            } else {
                output.push_str("\n  History: Not available or requested.\n");
            }
            Ok(output)
        }
        Err(e) => {
            error!(task_id = %task_id, error = %e, "Failed to get task details.");
            Err(anyhow!("Failed to get task: {}", e))
        }
    }
}

/// Handles the ':artifacts ID' REPL command logic.
#[instrument(skip(agent, task_id), fields(agent_id = %agent.agent_id))]
pub(super) async fn handle_show_artifacts(
    agent: &BidirectionalAgent,
    task_id: &str,
) -> Result<String> {
    debug!(task_id = %task_id, "Handling show artifacts command.");
    if task_id.is_empty() {
        error!("No task ID provided for :artifacts command.");
        return Err(anyhow!("No task ID provided. Use :artifacts TASK_ID"));
    }

    info!(task_id = %task_id, "Fetching artifacts for task.");
    let params = TaskQueryParams {
        id: task_id.to_string(),
        history_length: Some(0),
        metadata: None,
    };
    match agent.task_service.get_task(params).await {
        Ok(task) => {
            if let Some(artifacts) = &task.artifacts {
                if artifacts.is_empty() {
                    info!(task_id = %task.id, "No artifacts found for task.");
                    Ok(format!("📦 No artifacts for task {}", task.id))
                } else {
                    info!(task_id = %task.id, count = %artifacts.len(), "Formatting artifacts.");
                    let mut output = format!(
                        "\n📦 Artifacts for Task {} ({}):\n",
                        task.id,
                        artifacts.len()
                    );
                    for (i, artifact) in artifacts.iter().enumerate() {
                        output.push_str(&format!(
                            "  {}. Name: '{}', Desc: '{}', Index: {}, Parts: {}\n",
                            i + 1,
                            artifact.name.as_deref().unwrap_or("N/A"),
                            artifact.description.as_deref().unwrap_or("N/A"),
                            artifact.index,
                            artifact.parts.len()
                        ));
                    }
                    Ok(output)
                }
            } else {
                info!(task_id = %task.id, "Task found, but has no artifacts field.");
                Ok(format!("📦 No artifacts found for task {}", task.id))
            }
        }
        Err(e) => {
            error!(task_id = %task_id, error = %e, "Failed to get task for artifacts.");
            Err(anyhow!("Failed to get task: {}", e))
        }
    }
}

/// Handles the ':cancelTask ID' REPL command logic.
#[instrument(skip(agent, task_id), fields(agent_id = %agent.agent_id))]
pub(super) async fn handle_cancel_task(
    agent: &BidirectionalAgent,
    task_id: &str,
) -> Result<String> {
    debug!(task_id = %task_id, "Handling cancel task command.");
    if task_id.is_empty() {
        error!("No task ID provided for :cancelTask command.");
        return Err(anyhow!("No task ID provided. Use :cancelTask TASK_ID"));
    }

    info!(task_id = %task_id, "Attempting to cancel task.");
    let params = TaskIdParams {
        id: task_id.to_string(),
        metadata: None,
    };
    match agent.task_service.cancel_task(params).await {
        Ok(task) => {
            info!(task_id = %task.id, status = ?task.status.state, "Successfully canceled task.");
            Ok(format!(
                "✅ Successfully canceled task {}\n   Current state: {:?}",
                task.id, task.status.state
            ))
        }
        Err(e) => {
            error!(task_id = %task_id, error = %e, "Failed to cancel task.");
            Err(anyhow!("Failed to cancel task: {}", e))
        }
    }
}

/// Handles the ':tool NAME [PARAMS]' REPL command logic.
#[instrument(skip(agent, tool_name, params_str), fields(agent_id = %agent.agent_id))]
pub(super) async fn handle_tool_command(
    agent: &BidirectionalAgent,
    tool_name: &str,
    params_str: &str,
) -> Result<String> {
    debug!(tool_name = %tool_name, params_str = %params_str, "Handling tool command.");
    if tool_name.is_empty() {
        error!("No tool name provided for :tool command.");
        return Err(anyhow!(
            "No tool name provided. Use :tool TOOL_NAME [JSON_PARAMS]"
        ));
    }

    let params_json: Value = match serde_json::from_str(params_str) {
        Ok(json) => json,
        Err(e) => {
            error!(error = %e, input_params = %params_str, "Failed to parse JSON parameters for :tool command.");
            return Err(anyhow!("Invalid JSON parameters: {}", e));
        }
    };

    if let Some(executor) = &agent.task_service.tool_executor {
        match executor.execute_tool(tool_name, params_json).await {
            Ok(result) => {
                info!(tool_name = %tool_name, "Tool executed successfully via REPL.");
                let result_pretty =
                    serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
                Ok(format!(
                    "\n🛠️ Tool Result ({}):\n{}\n",
                    tool_name, result_pretty
                ))
            }
            Err(e) => {
                error!(tool_name = %tool_name, error = %e, "Error executing tool via REPL.");
                Err(anyhow!("Error executing tool '{}': {}", tool_name, e))
            }
        }
    } else {
        error!("ToolExecutor not available in TaskService for :tool command.");
        Err(anyhow!("Tool execution is not configured for this agent."))
    }
}

/// Handles the ':card' REPL command logic.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub(super) fn handle_show_card(agent: &BidirectionalAgent) -> Result<String> {
    debug!("Handling show card command.");
    let card = agent.create_agent_card(); // Logs internally
    info!(agent_name = %card.name, "Formatting agent card.");
    let mut output = String::from("\n📇 Agent Card:\n");
    output.push_str(&format!("  Name: {}\n", card.name));
    output.push_str(&format!(
        "  Description: {}\n",
        card.description.as_deref().unwrap_or("None")
    ));
    output.push_str(&format!("  URL: {}\n", card.url));
    output.push_str(&format!("  Version: {}\n", card.version));
    output.push_str("  Capabilities:\n");
    output.push_str(&format!(
        "    - Streaming: {}\n",
        card.capabilities.streaming
    ));
    output.push_str(&format!(
        "    - Push Notifications: {}\n",
        card.capabilities.push_notifications
    ));
    output.push_str(&format!(
        "    - State Transition History: {}\n",
        card.capabilities.state_transition_history
    ));
    output.push_str(&format!(
        "  Input Modes: {}\n",
        card.default_input_modes.join(", ")
    ));
    output.push_str(&format!(
        "  Output Modes: {}\n",
        card.default_output_modes.join(", ")
    ));
    output.push_str("\n");
    Ok(output)
}

/// Handles the ':help' REPL command logic.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub(super) fn handle_help(agent: &BidirectionalAgent) -> Result<String> {
    debug!("Handling help command.");
    // Reuse the print_repl_help logic, but capture output to string
    // This is a bit tricky, maybe just return a static help string?
    Ok(String::from(
        "\n========================================\n\
             ⚡ Bidirectional A2A Agent REPL Commands ⚡\n\
             ========================================\n\
               :help            - Show this help message\n\
               :card            - Show agent card\n\
               :servers         - List known remote servers\n\
               :connect URL     - Connect to a remote agent at URL\n\
               :connect HOST:PORT - Connect to a remote agent by host and port\n\
               :connect N       - Connect to Nth server in the server list\n\
               :disconnect      - Disconnect from current remote agent\n\
               :remote MSG      - Send message as task to connected agent\n\
               :listen PORT     - Start listening server on specified port\n\
               :stop            - Stop the currently running server\n\
               :session new     - Create a new conversation session\n\
               :session show    - Show the current session ID\n\
               :history         - Show message history for current session\n\
               :tasks           - List all tasks in the current session\n\
               :task ID         - Show details for a specific task\n\
               :artifacts ID    - Show artifacts for a specific task\n\
               :cancelTask ID   - Cancel a running task\n\
               :file PATH MSG   - Send message with a file attachment (Not Implemented)\n\
               :data JSON MSG   - Send message with JSON data (Not Implemented)\n\
               :tool NAME [PARAMS] - Execute local tool with optional JSON parameters\n\
               :quit            - Exit the REPL\n\
             ========================================\n\
             You can also type many commands without the ':' prefix (e.g., 'connect URL', 'list servers').\n",
    ))
}

/// Central handler for REPL commands (called by run_repl and process_message_locally)
/// Needs mutable access to self for state changes.
/// server_running and server_shutdown_token need to be passed in/out or managed differently.
/// For now, :listen and :stop are omitted from this refactoring.
#[instrument(skip(agent, command, args), fields(agent_id = %agent.agent_id))]
pub(super) async fn handle_repl_command(
    agent: &mut BidirectionalAgent,
    command: &str,
    args: &str,
) -> Result<String> {
    debug!(%command, %args, "Handling REPL command via central handler.");
    match command {
        "help" => handle_help(agent),
        "card" => handle_show_card(agent),
        "servers" => handle_list_servers(agent),
        "connect" => handle_connect(agent, args).await,
        "disconnect" => handle_disconnect(agent),
        "remote" => handle_remote_message(agent, args).await,
        "session" => match args {
            "new" => handle_new_session(agent),
            "show" => handle_show_session(agent),
            _ => Err(anyhow!(
                "Unknown session command. Use ':session new' or ':session show'"
            )),
        },
        "history" => handle_history(agent).await,
        "tasks" => handle_list_tasks(agent).await,
        "task" => handle_show_task(agent, args).await,
        "artifacts" => handle_show_artifacts(agent, args).await,
        "cancelTask" => handle_cancel_task(agent, args).await,
        "tool" => {
            let (tool_name, params_str) = match args.split_once(' ') {
                Some((name, params)) => (name, params),
                None => (args, "{}"), // Assume only tool name if no space
            };
            handle_tool_command(agent, tool_name, params_str).await
        }
        // :listen and :stop are NOT handled here due to state access issues
        "listen" | "stop" => {
            warn!(":listen and :stop commands must be handled directly in run_repl due to server state management.");
            Err(anyhow!(
                "Command '{}' cannot be handled by this central handler.",
                command
            ))
        }
        _ => {
            warn!(%command, "Attempted to handle unknown command centrally.");
            Err(anyhow!("Unknown command: '{}'", command))
        }
    }
}
