//! REPL command handlers for the Bidirectional Agent.

use anyhow::{anyhow, Result};
use serde_json::{/* json, */ Value}; // Removed json
                                     // use std::io; // Unused
use tracing::{debug, error, info, instrument, /* trace, */ warn}; // Removed trace
                                                                  // use uuid::Uuid; // Unused

use crate::{
    bidirectional::{agent_helpers, bidirectional_agent::BidirectionalAgent}, // Add agent_helpers
    client::A2aClient,
    types::{
        /* AgentCard, AgentCapabilities, */ Part,
        Role,
        /* Task, */ TaskIdParams,
        TaskQueryParams,
        TaskState, // Removed unused
                   /* TextPart, */ // Unused
    },
};

/// Prints the REPL help message to the console.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub fn print_repl_help(agent: &BidirectionalAgent) { // Made pub for run_repl
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
    println!("  :memory          - Show the agent's rolling memory of outgoing requests");
    println!("  :memory clear    - Clear the agent's rolling memory");
    println!("  :memoryTask ID   - Show details for a specific task in rolling memory");
    println!("  :file PATH MSG   - Send message with a file attachment (Not Implemented)"); // Keep note
    println!("  :data JSON MSG   - Send message with JSON data (Not Implemented)"); // Keep note
    println!("  :tool NAME [PARAMS] - Execute local tool with optional JSON parameters");
    println!("  :quit            - Exit the REPL");
    println!("========================================\n");
}

// Removed handle_connect, handle_disconnect, handle_list_servers

/// Handles the ':remote' REPL command logic.
#[instrument(skip(agent, message), fields(agent_id = %agent.agent_id))]
pub(super) async fn handle_remote_message( // Keep this as it's specific to REPL's remote interaction
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
        let task_response = agent_helpers::extract_text_from_task(agent, &task); // Call helper
        if task_response != "No response text available." {
            debug!(task_id = %task.id, "Appending immediate response from completed remote task.");
            response.push_str("\n\n📥 Response from remote agent:\n");
            response.push_str(&task_response);
        }
    }
    Ok(response)
}

// Removed handle_new_session, handle_show_session

/// Handles the ':session show' REPL command logic.
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub(super) fn handle_show_session(agent: &BidirectionalAgent) -> Result<String> { // Keep for now, or make it a tool
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
        let tasks = agent_helpers::get_current_session_tasks(agent).await?; // Call helper
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
                let mut output = format!(
                    "\n📋 Tasks Recorded in Current Session ({}):\n",
                    task_ids.len()
                );
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
                output.push_str(&format!(
                    "\n  History Preview ({} messages):\n",
                    history.len()
                ));
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

    // Extract the JSON part (in case user includes text before/after JSON)
    let extracted_json = crate::bidirectional::task_router::extract_json_from_text(params_str);
    let params_json: Value = match serde_json::from_str(&extracted_json) {
        Ok(json) => json,
        Err(e) => {
            error!(error = %e, input_params = %params_str, extracted_json = %extracted_json, 
                  "Failed to parse JSON parameters for :tool command.");
            return Err(anyhow!(
                "Invalid JSON parameters: {}. Make sure to provide valid JSON.",
                e
            ));
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

// Removed handle_show_card

/// Handles the ':memory' REPL command logic.
#[instrument(skip(agent, args), fields(agent_id = %agent.agent_id))]
pub fn handle_memory(agent: &mut BidirectionalAgent, args: &str) -> Result<String> { // Keep this, it's REPL specific state
    debug!(args = %args, "Handling memory command.");

    // Check if this is a clear command
    if args.trim() == "clear" {
        debug!("Clearing rolling memory.");
        agent.rolling_memory.clear();
        return Ok("🧠 Rolling memory cleared.".to_string());
    }

    // Get the tasks from rolling memory in chronological order
    let tasks = agent.rolling_memory.get_tasks_chronological();

    if tasks.is_empty() {
        debug!("Rolling memory is empty.");
        return Ok(
            "🧠 Rolling memory is empty. No outgoing requests have been stored.".to_string(),
        );
    }

    // Format the output
    let mut output = format!(
        "\n🧠 Rolling Memory ({} outgoing requests):\n\n",
        tasks.len()
    );

    for (i, task) in tasks.iter().enumerate() {
        // Extract text from the first user message (original request)
        let request_text = task
            .history
            .as_ref()
            .and_then(|history| {
                history
                    .iter()
                    .find(|msg| msg.role == Role::User)
                    .map(|msg| {
                        msg.parts
                            .iter()
                            .filter_map(|part| match part {
                                Part::TextPart(tp) => Some(tp.text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
            })
            .unwrap_or_else(|| "No request text available".to_string());

        // Get a short preview of the request
        let short_request = if request_text.len() > 70 {
            format!("{}...", &request_text[..67])
        } else {
            request_text
        };

        // Extract status from the task
        let status = &task.status.state;

        // Add to output
        output.push_str(&format!(
            "{}. Task ID: {} (Status: {:?})\n",
            i + 1,
            task.id,
            status
        ));
        output.push_str(&format!("   Request: {}\n", short_request));

        // Add a separator between entries
        if i < tasks.len() - 1 {
            output.push('\n');
        }
    }

    output.push_str("\nUse :memoryTask ID to view details of a specific task\n");
    Ok(output)
}

/// Handles the ':memoryTask ID' REPL command logic.
#[instrument(skip(agent, task_id), fields(agent_id = %agent.agent_id))]
pub fn handle_memory_task(agent: &BidirectionalAgent, task_id: &str) -> Result<String> {
    debug!(task_id = %task_id, "Handling memory task command.");

    if task_id.is_empty() {
        error!("No task ID provided for :memoryTask command.");
        return Err(anyhow!("No task ID provided. Use :memoryTask TASK_ID"));
    }

    // Get the task from rolling memory
    match agent.rolling_memory.get_task(task_id) {
        Some(task) => {
            debug!(task_id = %task.id, "Found task in rolling memory.");

            // Extract original request text
            let request_text = task
                .history
                .as_ref()
                .and_then(|history| {
                    history
                        .iter()
                        .find(|msg| msg.role == Role::User)
                        .map(|msg| {
                            msg.parts
                                .iter()
                                .filter_map(|part| match part {
                                    Part::TextPart(tp) => Some(tp.text.as_str()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                })
                .unwrap_or_else(|| "No request text available".to_string());

            // Extract response text
            let response_text =
                crate::bidirectional::agent_helpers::extract_text_from_task(agent, &task);

            // Format the output
            let mut output = format!("\n🧠 Memory Task Details for ID: {}\n\n", task_id);
            output.push_str(&format!("Status: {:?}\n", task.status.state));

            if let Some(timestamp) = &task.status.timestamp {
                output.push_str(&format!("Timestamp: {}\n", timestamp));
            }

            output.push_str("\n📤 Original Request:\n");
            output.push_str(&format!("{}\n", request_text));

            output.push_str("\n📥 Response:\n");
            output.push_str(&format!("{}\n", response_text));

            Ok(output)
        }
        None => {
            error!(task_id = %task_id, "Task not found in rolling memory.");
            Err(anyhow!("Task ID {} not found in rolling memory", task_id))
        }
    }
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
               :memory          - Show the agent's rolling memory of outgoing requests\n\
               :memory clear    - Clear the agent's rolling memory\n\
               :memoryTask ID   - Show details for a specific task in rolling memory\n\
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
pub async fn handle_repl_command(
    agent: &mut BidirectionalAgent,
    command: &str,
    args: &str,
) -> Result<String> {
    debug!(%command, %args, "Handling REPL command via central handler.");
    // Most commands will now be rephrased and sent to process_message_locally
    // Only truly REPL-specific commands or those needing direct REPL state remain.
    match command {
        "help" => handle_help(agent), // Stays as it's REPL specific UI
        "remote" => handle_remote_message(agent, args).await, // Stays as it directly uses agent.client
        "session" if args == "show" => handle_show_session(agent), // Read-only, could be a tool but simple enough here
        // "session new" will become "create new session" and go through process_message_locally
        // "card" will become "show my agent card" -> tool
        // "servers" will become "list known servers" -> tool
        // "connect", "disconnect" will become natural language -> agent action

        // These remain as they interact with TaskService directly for now
        "history" => handle_history(agent).await,
        "tasks" => handle_list_tasks(agent).await,
        "task" => handle_show_task(agent, args).await,
        "artifacts" => handle_show_artifacts(agent, args).await,
        "cancelTask" => handle_cancel_task(agent, args).await,
        "memory" => handle_memory(agent, args),
        "memoryTask" => handle_memory_task(agent, args),
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
