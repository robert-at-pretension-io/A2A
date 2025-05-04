//! Simplified Bidirectional A2A Agent Implementation
//!
//! This file implements a minimal but complete bidirectional A2A agent
//! that can both serve requests and delegate them to other agents.
//!
//! ## Configuration
//!
//! The agent is configured using a TOML file. A sample configuration file 
//! (bidirectional_agent.toml) is included in the root of the project.
//!
//! ```toml
//! [server]
//! port = 8080
//! bind_address = "0.0.0.0"
//! agent_id = "bidirectional-agent"
//!
//! [client]
//! # Optional URL of remote agent to connect to
//! target_url = "http://localhost:8081"
//!
//! [llm]
//! # API key for Claude (can also use CLAUDE_API_KEY environment variable)
//! # claude_api_key = "your-api-key-here"
//! ```
//!
//! ## Usage Examples
//!
//! Start with no arguments (defaults to REPL mode):
//! ```bash
//! cargo run --quiet -- bidirectional-agent
//! ```
//!
//! Start with server:port to connect to a remote agent:
//! ```bash
//! cargo run --quiet -- bidirectional-agent localhost:8080
//! ```
//!
//! Start with config file path:
//! ```bash
//! cargo run --quiet -- bidirectional-agent config_file.toml
//! ```
//!
//! Process a single message locally (configure in TOML):
//! ```toml
//! [mode]
//! message = "What is the capital of France?"
//! ```
//!
//! Get agent card from a remote A2A agent (configure in TOML):
//! ```toml
//! [mode]
//! get_agent_card = true
//! ```
//!
//! Send a task to a remote A2A agent (configure in TOML):
//! ```toml
//! [mode]
//! remote_task = "Hello from bidirectional agent!"
//! ```
//!
//! Start in interactive REPL mode (configure in TOML):
//! ```toml
//! [mode]
//! repl = true
//! ```
//!
//! In REPL mode, you can:
//! - Type messages to process them directly with the agent
//! - Use `:servers` to list known remote servers
//! - Use `:connect URL` to connect to a remote agent by URL
//! - Use `:connect N` to connect to Nth server in the servers list
//! - Use `:disconnect` to disconnect from the current remote agent
//! - Use `:remote MESSAGE` to send a task to the connected agent
//! - Use `:listen PORT` to start a server on the specified port
//! - Use `:stop` to stop the currently running server
//! - Use `:card` to view the agent card
//! - Use `:help` to see all available commands
//! - Use `:quit` to exit the REPL
//!
//! If no configuration file is provided, the agent starts with sensible defaults
//! and automatically enters REPL mode for interactive use.

// Import from other modules in the crate
use crate::client::{
    A2aClient,
    errors::ClientError,
    streaming::{StreamingResponse, StreamingResponseStream},
};

use crate::server::{
    repositories::task_repository::{TaskRepository, InMemoryTaskRepository},
    services::{
        task_service::TaskService,
        streaming_service::StreamingService,
        notification_service::NotificationService,
    },
    // Use the canonical TaskRouter and ToolExecutor from the server module
    task_router::{LlmTaskRouterTrait, RoutingDecision}, // Import LlmTaskRouterTrait and RoutingDecision
    tool_executor::ToolExecutor, // Import ToolExecutor
    // Use the canonical AgentRegistry and ClientManager
    agent_registry::AgentRegistry,
    client_manager::ClientManager,
    run_server,
    error::ServerError, // Import ServerError for error mapping
};

use crate::types::{
    Task, TaskState, Message, Part, TextPart, Role, AgentCard, AgentCapabilities,
    TaskSendParams, TaskQueryParams, TaskIdParams, PushNotificationConfig,
    DataPart, FilePart, TaskStatus, // Import TaskStatus
};


use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use futures_util::{StreamExt, TryStreamExt};
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value, Map}; // Import Map
use std::{
    sync::Arc, 
    error::Error as StdError, 
    fs, 
    path::Path,
    io::{self, Write, BufRead},
    path::PathBuf, // Import PathBuf
    collections::HashMap, // Import HashMap
}; // Import StdError for error mapping and IO
use tokio::{
    fs::OpenOptions, // Import OpenOptions for async file I/O
    io::AsyncWriteExt, // Import AsyncWriteExt for write_all
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use toml;
use tracing::{debug, error, info, trace, warn, instrument, Level, Instrument}; // Import trace, Instrument trait
use tracing_subscriber::{fmt::{self, format::FmtSpan}, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer, filter}; // Import FmtSpan
use uuid::Uuid;

// REMOVED AgentLogContext struct and impl block

// Constants
const AGENT_NAME: &str = "Bidirectional A2A Agent";
const AGENT_VERSION: &str = "1.0.0";
const DEFAULT_PORT: u16 = 8080;
const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0";
const SYSTEM_PROMPT: &str = r#"
You are an AI agent assistant that helps with tasks. You can:
1. Process tasks directly (for simple questions or tasks you can handle)
2. Delegate tasks to other agents when appropriate
3. Use tools when needed

Always think step-by-step about the best way to handle each request.
"#;

// Helper Types (Keep ExecutionMode, AgentDirectoryEntry, AgentDirectory for local logic)

/// Task execution mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// Process the task locally with the agent
    Local,
    /// Delegate to a remote agent
    Remote { agent_id: String },
}

// REMOVED AgentDirectoryEntry struct definition
// REMOVED AgentDirectory struct definition and impl block


// REMOVED Local AgentRegistry definition
// REMOVED Local ClientManager definition


/// Simple LLM client interface
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn complete(&self, prompt: &str) -> Result<String>;
}

/// Simple implementation of an LLM client that delegates to Claude
struct ClaudeLlmClient {
    api_key: String,
    system_prompt: String,
}

impl ClaudeLlmClient {
    fn new(api_key: String, system_prompt: String) -> Self {
        Self {
            api_key,
            system_prompt,
        }
    }
}

#[async_trait]
impl LlmClient for ClaudeLlmClient {
    #[instrument(skip(self, prompt), fields(prompt_len = prompt.len()))]
    async fn complete(&self, prompt: &str) -> Result<String> {
        debug!("Sending request to Claude LLM.");
        trace!(?prompt, "LLM prompt content.");
        // Create a new client for the Claude API
        let client = reqwest::Client::new();
        // Prepare the request payload
        let payload = json!({
            "model": "claude-3-7-sonnet-20250219",
            "max_tokens": 60000,
            "system": self.system_prompt,
            "messages": [{
                "role": "user",
                "content": prompt
            }]
        });
        trace!(payload = %payload, "Claude API request payload.");

        // Send the request to the Claude API
        debug!("Posting request to Claude API endpoint.");
        let response = client.post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key) // API key is sensitive, not logged
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .context("Failed to send request to Claude API")?;
        debug!(status = %response.status(), "Received response from Claude API.");

        // Check if the request was successful
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            error!(%status, error_body = %error_text, "Claude API request failed.");
            return Err(anyhow!("Claude API error ({}): {}", status, error_text));
        }

        // Parse the response
        trace!("Parsing Claude API JSON response.");
        let response_json: Value = response.json().await
            .context("Failed to parse Claude API response")?;
        trace!(response_json = %response_json, "Parsed Claude API response.");

        // Extract the completion
        trace!("Extracting completion text from response.");
        let completion = response_json["content"][0]["text"].as_str()
            .ok_or_else(|| anyhow!("Failed to extract completion from response"))?;
        debug!(completion_len = completion.len(), "Extracted completion from Claude API.");
        trace!(completion = %completion, "Completion content.");

        Ok(completion.to_string())
    }
}

// REMOVED Local TaskRouter definition

// Define a struct that implements the server's TaskRouterTrait
#[derive(Clone)]
pub struct BidirectionalTaskRouter {
    llm: Arc<dyn LlmClient>,
    agent_registry: Arc<AgentRegistry>, // Use the canonical registry
    enabled_tools: Arc<Vec<String>>,
}

impl BidirectionalTaskRouter {
     pub fn new(
         llm: Arc<dyn LlmClient>,
         agent_registry: Arc<AgentRegistry>, // Use canonical registry
         enabled_tools: Arc<Vec<String>>,
     ) -> Self {
        // Ensure "echo" is always considered enabled internally for fallback, even if not in config
        let mut tools = enabled_tools.as_ref().clone();
        if !tools.contains(&"echo".to_string()) {
            tools.push("echo".to_string());
        }
        Self {
            llm,
            agent_registry, // Store the registry
            enabled_tools: Arc::new(tools),
        }
    }

    // Helper to perform the actual routing logic AND tool selection
    // Now returns RoutingDecision directly
    #[instrument(skip(self, task), fields(task_id = %task.id))]
    pub async fn decide_execution_mode(&self, task: &Task) -> Result<RoutingDecision, ServerError> {
        debug!("Deciding execution mode for task.");
        trace!(?task, "Task details for routing decision.");
        
        // Extract the task content for analysis
        // Use history if available, otherwise maybe the top-level message if applicable
        trace!("Extracting task text from history/message.");
        let task_text = task.history.as_ref().map(|history| {
            history.iter()
                .filter(|m| m.role == Role::User)
                .flat_map(|m| m.parts.iter())
                .filter_map(|p| match p {
                    Part::TextPart(tp) => Some(tp.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n")
        }).unwrap_or_else(|| {
             // If history is None or empty, try getting text from the initial message part if available
             task.history.as_ref()
                 .and_then(|h| h.first()) // Get the first message (assuming it's the initial one)
                 .map(|m| m.parts.iter()
                     .filter_map(|p| match p {
                         Part::TextPart(tp) => Some(tp.text.clone()),
                         _ => None,
                     })
                     .collect::<Vec<_>>()
                     .join("\n")
                 )
                 .unwrap_or_default()
        });
        trace!(task_text = %task_text, "Extracted task text.");


        if task_text.is_empty() {
            // If still empty after checking history and initial message
            warn!("Task text is empty after checking history and message, defaulting to local execution with 'echo' tool.");
            // Use tool_name and add default params
            return Ok(RoutingDecision::Local { tool_name: "echo".to_string(), params: json!({}) });
        }
 
        // Get the list of available agents from the canonical registry
        debug!("Fetching available agents from AgentRegistry for routing prompt.");
        let available_agents = self.agent_registry.list_all_agents(); // Returns Vec<(String, AgentCard)>
        trace!(count = available_agents.len(), "Found available agents in registry.");
        let agent_descriptions = available_agents.iter()
            .map(|(agent_id, card)| { // Use agent_id and card directly from registry list
                trace!(%agent_id, agent_name = %card.name, "Formatting agent description for prompt.");
                // Construct capabilities string manually
                let mut caps = Vec::new();
                if card.capabilities.push_notifications { caps.push("pushNotifications"); }
                if card.capabilities.state_transition_history { caps.push("stateTransitionHistory"); }
                if card.capabilities.streaming { caps.push("streaming"); }
                // Add other capabilities fields if they exist in AgentCapabilities struct

                format!("ID: {}\nName: {}\nDescription: {}\nCapabilities: {}",
                    agent_id, // Use the ID from the registry list
                    card.name.as_str(),
                    card.description.as_deref().unwrap_or(""), // Use deref for Option<String>
                    caps.join(", "))
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        trace!(agent_descriptions = %agent_descriptions, "Formatted agent descriptions for prompt.");

        // --- Add Local Tool Descriptions to Prompt ---
        debug!("Fetching descriptions of locally enabled tools for routing prompt.");
        let local_tool_descriptions = self.enabled_tools.iter()
            .map(|tool_name| {
                let description = match tool_name.as_str() {
                    "llm" => "General purpose LLM request. Good for questions, generation, analysis if no specific tool fits.",
                    "summarize" => "Summarizes the input text.",
                    "list_agents" => "Lists known agents registered with this agent.",
                    "remember_agent" => "Stores information about another agent given its URL.",
                    "echo" => "Simple echo tool for testing.",
                    _ => "A custom tool.",
                };
                format!("- {}: {}", tool_name, description)
            })
            .collect::<Vec<_>>()
            .join("\n");
        trace!(local_tool_descriptions = %local_tool_descriptions, "Formatted local tool descriptions for routing prompt.");
        // --- End Add Local Tool Descriptions ---


        // Build a prompt for the LLM to decide routing
        debug!("Building routing prompt for LLM.");
        let routing_prompt = format!(r#"
You need to decide whether to handle a task locally using your own tools, delegate it to another available agent, or reject it entirely.

YOUR LOCAL TOOLS:
{}

AVAILABLE REMOTE AGENTS:
{}

TASK:
{}

Based on the TASK, YOUR LOCAL TOOLS, and AVAILABLE REMOTE AGENTS, decide the best course of action:
1. Handle it locally if one of YOUR LOCAL TOOLS is suitable (respond with "LOCAL").
   - IMPORTANT: If the task matches an internal command like 'connect', 'disconnect', 'list servers', 'session new', 'card', etc., you MUST choose LOCAL execution so the 'execute_command' tool can handle it. Do NOT reject these internal commands.
2. Delegate to a specific remote agent if it's more appropriate (respond with "REMOTE: [agent-id]"). Choose the most relevant agent if multiple are available.
3. Reject the task ONLY if it's inappropriate, harmful, impossible, OR if it's an internal command that cannot be handled by the 'execute_command' tool (e.g., ':listen', ':stop', ':quit'). Provide a brief explanation for rejection.

Your response should be exactly one of those formats (LOCAL, REMOTE: agent-id, or REJECT: reason), with no additional text.
"#, local_tool_descriptions, agent_descriptions, task_text); // Added local_tool_descriptions
        trace!(routing_prompt = %routing_prompt, "Constructed routing prompt.");

        info!("Requesting routing decision from LLM.");
        // Get the routing decision from the LLM
        let decision_result = self.llm.complete(&routing_prompt).await;
        trace!(?decision_result, "LLM routing decision result received.");

        // Map anyhow::Error from LLM to ServerError::Internal
        let decision = match decision_result {
            Ok(d) => {
                let trimmed_decision = d.trim().to_string();
                info!(llm_response = %trimmed_decision, "LLM routing decision response received.");
                trimmed_decision
            }
            Err(e) => {
                error!(error = %e, "LLM routing decision failed. Falling back to local 'echo'.");
                // Fallback to local echo on LLM error
                // Use tool_name and add default params
                return Ok(RoutingDecision::Local { tool_name: "echo".to_string(), params: json!({}) });
            }
        };
        trace!(decision = %decision, "Raw LLM decision text.");

        // Parse the decision more robustly - check prefixes
        if decision.starts_with("LOCAL") {
            info!("LLM decided LOCAL execution. Proceeding to tool selection and parameter extraction.");
            
            // --- Combined LLM Tool Selection & Parameter Extraction ---
            debug!("Fetching descriptions of locally enabled tools for combined prompt.");
            let local_tool_descriptions = self.enabled_tools.iter()
                .map(|tool_name| {
                    // Provide descriptions and expected parameters for better LLM guidance
                    let description = match tool_name.as_str() {
                        "llm" => "General purpose LLM request. Good for questions, generation, analysis if no specific tool fits. Expects {\"text\": \"...\"}",
                        "summarize" => "Summarizes the input text. Expects {\"text\": \"...\"}",
                        "list_agents" => "Lists known agents. Use only if the task is explicitly about listing agents. Expects {} or {\"format\": \"simple\" | \"detailed\"}",
                        "remember_agent" => "Stores information about another agent given its URL. Expects {\"agent_base_url\": \"http://...\"}",
                        "execute_command" => "Executes internal agent commands (like connect, disconnect, servers, session new, card). Use for requests matching these commands. Expects {\"command\": \"command_name\", \"args\": \"arguments_string\"}", // <-- Add description
                        "echo" => "Simple echo tool for testing. Use only if the task is explicitly to echo. Expects {\"text\": \"...\"}",
                        _ => "A custom tool with potentially unknown parameters.",
                    };
                    format!("- {}: {}", tool_name, description)
                })
                .collect::<Vec<_>>()
                .join("\n");
            trace!(local_tool_descriptions = %local_tool_descriptions, "Formatted local tool descriptions for combined prompt.");

            let tool_param_prompt = format!(
r#"You have decided to handle the following TASK locally:
{}

Based on the TASK and the AVAILABLE LOCAL TOOLS listed below, choose the SINGLE most appropriate tool and extract its required parameters.

AVAILABLE LOCAL TOOLS:
{}

Respond ONLY with a valid JSON object containing the chosen tool's name and its parameters. The JSON object MUST have the following structure:
{{
  "tool_name": "<chosen_tool_name>",
  "params": {{ <parameters_object> }}
}}

CRITICAL: If the original TASK was a request to perform an internal agent action (like connecting, listing servers, managing sessions), you MUST select the 'execute_command' tool and extract the command name and arguments into the 'params' object (e.g., {{"command": "connect", "args": "http://..."}}). Do NOT select the 'llm' tool for these internal commands.

Examples:
- For a task like "remember agent at http://foo.com": {{"tool_name": "remember_agent", "params": {{"agent_base_url": "http://foo.com"}}}}
- For a task like "list known agents simply": {{"tool_name": "list_agents", "params": {{"format": "simple"}}}}
- For a task like "echo hello": {{"tool_name": "echo", "params": {{"text": "hello"}}}}
- For a task like "connect to http://bar.com": {{"tool_name": "execute_command", "params": {{"command": "connect", "args": "http://bar.com"}}}} // <-- Add example
- For a task like "list servers": {{"tool_name": "execute_command", "params": {{"command": "servers", "args": ""}}}} // <-- Add example
- For a task like "start a new session": {{"tool_name": "execute_command", "params": {{"command": "session", "args": "new"}}}} // <-- Add example
- For a general question: {{"tool_name": "llm", "params": {{"text": "original question text..."}}}}
- If no specific parameters are needed for the chosen tool (like list_agents with default format): {{"tool_name": "list_agents", "params": {{}}}}

Ensure the 'params' value is always a JSON object (even if empty: {{}})."#,
                task_text,
                local_tool_descriptions
            );
            trace!(tool_param_prompt = %tool_param_prompt, "Constructed combined tool/param extraction prompt.");

            info!("Asking LLM to choose tool and extract parameters.");
            let tool_param_result = self.llm.complete(&tool_param_prompt).await;

            match tool_param_result {
                Ok(json_str_raw) => {
                    let json_str = json_str_raw.trim();
                    trace!(raw_json = %json_str_raw, trimmed_json = %json_str, "Received tool/param JSON string from LLM.");
                    // Attempt to parse the JSON response
                    match serde_json::from_str::<Value>(json_str) {
                        Ok(json_value) => {
                            if let (Some(tool_name), Some(params)) = (
                                json_value.get("tool_name").and_then(Value::as_str),
                                json_value.get("params").cloned() // Clone the params Value
                            ) {
                                // Validate the chosen tool name
                                if self.enabled_tools.contains(&tool_name.to_string()) {
                                    info!(tool_name = %tool_name, ?params, "Successfully parsed tool and params from LLM response.");
                                    Ok(RoutingDecision::Local { tool_name: tool_name.to_string(), params })
                                } else {
                                    warn!(chosen_tool = %tool_name, enabled_tools = ?self.enabled_tools, "LLM chose an unknown/disabled tool in JSON response. Falling back to 'llm' tool.");
                                    Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": task_text}) })
                                }
                            } else {
                                warn!(json_response = %json_str, "LLM returned valid JSON but missing 'tool_name' or 'params'. Falling back to 'llm' tool.");
                                Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": task_text}) })
                            }
                        },
                        Err(e) => {
                            warn!(error = %e, json_response = %json_str, "LLM returned invalid JSON for tool/params. Falling back to 'llm' tool.");
                            Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": task_text}) })
                        }
                    }
                },
                Err(e) => {
                    warn!(error = %e, "LLM failed to choose tool/extract params. Falling back to 'llm' tool.");
                    Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": task_text}) })
                }
            }
            // --- End Combined LLM Tool Selection & Parameter Extraction ---

        } else if decision.starts_with("REMOTE: ") {
            let agent_id = decision.strip_prefix("REMOTE: ").unwrap().trim().to_string();
            info!(remote_agent_id = %agent_id, "LLM decided REMOTE execution.");

            // Verify the agent exists in the canonical registry
            trace!(remote_agent_id = %agent_id, "Verifying remote agent existence in AgentRegistry.");
            if self.agent_registry.get(&agent_id).is_none() { // Check canonical registry
                 warn!(remote_agent_id = %agent_id, "LLM decided to delegate to unknown agent (not in registry), falling back to local execution with 'llm' tool and original task text.");
                 // Fall back to local if agent not found, using llm tool with original text
                 Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": task_text}) })
            } else {
                 info!(remote_agent_id = %agent_id, "Routing decision: Remote delegation confirmed.");
                 Ok(RoutingDecision::Remote { agent_id })
            }
        } else if decision.starts_with("REJECT: ") {
            let reason = decision.strip_prefix("REJECT: ").unwrap().trim().to_string();
            info!(reason = %reason, "LLM decided to REJECT the task.");
            Ok(RoutingDecision::Reject { reason })
        } else {
            warn!(llm_decision = %decision, "LLM routing decision was unclear, falling back to local execution with 'llm' tool and original task text.");
            // Default to local llm tool if the decision isn't clear, using original text
            Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": task_text}) })
        }
    }
}
 
#[async_trait]
impl LlmTaskRouterTrait for BidirectionalTaskRouter {
    // Match the trait signature: takes TaskSendParams, returns Result<RoutingDecision, ServerError>
    // REMOVED #[instrument(skip(self, params), fields(task_id = %params.id))]
    async fn route_task(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError> {
        debug!("Routing task based on TaskSendParams.");
        trace!(?params, "TaskSendParams for routing.");
        // We need a Task object to make the decision based on history/message.
        // Construct a temporary Task from TaskSendParams.
        trace!("Constructing temporary Task object for routing decision.");
        let task = Task {
            id: params.id.clone(),
            status: TaskStatus { // Default status for routing decision
                state: TaskState::Submitted,
                timestamp: Some(Utc::now()),
                message: None, // Status message is usually set later
            },
            // Use the incoming message as the start of history for decision making
            history: Some(vec![params.message.clone()]),
            artifacts: None,
            metadata: params.metadata.clone(),
            session_id: params.session_id.clone(),
        };
        trace!(?task, "Temporary Task object created.");

        // Call the updated decide_execution_mode which now returns RoutingDecision
        debug!("Delegating routing decision to decide_execution_mode.");
        let decision = self.decide_execution_mode(&task).await;
        debug!(?decision, "Routing decision made.");
        decision
    }
 
    // Add the required process_follow_up method
    // REMOVED #[instrument(skip(self, message), fields(task_id))]
    async fn process_follow_up(&self, task_id: &str, message: &Message) -> Result<RoutingDecision, ServerError> {
        debug!("Processing follow-up message for task.");
        trace!(?message, "Follow-up message details.");
        // For now, always route follow-ups locally using the 'llm' tool as a simple default.
        // A real implementation might involve the LLM again to choose a tool based on history.
        info!("Defaulting follow-up routing to LOCAL execution using 'llm' tool.");
        
        // Extract text from the follow-up message for the 'llm' tool's 'text' parameter
        let follow_up_text = message.parts.iter()
            .filter_map(|p| match p {
                Part::TextPart(tp) => Some(tp.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
            
        Ok(RoutingDecision::Local { 
            tool_name: "llm".to_string(), 
            params: json!({"text": follow_up_text}) 
        })
    }

    // Implement the decide method required by LlmTaskRouterTrait
    // This method is now effectively the same as route_task, as the full decision happens there.
    #[instrument(skip(self, params), fields(task_id = %params.id))]
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError> {
        debug!("Decide method called, delegating to route_task.");
        // Delegate to route_task, which now handles the full decision including tool choice
        self.route_task(params).await
    }

    // Implement should_decompose method required by LlmTaskRouterTrait
    #[instrument(skip(self, params), fields(task_id = %params.id))]
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, ServerError> {
        debug!("Checking if task should be decomposed.");
        trace!(?params, "Task parameters for decomposition check.");
        // Simple implementation that never decomposes tasks
        let should = false;
        debug!(should_decompose = %should, "Decomposition decision.");
        Ok(should)
    }

    // Implement decompose_task method required by LlmTaskRouterTrait
    #[instrument(skip(self, params), fields(task_id = %params.id))]
    async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<crate::server::task_router::SubtaskDefinition>, ServerError> {
        debug!("Decomposing task (currently no-op).");
        trace!(?params, "Task parameters for decomposition.");
        // Simple implementation that returns an empty list (no decomposition)
        let subtasks = Vec::new();
        debug!("Returning empty subtask list.");
        Ok(subtasks)
    }
}


// REMOVED Local ToolExecutor definition

// Define a struct that implements the server's ToolExecutorTrait
// We'll use the provided ToolExecutor from the server module
// instead of implementing our own ToolExecutorTrait


/// Main bidirectional agent implementation
pub struct BidirectionalAgent {
    // Core components (TaskService, StreamingService, NotificationService are needed for run_server)
    task_service: Arc<TaskService>,
    streaming_service: Arc<StreamingService>,
    notification_service: Arc<NotificationService>,

    // Use canonical server components
    agent_registry: Arc<AgentRegistry>, // From crate::server::agent_registry
    client_manager: Arc<ClientManager>, // From crate::server::client_manager

    // Local components for specific logic
    // REMOVED agent_directory field
    // REMOVED agent_directory_path field
    llm: Arc<dyn LlmClient>, // Local LLM client

    // Server configuration
    port: u16,
    bind_address: String,
    agent_id: String, // Keep agent_id for internal identification
    agent_name: String, // Name used for the agent card
    client_config: ClientConfig, // Store client config for URL access

    // A2A client for making outbound requests to other agents
    // This is a simple client that can be expanded later
    pub client: Option<A2aClient>,
    
    // Session management
    current_session_id: Option<String>,
    session_tasks: Arc<DashMap<String, Vec<String>>>, // Map session ID to task IDs

    // REPL logging
    repl_log_file: Option<std::path::PathBuf>,

    // Known servers discovered/connected to (URL -> Name) - Shared for background updates
    known_servers: Arc<DashMap<String, String>>,
}

impl BidirectionalAgent {
    /// Replace the task router with a custom implementation (for testing)
    pub fn with_router(&self, router: Arc<dyn LlmTaskRouterTrait>) {
        info!("Replacing task router with custom implementation");
        // The task_service is not an Option, so we can use it directly
        self.task_service.set_router(router);
    }
    
    /// Access the task service for testing purposes
    pub fn get_task_service(&self) -> Option<Arc<crate::server::services::task_service::TaskService>> {
        // Wrap in Some since we're returning an Option but task_service is not optional
        Some(self.task_service.clone())
    }
    #[instrument(skip(config), fields(config_path = config.config_file_path))]
    pub fn new(config: BidirectionalAgentConfig) -> Result<Self> {
        debug!("Creating new BidirectionalAgent instance."); // Changed to debug
        trace!(?config, "Agent configuration provided.");
        // REMOVED AgentDirectory creation and loading logic

        // Create the LLM client (local helper)
        debug!("Initializing LLM client.");
        let llm: Arc<dyn LlmClient> = if let Some(api_key) = &config.llm.claude_api_key {
            debug!("Using Claude API key from configuration/environment."); // Changed to debug
            Arc::new(ClaudeLlmClient::new(api_key.clone(), config.llm.system_prompt.clone()))
        } else {
            // Allow running without LLM if only acting as server/client without local processing
            warn!("No Claude API key provided. Local LLM processing will not be available.");
            // Provide a dummy LLM client or handle this case appropriately
            // TODO: Implement a dummy LLM client that returns errors or default responses
            error!("No LLM configuration provided. Set CLAUDE_API_KEY environment variable or add claude_api_key to config file.");
            return Err(anyhow!("No LLM configuration provided. Cannot proceed without LLM client.")); // Make it an error
        };
        trace!(system_prompt = %config.llm.system_prompt, "LLM client created.");

        // Create the task repository (needed by services)
        debug!("Initializing InMemoryTaskRepository.");
        let task_repository: Arc<dyn TaskRepository> = Arc::new(InMemoryTaskRepository::new());

        // Create the canonical agent registry (from server module)
        debug!("Initializing AgentRegistry.");
        let agent_registry = Arc::new(AgentRegistry::new()); // Assuming AgentRegistry::new() exists

        // Create the canonical client manager (from server module)
        debug!("Initializing ClientManager.");
        let client_manager = Arc::new(ClientManager::new(agent_registry.clone()));

        // Create our custom task router implementation
        debug!("Initializing BidirectionalTaskRouter.");
        let enabled_tools_list = config.tools.enabled.clone();
        let enabled_tools = Arc::new(enabled_tools_list); // Arc for sharing
        trace!(?enabled_tools, "Enabled tools for router and executor.");

        // Create a tool executor using the new constructor with enabled tools
        debug!("Initializing ToolExecutor.");
        // Create new known_servers map to pass to ToolExecutor
        let known_servers = Arc::new(DashMap::new());

        // Get agent's own info needed for ExecuteCommandTool
        let agent_id_for_tool = config.server.agent_id.clone();
        let agent_name_for_tool = config.server.agent_name.clone().unwrap_or_else(|| agent_id_for_tool.clone());
        let agent_version_for_tool = AGENT_VERSION; // <-- Get agent version from constant
        let bind_address_for_tool = config.server.bind_address.clone();
        let port_for_tool = config.server.port;

        let bidirectional_tool_executor = Arc::new(ToolExecutor::with_enabled_tools(
            &config.tools.enabled,            // Pass slice of enabled tool names
            Some(llm.clone()),                // Pass LLM client (as Option)
            Some(agent_registry.clone()),     // Pass canonical AgentRegistry (as Option)
            Some(known_servers.clone()),      // Pass known_servers map for synchronization
            // Pass agent's own info
            &agent_id_for_tool,
            &agent_name_for_tool,
            agent_version_for_tool, // <-- Pass agent_version
            &bind_address_for_tool,
            port_for_tool,
            // None, // Omit TaskService for now
        ));
        trace!("ToolExecutor created.");

        // Create the task router, passing the enabled tools list and registry
        let bidirectional_task_router: Arc<dyn LlmTaskRouterTrait> =
            Arc::new(BidirectionalTaskRouter::new(
                llm.clone(),
                agent_registry.clone(), // Pass registry instead of directory
                enabled_tools.clone(),
            ));
        trace!("BidirectionalTaskRouter created.");

        // Create the task service using the canonical components and our trait implementations
        debug!("Initializing TaskService.");
        let task_service = Arc::new(TaskService::bidirectional(
            task_repository.clone(),
            bidirectional_task_router, // Pass our LlmTaskRouterTrait implementation
            bidirectional_tool_executor, // Pass the ToolExecutor
            client_manager.clone(),
            agent_registry.clone(),
            config.server.agent_id.clone(),
            Some(llm.clone()), // Pass the LLM client to TaskService
        ));
        debug!("TaskService created."); // Changed to debug

        // Create the streaming service
        debug!("Initializing StreamingService.");
        let streaming_service = Arc::new(StreamingService::new(task_repository.clone()));
        debug!("StreamingService created."); // Changed to debug

        // Create the notification service (pass repository)
        debug!("Initializing NotificationService.");
        let notification_service = Arc::new(NotificationService::new(task_repository.clone()));
        debug!("NotificationService created."); // Changed to debug

        // Initialize an A2A client if target URL is provided
        debug!("Checking for client target URL.");
        let client = if let Some(target_url) = &config.client.target_url {
            debug!(target_url = %target_url, "Initializing A2A client for target URL."); // Changed to debug
            Some(A2aClient::new(target_url))
        } else {
            debug!("No target URL provided, A2A client not initialized."); // Changed to debug
            None
        };
        trace!(client_initialized = client.is_some(), "A2A client status.");

        // Store agent directory path if provided
        let agent_directory_path = config.tools.agent_directory_path.as_ref().map(PathBuf::from);
        trace!(?agent_directory_path, "Agent directory path stored.");

        debug!("Constructing BidirectionalAgent struct.");
        let agent = Self {
            task_service,
            streaming_service,
            notification_service,

            // Store canonical versions needed by the agent itself (if any)
            agent_registry,
            client_manager,

            // Store local helper components
            // REMOVED agent_directory field
            // REMOVED agent_directory_path field
            llm,

            port: config.server.port,
            bind_address: config.server.bind_address.clone(),
            agent_id: config.server.agent_id.clone(),
            // Use configured agent_name, or fall back to agent_id
            agent_name: config.server.agent_name.unwrap_or_else(|| config.server.agent_id.clone()),
            client_config: config.client.clone(), // Store the client config

            // Store the A2A client (if configured)
            client,
            
            // Initialize session management
            current_session_id: None,
            session_tasks: Arc::new(DashMap::new()),

            // Initialize REPL log file path
            repl_log_file: config.mode.repl_log_file.map(std::path::PathBuf::from),

            // Initialize known servers map with the one we already passed to ToolExecutor
            known_servers: known_servers,
        };
        
        // REMOVED periodic agent directory saving logic

        // Logging initialization happens in main() now.
        // We can log that the agent struct itself is constructed.
        debug!(agent_id = %agent.agent_id, bind_address = %agent.bind_address, port = %agent.port, "BidirectionalAgent struct created successfully."); // Changed to debug

        Ok(agent)
    }

    /// Create a session on demand so remote tasks can be grouped
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    async fn ensure_session(&mut self) {
        trace!("Ensuring session exists.");
        if self.current_session_id.is_none() {
            debug!("No current session ID found. Creating one.");
            let session_id = self.create_new_session(); // existing helper logs internally
            debug!(session_id = %session_id, "Created new session on demand."); // Changed to debug
        } else {
            trace!(session_id = %self.current_session_id.as_deref().unwrap_or("None"), "Session already exists.");
        }
    }


    /// Process a message locally (e.g., from REPL input that isn't a command)
    #[instrument(skip(self, message_text), fields(agent_id = %self.agent_id, message_len = message_text.len()))]
    pub async fn process_message_locally(&mut self, message_text: &str) -> Result<String> {
        debug!("Processing message locally.");
        trace!(message = %message_text, "Message content.");

        // --- Command Interception ---
        let trimmed_text = message_text.trim();
        let (command, args) = match trimmed_text.split_once(' ') {
            Some((cmd, arguments)) => (cmd.to_lowercase(), arguments.trim()),
            None => (trimmed_text.to_lowercase(), ""),
        };

        // Check if the input matches a known command keyword
        // Note: :listen and :stop are excluded here as they need direct access to run_repl state
        let is_command = [
            "help", "card", "servers", "connect", "disconnect", "remote",
            "session", "history", "tasks", "task", "artifacts", "cancelTask", "tool"
        ].contains(&command.as_str())
        // Special case for session commands
        || (command == "session" && ["new", "show"].contains(&args));

        if is_command {
            info!(command = %command, args_len = args.len(), "Input matches command keyword. Handling directly.");
            // Call the central command handler
            // We need to pass the *original* arguments string, not the potentially empty one
            let full_args = if command == trimmed_text.to_lowercase() { "" } else { trimmed_text.split_once(' ').map_or("", |(_, a)| a) };
            return self.handle_repl_command(&command, full_args).await;
        }
        // --- End Command Interception ---


        // --- Original Logic (if not intercepted as a command) ---
        debug!("Input does not match command keyword. Proceeding with standard task processing.");

        // Ensure we have an active session before processing
        debug!("Ensuring session exists before processing message.");
        self.ensure_session().await; // Logs internally

        // Check if we're continuing an existing task that is in InputRequired state
        debug!("Checking if continuing an InputRequired task.");
        let mut continue_task_id: Option<String> = None;
        if let Some(session_id) = &self.current_session_id {
            trace!(%session_id, "Current session ID found.");
            if let Some(task_ids) = self.session_tasks.get(session_id) {
                trace!(task_count = task_ids.len(), "Found tasks in session map.");
                // Check the last task in the session
                if let Some(last_task_id) = task_ids.iter().last() {
                    trace!(%last_task_id, "Checking state of last task in session.");
                    // Get the task to check its state
                    let params = TaskQueryParams {
                        id: last_task_id.clone(),
                        history_length: Some(0), // Don't need history to check state
                        metadata: None,
                    };
                    trace!(?params, "Params for getting last task state.");

                    match self.task_service.get_task(params).await {
                        Ok(task) => {
                            trace!(task_id = %task.id, state = ?task.status.state, "Got last task state.");
                            if task.status.state == TaskState::InputRequired {
                                debug!(task_id = %last_task_id, "Found task in InputRequired state. Continuing this task."); // Changed to debug
                                // We should continue this task instead of creating a new one
                                continue_task_id = Some(last_task_id.clone());
                            } else {
                                debug!(task_id = %last_task_id, state = ?task.status.state, "Last task not in InputRequired state.");
                            }
                        }
                        Err(e) => {
                             warn!(task_id = %last_task_id, error = %e, "Failed to get last task to check state.");
                        }
                    }
                } else {
                    debug!(session_id = %session_id, "No previous tasks found in session map.");
                }
            } else {
                // This case should ideally not happen if ensure_session is called correctly
                warn!(%session_id, "Session ID not found in task map while checking for InputRequired task.");
            }
        } else {
            warn!("No active session ID while checking for InputRequired task.");
        }
        trace!(?continue_task_id, "Result of InputRequired check.");

        // Create a unique task ID if not continuing an existing task
        let task_id = if let Some(id) = continue_task_id {
            debug!(task_id = %id, "Continuing existing task."); // Changed to debug
            id
        } else {
            let new_id = Uuid::new_v4().to_string();
            debug!(task_id = %new_id, "Creating new task ID."); // Changed to debug
            new_id
        };

        let task_id_clone = task_id.clone(); // Clone for instrument span
        trace!(%task_id, "Using task ID for processing.");

        // Create the message
        debug!("Creating initial message structure.");
        let initial_message = Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                text: message_text.to_string(),
                metadata: None,
                type_: "text".to_string(),
            })],
            metadata: None,
        };
        trace!(?initial_message, "Initial message created.");

        // Create TaskSendParams
        debug!("Creating TaskSendParams.");
        let params = TaskSendParams {
            id: task_id.clone(),
            message: initial_message,
            session_id: self.current_session_id.clone(),
            metadata: None, // Add metadata if needed from REPL context
            history_length: None, // Request history if needed
            push_notification: None, // Set push config if needed
        };
        trace!(?params, "TaskSendParams created.");

        debug!(task_id = %task_id, "Calling task_service.process_task."); // Changed to debug
        // Use task_service to process the task
        // Instrument the call to task_service
        let task_result = async {
            self.task_service.process_task(params).await
        }.instrument(tracing::info_span!("task_service_process", task_id = %task_id_clone)).await;

        let task = match task_result {
            Ok(t) => {
                debug!(task_id = %t.id, status = ?t.status.state, "Task processing completed by task_service."); // Changed to debug
                t
            }
            Err(e) => {
                error!(task_id = %task_id, error = %e, "Task processing failed.");
                // Return the error to the REPL
                return Err(anyhow!("Task processing failed: {}", e));
            }
        };
        trace!(?task, "Resulting task object after processing.");

        // Save task to history
        debug!(task_id = %task.id, "Saving task to session history.");
        self.save_task_to_history(task.clone()).await?; // save_task_to_history logs internally now

        // Extract response from task
        debug!(task_id = %task.id, "Extracting text response from task.");
        let mut response = self.extract_text_from_task(&task); // Logs internally now
        trace!(response = %response, "Extracted response text.");

        // If the task is in InputRequired state, indicate that in the response
        if task.status.state == TaskState::InputRequired {
            debug!(task_id = %task.id, "Task requires more input. Appending message to response."); // Changed to debug
            response.push_str("\n\n[The agent needs more information. Your next message will continue this task.]");
        }

        debug!(task_id = %task.id, "Local message processing complete. Returning response."); // Changed to debug
        Ok(response)
    }

    /// Prints the REPL help message to the console.
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    fn print_repl_help(&self) {
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
        println!("  :file PATH MSG   - Send message with a file attachment");
        println!("  :data JSON MSG   - Send message with JSON data");
        println!("  :tool NAME [PARAMS] - Execute local tool with optional JSON parameters");
        println!("  :quit            - Exit the REPL");
        println!("========================================\n");
    }

    // Helper to extract text from task
    #[instrument(skip(self, task), fields(task_id = %task.id))]
    fn extract_text_from_task(&self, task: &Task) -> String {
        debug!("Extracting text response from task.");
        trace!(?task, "Task details for text extraction.");

        // First check if we have artifacts that contain results
        trace!("Checking task artifacts for text.");
        if let Some(ref artifacts) = task.artifacts {
            // Get the latest artifact
            if let Some(latest_artifact) = artifacts.iter().last() {
                trace!(artifact_index = latest_artifact.index, "Found latest artifact.");
                // Extract text from the artifact
                let text = latest_artifact.parts.iter()
                    .filter_map(|p| match p {
                        Part::TextPart(tp) => {
                            trace!("Found TextPart in artifact.");
                            Some(tp.text.clone())
                        },
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
                        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(cleaned_text) {
                            let pretty_json = serde_json::to_string_pretty(&json_value).unwrap_or(cleaned_text.to_string());
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
            let text = message.parts.iter()
                .filter_map(|p| match p {
                    Part::TextPart(tp) => {
                        trace!("Found TextPart in status message.");
                        Some(tp.text.clone())
                    },
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
            let agent_messages = history.iter()
                .filter(|m| m.role == Role::Agent)
                .flat_map(|m| {
                    trace!(message_role = ?m.role, "Processing message in history.");
                    m.parts.iter()
                })
                .filter_map(|p| match p {
                    Part::TextPart(tp) => {
                        trace!("Found TextPart in history agent message.");
                        Some(tp.text.clone())
                    },
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
        "No response text available.".to_string()
    }

    /// Run an interactive REPL (Read-Eval-Print Loop)
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    pub async fn run_repl(&mut self) -> Result<()> {
        info!("Entering REPL mode. Type :help for commands."); // Updated info message
        // Print the initial help message
        self.print_repl_help();

        // --- Spawn Background Task for Initial Connection ---
        debug!("Checking if background connection task should be spawned.");
        if let Some(initial_url) = self.client_url() {
            debug!(url = %initial_url, "Spawning background task for initial connection attempt."); // Changed to debug
            println!("⏳ Configured target URL: {}. Attempting connection in background...", initial_url); // Added emoji
            // Clone necessary data for the background task
            // Clone necessary data for the background task's tracing span
            let bg_agent_id = self.agent_id.clone();
            let bg_initial_url = initial_url.clone();
            // Remove agent_directory clone
            let agent_registry = self.agent_registry.clone(); // Canonical registry for execution
            let known_servers = self.known_servers.clone(); // Clone Arc<DashMap>

            tokio::spawn(async move {
                // Create a tracing span for the background task
                let span = tracing::info_span!(
                    "bg_connect",
                    agent_id = %bg_agent_id,
                    target_url = %bg_initial_url
                );
                // Enter the span
                let _enter = span.enter();

                debug!("Background connection task started."); // Changed to debug

                let max_retries = 5;
                let retry_delay = std::time::Duration::from_secs(2);
                let mut connected = false;

                // Create a *new* client instance specifically for this background task
                debug!("Creating A2aClient for background connection.");
                let mut bg_client = A2aClient::new(&initial_url);

                for attempt in 1..=max_retries {
                    debug!(attempt, max_retries, "Attempting to get agent card in background.");
                    match bg_client.get_agent_card().await {
                        Ok(card) => {
                            let remote_agent_name = card.name.clone();
                            info!(remote_agent_name = %remote_agent_name, "Background connection successful."); // Keep info for success
                            println!("✅ Background connection successful to {} ({})", remote_agent_name, initial_url); // Print to console as well

                            // Update canonical AgentRegistry via discover
                            debug!(url = %initial_url, "Attempting to update canonical AgentRegistry via discover.");
                            match agent_registry.discover(&initial_url).await {
                                Ok(discovered_agent_id) => {
                                    debug!(url = %initial_url, %discovered_agent_id, "Successfully updated canonical AgentRegistry."); // Changed to debug
                                    // Update known servers map with the ID from discover
                                    debug!(url = %initial_url, name = %discovered_agent_id, "Updating known servers map.");
                                    known_servers.insert(initial_url.clone(), discovered_agent_id);
                                }
                                Err(reg_err) => {
                                    // Log warning if updating canonical registry fails, but don't fail the connection
                                    warn!(error = %reg_err, url = %initial_url, "Failed to update canonical AgentRegistry after successful connection.");
                                    // Still update known_servers with the name from the card as a fallback
                                    debug!(url = %initial_url, name = %remote_agent_name, "Updating known servers map with card name as fallback.");
                                    known_servers.insert(initial_url.clone(), remote_agent_name.clone());
                                }
                            }

                            connected = true;
                            break; // Exit retry loop on success
                        }
                        Err(e) => {
                            warn!(attempt, max_retries, error = %e, "Background connection attempt failed.");
                            if attempt < max_retries {
                                debug!(delay_seconds = %retry_delay.as_secs(), "Retrying background connection..."); // Changed to debug
                                tokio::time::sleep(retry_delay).await;
                            } else {
                                error!(attempt, max_retries, "Final background connection attempt failed.");
                            }
                        }
                    }
                }

                if !connected {
                    error!(attempts = %max_retries, "Background connection failed after all attempts.");
                    println!("❌ Background connection to {} failed after {} attempts.", initial_url, max_retries); // Print final failure to console
                } else {
                    debug!("Background connection task finished successfully."); // Changed to debug
                }
            }); // Background task ends here
        } else {
            debug!("No initial target URL configured, skipping background connection task.");
        }
        // --- End Background Task Spawn ---


        // Flag to track if we have a listening server running
        let mut server_running = false;
        let mut server_shutdown_token: Option<CancellationToken> = None;
        // Removed misplaced match block and duplicate variable declarations here

        // Check if auto-listen should happen (based on config/env var checked during init)
        // We need to re-evaluate here as config isn't stored directly on self
        let should_auto_listen = self.client_config.target_url.is_none() || // Auto-listen if no target URL
                                 std::env::var("AUTO_LISTEN").map(|v| v == "1" || v.to_lowercase() == "true").unwrap_or(false);
        // Note: We cannot check config.mode.auto_listen here easily as config is not stored.
        // Relying on env var set during init or absence of target_url.
        debug!(%should_auto_listen, "Determined auto-listen status for REPL.");

        // Start server automatically if auto-listen is enabled
        // Use the calculated should_auto_listen variable
        if should_auto_listen {
            info!(port = %self.port, bind_address = %self.bind_address, "Auto-starting server."); // Keep info for server start
            println!("🚀 Auto-starting server on port {}...", self.port);

            // Create a cancellation token
            debug!("Creating cancellation token for auto-started server.");
            let token = CancellationToken::new();
            server_shutdown_token = Some(token.clone());

            // Create a channel to communicate server start status back to REPL
            debug!("Creating channel for server start status.");
            let (tx, rx) = tokio::sync::oneshot::channel();

            // Clone what we need for the task
            trace!("Cloning services and config for server task.");
            let task_service = self.task_service.clone();
            let streaming_service = self.streaming_service.clone();
            let notification_service = self.notification_service.clone();
            let bind_address = self.bind_address.clone();
            let port = self.port;
            debug!("Creating agent card for auto-started server.");
            let agent_card = serde_json::to_value(self.create_agent_card()).unwrap_or_else(|e| { // create_agent_card logs internally
                warn!(error = %e, "Failed to serialize agent card for server. Using empty object.");
                serde_json::json!({})
            });
            trace!(?agent_card, "Agent card JSON for server.");

            let agent_id_clone = self.agent_id.clone(); // Clone for tracing span
            debug!("Spawning server task.");
            tokio::spawn(async move {
                 // Create a tracing span for the server task
                let span = tracing::info_span!(
                    "auto_server_task",
                    agent_id = %agent_id_clone,
                    %port,
                    %bind_address
                );
                // Enter the span
                let _enter = span.enter();
                debug!("Auto-started server task running."); // Changed to debug

                match run_server(
                    port,
                    &bind_address,
                    task_service,
                    streaming_service,
                    notification_service,
                    token.clone(),
                    Some(agent_card),
                ).await {
                    Ok(handle) => {
                        debug!("Server task successfully started."); // Changed to debug
                        // Send success status back to REPL
                        trace!("Sending success status back to REPL via channel.");
                        let _ = tx.send(Ok(())); // Ignore error if REPL already timed out

                        // Wait for the server to complete or be cancelled
                        debug!("Waiting for server task to complete or be cancelled."); // Changed to debug
                        match handle.await {
                            Ok(()) => info!("Server task shut down gracefully."), // Keep info for shutdown
                            Err(e) => error!(error = %e, "Server task join error."),
                        }
                    },
                    Err(e) => {
                        error!(error = %e, "Failed to start server task.");
                        // Send error status back to REPL
                        trace!("Sending error status back to REPL via channel.");
                        let _ = tx.send(Err(format!("{}", e))); // Ignore error if REPL already timed out
                    }
                }
            }); // Server task ends here

            // Wait for the server start status
            debug!("Waiting for server start status from channel.");
            let server_start_timeout = std::time::Duration::from_secs(5); // Slightly longer timeout
            match tokio::time::timeout(server_start_timeout, rx).await {
                Ok(Ok(Ok(()))) => {
                    server_running = true;
                    info!(bind_address = %self.bind_address, port = %self.port, "Auto-started server successfully confirmed by REPL."); // Keep info for confirmation
                    println!("✅ Server started on http://{}:{}", self.bind_address, self.port);
                    println!("   (Server will run until you exit the REPL or send :stop)"); // Indented for clarity
                },
                Ok(Ok(Err(e))) => {
                    error!(bind_address = %self.bind_address, port = %self.port, error = %e, "Error reported during auto-start server initialization.");
                    println!("❌ Error starting server: {}", e);
                    println!("The server could not be started. Try a different port or check for other services using this port.");
                    server_shutdown_token = None; // Clean up token
                },
                Ok(Err(channel_err)) => {
                    error!(bind_address = %self.bind_address, port = %self.port, error = %channel_err, "Server init channel error during auto-start (REPL side).");
                    println!("❌ Error: Server initialization failed due to channel error");
                    server_shutdown_token = None; // Clean up token
                },
                Err(timeout_err) => {
                    error!(bind_address = %self.bind_address, port = %self.port, error = %timeout_err, "Timeout waiting for auto-start server confirmation.");
                    println!("❌ Timeout waiting for server to start");
                    println!("The server is taking too long to start. It might be starting in the background or could have failed.");
                    println!("You can try :stop to cancel any server processes that might be running.");
                    // Keep running flag true so user can try :stop, but clear token as we don't know its state
                    server_running = true;
                    server_shutdown_token = None;
                    warn!("Server state unknown due to timeout, cleared shutdown token.");
                }
            }
        } else {
            debug!("Auto-start server disabled.");
        }

        let stdin = io::stdin();
        let mut reader = stdin.lock();
        let mut input = String::new();

        // Flag to track if we have a listening server running is now set above
        // let mut server_running = false; // Removed duplicate
        // let mut server_shutdown_token: Option<CancellationToken> = None; // Removed duplicate

        loop {
            // Display prompt (with connected agent information if available)
            let prompt = if let Some(url) = self.client_url() {
                format!("agent@{} > ", url)
            } else {
                "agent > ".to_string()
            };
            trace!(prompt = %prompt, "Displaying REPL prompt.");
            print!("{}", prompt);
            io::stdout().flush().ok();

            input.clear();
            trace!("Reading line from stdin.");
            if reader.read_line(&mut input)? == 0 {
                // EOF reached (e.g., Ctrl+D)
                info!("EOF detected on stdin. Exiting REPL.");
                println!("\nEOF detected. Exiting REPL."); // Add newline for cleaner exit
                // Perform cleanup similar to :quit
                // REMOVED agent directory saving on exit

                if let Some(token) = server_shutdown_token.take() {
                    info!("Shutting down server due to EOF.");
                    println!("Shutting down server...");
                    token.cancel();
                    trace!("Server cancellation token cancelled.");
                } else {
                    trace!("No server running or token available to cancel on EOF.");
                }
                break; // Exit loop
            }

            let input_trimmed = input.trim();
            debug!(user_input = %input_trimmed, "REPL input received.");

            if input_trimmed.is_empty() {
                trace!("Empty input received, continuing loop.");
                continue;
            }

            // --- Updated REPL Command Handling ---
            if input_trimmed.starts_with(":") {
                debug!(command = %input_trimmed, "Processing REPL command.");

                let parts: Vec<&str> = input_trimmed.splitn(2, ' ').collect();
                let command = parts[0].trim_start_matches(':').to_lowercase(); // Lowercase for matching
                let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

                // Handle commands that need direct access to run_repl state (:listen, :stop, :quit)
                if command == "quit" {
                    info!("User initiated :quit command.");
                    println!("👋 Exiting REPL. Goodbye!");
                    if let Some(token) = server_shutdown_token.take() {
                        info!("Shutting down server via :quit command.");
                        println!("🔌 Shutting down server...");
                        token.cancel();
                        trace!("Server cancellation token cancelled.");
                    }
                    break; // Exit loop
                } else if command == "listen" {
                    debug!(port_str = %args, "Processing :listen command directly in run_repl.");
                    if server_running {
                        warn!("Attempted to listen while server already running.");
                        println!("⚠️ Server already running. Stop it first with :stop");
                        continue;
                    }
                    match args.parse::<u16>() {
                        Ok(port) => {
                            self.port = port;
                            let token = CancellationToken::new();
                            server_shutdown_token = Some(token.clone()); // Store token locally
                            info!(%port, "Attempting to start server.");
                            println!("🚀 Starting server on port {}...", port);

                            // Clone what we need for the task
                            let task_service = self.task_service.clone();
                            let streaming_service = self.streaming_service.clone();
                            let notification_service = self.notification_service.clone();
                            let bind_address = self.bind_address.clone();
                            let agent_card = serde_json::to_value(self.create_agent_card()).unwrap_or_else(|e| {
                                warn!("Failed to serialize agent card: {}", e);
                                serde_json::json!({})
                            });

                            // Create a channel to communicate server start status back to REPL
                            let (tx, rx) = tokio::sync::oneshot::channel();

                            tokio::spawn(async move {
                                match run_server(
                                    port,
                                    &bind_address,
                                    task_service,
                                    streaming_service,
                                    notification_service,
                                    token.clone(),
                                    Some(agent_card),
                                ).await {
                                    Ok(handle) => {
                                        let _ = tx.send(Ok(())); // Send success
                                        debug!(%port, "Server task started.");
                                        match handle.await { // Wait for server task completion
                                            Ok(()) => info!(%port, "Server shut down gracefully."),
                                            Err(e) => error!(%port, error = %e, "Server error."),
                                        }
                                    },
                                    Err(e) => {
                                        let error_msg = format!("Failed to start server: {}", e);
                                        let _ = tx.send(Err(error_msg.clone())); // Send error
                                        error!(%port, error = %e, "Failed to start server task.");
                                    }
                                }
                            }); // Server task ends here

                            // Wait for the server start status
                            match tokio::time::timeout(std::time::Duration::from_secs(3), rx).await {
                                Ok(Ok(Ok(()))) => {
                                    server_running = true; // Update local state
                                    info!(%port, "Server started successfully via REPL command.");
                                    println!("✅ Server started on http://{}:{}", self.bind_address, port);
                                    println!("   (Server will run until you exit the REPL or send :stop)");
                                },
                                Ok(Ok(Err(e))) => {
                                    error!(%port, error = %e, "Error starting server via REPL command.");
                                    println!("❌ Error starting server: {}", e);
                                    println!("The server could not be started. Try a different port or check for other services using this port.");
                                    server_shutdown_token = None; // Clean up token
                                },
                                Ok(Err(channel_err)) => {
                                    error!(%port, error = %channel_err, "Server init channel error via REPL command.");
                                    println!("❌ Error: Server initialization failed due to channel error");
                                    server_shutdown_token = None; // Clean up token
                                },
                                Err(timeout_err) => {
                                    error!(%port, error = %timeout_err, "Timeout waiting for server start via REPL command.");
                                    println!("❌ Timeout waiting for server to start");
                                    println!("The server is taking too long to start. It might be starting in the background or could have failed.");
                                    println!("You can try :stop to cancel any server processes that might be running.");
                                    server_running = true; // Keep running flag true so user can try :stop
                                    // server_shutdown_token remains Some, allowing :stop attempt
                                }
                            }
                        },
                        Err(parse_err) => {
                            error!(input = %args, error = %parse_err, "Invalid port format for :listen command.");
                            println!("❌ Error: Invalid port number. Please provide a valid port.");
                        }
                    }
                } else if command == "stop" {
                    debug!("Processing :stop command directly in run_repl.");
                    if let Some(token) = server_shutdown_token.take() { // Take token from local variable
                        info!("Stopping server.");
                        println!("🔌 Shutting down server...");
                        token.cancel();
                        server_running = false; // Update local state
                        println!("✅ Server stop signal sent.");
                    } else {
                        warn!("Attempted to stop server, but none was running or token missing.");
                        println!("⚠️ No server currently running or already stopped.");
                    }
                } else {
                    // Handle other commands via the central handler
                    match self.handle_repl_command(&command, args).await {
                        Ok(response) => println!("{}", response),
                        Err(e) => println!("❌ Error: {}", e),
                    }
                }
            } else { // Input does not start with ':'
                // Process as local message (which now also checks for command keywords)
                debug!(message = %input_trimmed, "Processing input as local message/command.");
                match self.process_message_locally(input_trimmed).await {
                    Ok(response) => {
                        println!("\n🤖 Agent response:\n{}\n", response);
                        debug!(response_length = %response.len(), "Local processing successful, displayed response.");
                    },
                    Err(e) => {
                        let error_msg = format!("Error processing message: {}", e);
                        error!(error = %e, "Error processing message locally.");
                        println!("❌ {}", error_msg);
                    }
                }
            }
            // --- End Updated REPL Command Handling ---
        }

        info!("Exiting REPL mode.");
        Ok(())
    }

    /// Create a new session ID and store it.
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    pub fn create_new_session(&mut self) -> String {
        let session_id = format!("session-{}", Uuid::new_v4());
        info!(new_session_id = %session_id, "Creating new session."); // Keep info for new session
        self.current_session_id = Some(session_id.clone());
        trace!("Inserting new session ID into session_tasks map.");
        self.session_tasks.insert(session_id.clone(), Vec::new());
        session_id
    }

    /// Add task ID to the current session's task list.
    #[instrument(skip(self, task), fields(agent_id = %self.agent_id, task_id = %task.id))]
    async fn save_task_to_history(&self, task: Task) -> Result<()> {
        debug!("Attempting to save task to session history.");
        if let Some(session_id) = &self.current_session_id {
            trace!(%session_id, "Current session ID found.");
            if let Some(mut tasks) = self.session_tasks.get_mut(session_id) {
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
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    pub async fn get_current_session_tasks(&self) -> Result<Vec<Task>> {
        let mut tasks = Vec::new();
        if let Some(session_id) = &self.current_session_id {
            debug!(session_id = %session_id, "Fetching full tasks for current session."); // Changed to debug
            if let Some(task_ids) = self.session_tasks.get(session_id) {
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

                    match self.task_service.get_task(params).await {
                        Ok(task) => {
                            trace!(task_id = %task.id, "Successfully fetched task details.");
                            tasks.push(task);
                        },
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
        } else {
             warn!("Cannot get session tasks: No active session.");
        }
        Ok(tasks)
    }

    // REMOVED log_agent_action function
    // REMOVED clone_for_logging function

    /// Get the URL of the currently configured client from the agent's config
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    fn client_url(&self) -> Option<String> {
        // Use the target_url stored in the agent's own configuration
        let url = self.client_config.target_url.clone();
        trace!(?url, "Retrieved client URL from config.");
        url
    }

    /// Run the agent server
    #[instrument(skip(self), fields(agent_id = %self.agent_id, port = %self.port, bind_address = %self.bind_address))]
    pub async fn run(&self) -> Result<()> {
        info!("Starting agent server run sequence."); // Keep info for server start sequence
        // Create a cancellation token for graceful shutdown
        debug!("Creating server shutdown token.");
        let shutdown_token = CancellationToken::new();
        let shutdown_token_clone = shutdown_token.clone();

        // Set up signal handlers for graceful shutdown
        debug!("Setting up signal handlers (Ctrl+C).");
        let agent_id_clone = self.agent_id.clone(); // Clone for the signal handler task
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
        let agent_card = self.create_agent_card(); // Logs internally
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

        info!(bind_address = %self.bind_address, port = %self.port, "Attempting to start server via run_server.");
        // Start the server - handle the Box<dyn Error> specially
        let server_handle = match run_server(
            self.port,
            &self.bind_address,
            self.task_service.clone(),
            self.streaming_service.clone(),
            self.notification_service.clone(),
            shutdown_token.clone(),
            Some(agent_card_json), // Pass our custom agent card
        ).await {
            Ok(handle) => {
                info!(bind_address = %self.bind_address, port = %self.port, "Server started successfully."); // Keep info for server start success
                handle
            },
            Err(e) => {
                error!(bind_address = %self.bind_address, port = %self.port, error = %e, "Failed to start server.");
                // Return an error that can be handled by the caller (e.g., main)
                return Err(anyhow!("Failed to start server: {}", e));
            }
        };

        println!("🔌 Server running on http://{}:{}", self.bind_address, self.port); // Added emoji

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

    /// Send a task to a remote agent using the A2A client, ensuring session consistency.
    #[instrument(skip(self, message), fields(agent_id = %self.agent_id, message_len = message.len()))]
    pub async fn send_task_to_remote(&mut self, message: &str) -> Result<Task> {
        debug!("Attempting to send task to remote agent."); // Changed to debug
        trace!(message = %message, "Message content for remote task.");
        // ① Ensure a session exists locally
        debug!("Ensuring local session exists.");
        self.ensure_session().await; // Logs internally if new session created
        let session_id = self.current_session_id.clone(); // Clone session_id for sending
        trace!(?session_id, "Using session ID for remote task.");

        let remote_url = self.client_url().unwrap_or_else(|| "unknown".to_string());
        info!(remote_url = %remote_url, session_id = ?session_id, "Attempting to send task to remote agent."); // Keep info for sending attempt

        // Get mutable reference to the client
        debug!("Getting A2A client instance.");
        let client = match self.client.as_mut() {
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
        match self.task_service.import_task(task.clone()).await {
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
        self.save_task_to_history(task.clone()).await?; // save_task_to_history logs internally

        debug!(task_id = %task.id, session_id = ?session_id, "Remote task processing initiated and linked to session."); // Changed to debug
        Ok(task) // Return the original task received from the remote agent
    }

    /// Get capabilities of a remote agent (using A2A client)
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    pub async fn get_remote_agent_card(&mut self) -> Result<AgentCard> {
        let remote_url = self.client_url().unwrap_or_else(|| "unknown".to_string());
        info!(remote_url = %remote_url, "Attempting to get remote agent card."); // Keep info for attempt

        // Check if we have a client configured
        debug!("Checking for A2A client configuration.");
        if let Some(client) = &mut self.client {
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

    /// Create an agent card for this agent instance.
    // No instrument macro needed here as it's synchronous and called from instrumented contexts.
    pub fn create_agent_card(&self) -> AgentCard {
        debug!(agent_id = %self.agent_id, "Creating agent card for this instance.");
        // Construct AgentCapabilities based on actual capabilities
        // TODO: Make these capabilities configurable or dynamically determined
        trace!("Determining agent capabilities.");
        let capabilities = AgentCapabilities {
            push_notifications: true, // Example: Assuming supported
            state_transition_history: true, // Example: Assuming supported
            streaming: true, // Example: Assuming supported by default bidirectional agent server? Check run_server.
            // Add other fields from AgentCapabilities if they exist
        };
        trace!(?capabilities, "Agent capabilities determined.");

        let card = AgentCard {
            // id field does not exist on AgentCard in types.rs
            name: self.agent_name.clone(), // Use the configured agent name (falls back to agent_id if None)
            description: Some("A bidirectional A2A agent that can process tasks and delegate to other agents".to_string()),
            version: AGENT_VERSION.to_string(), // version is String
            url: format!("http://{}:{}", self.bind_address, self.port), // url is String
            capabilities, // Use the capabilities struct
            authentication: None, // Set authentication if needed, otherwise None
            default_input_modes: vec!["text".to_string()], // Example
            default_output_modes: vec!["text".to_string()], // Example
            documentation_url: None,
            provider: None,
            skills: vec![], // skills is Vec<AgentSkill>, provide empty vec
            // Add other fields from AgentCard if they exist
        };
        trace!(?card, "Agent card created.");
        card
    }

    /// Handles the ':connect' REPL command logic.
    #[instrument(skip(self, target), fields(agent_id = %self.agent_id))]
    async fn handle_connect(&mut self, target: &str) -> Result<String> {
        debug!(target = %target, "Handling connect command.");

        // Check if it's a number (referring to a server in the list)
        if let Ok(server_idx) = target.parse::<usize>() {
            // Read from shared known_servers map
            let mut server_list: Vec<(String, String)> = self.known_servers.iter()
                .map(|entry| (entry.value().clone(), entry.key().clone())) // (Name, URL)
                .collect();
            server_list.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by name to match display order

            if server_idx > 0 && server_idx <= server_list.len() {
                let (name, url) = &server_list[server_idx - 1];
                let url_clone = url.clone(); // Clone for async block

                // Create a new client with the selected URL
                self.client = Some(A2aClient::new(&url_clone));
                info!(server_name = %name, server_url = %url_clone, "Connecting to known server by number.");
                let connect_msg = format!("🔗 Connecting to {}: {}", name, url_clone);

                // Attempt to get card after connecting (spawn to avoid blocking REPL)
                let mut agent_client = self.client.as_mut().unwrap().clone(); // Clone client for task
                let agent_registry_clone = self.agent_registry.clone(); // Clone registry for update
                let known_servers_clone = self.known_servers.clone(); // Clone known_servers
                let agent_id_clone = self.agent_id.clone(); // For tracing span

                tokio::spawn(async move {
                    let span = tracing::info_span!("connect_get_card", agent_id = %agent_id_clone, remote_url = %url_clone);
                    let _enter = span.enter();
                    match agent_client.get_agent_card().await {
                        Ok(card) => {
                            let remote_agent_name = card.name.clone();
                            debug!(%remote_agent_name, "Successfully got card after connecting.");
                            // Update known servers
                            known_servers_clone.insert(url_clone.clone(), remote_agent_name.clone());
                            // Update canonical registry using discover
                            match agent_registry_clone.discover(&url_clone).await {
                                Ok(discovered_agent_id) => {
                                    debug!(url = %url_clone, %discovered_agent_id, "Successfully updated canonical registry after connecting by number.");
                                    if discovered_agent_id != remote_agent_name {
                                        debug!(url = %url_clone, old_name = %remote_agent_name, new_id = %discovered_agent_id, "Updating known_servers with discovered ID.");
                                        known_servers_clone.insert(url_clone.clone(), discovered_agent_id);
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
                Err(anyhow!("Invalid server number. Use :servers to see available servers."))
            }
        } else {
            // Treat as URL
            let target_url = target.to_string();
            if target_url.is_empty() {
                error!("No URL provided for :connect command.");
                return Err(anyhow!("No URL provided. Use :connect URL"));
            }

            debug!(url = %target_url, "Attempting connection to URL.");
            let mut client = A2aClient::new(&target_url);

            match client.get_agent_card().await {
                Ok(card) => {
                    let remote_agent_name = card.name.clone();
                    info!(remote_agent_name = %remote_agent_name, url = %target_url, "Successfully connected to agent.");
                    let success_msg = format!("✅ Successfully connected to agent: {}", remote_agent_name);

                    self.known_servers.insert(target_url.clone(), remote_agent_name.clone());
                    self.client = Some(client);

                    // Update registry in background
                    let registry_clone = self.agent_registry.clone();
                    let url_clone = target_url.clone();
                    let known_servers_clone = self.known_servers.clone();
                    let remote_agent_name_clone = remote_agent_name.clone();
                    tokio::spawn(async move {
                        match registry_clone.discover(&url_clone).await {
                            Ok(discovered_agent_id) => {
                                debug!(url = %url_clone, %discovered_agent_id, "Successfully updated canonical registry after connecting by URL.");
                                if discovered_agent_id != remote_agent_name_clone {
                                    debug!(url = %url_clone, old_name = %remote_agent_name_clone, new_id = %discovered_agent_id, "Updating known_servers with discovered ID.");
                                    known_servers_clone.insert(url_clone.clone(), discovered_agent_id);
                                }
                            }
                            Err(e) => {
                                warn!(error = %e, url = %url_clone, "Failed to update canonical registry after connecting by URL.");
                            }
                        }
                    });
                    Ok(success_msg)
                },
                Err(e) => {
                    error!(url = %target_url, error = %e, "Failed to connect to agent via URL.");
                    // Don't ask y/n here, just return error
                    if !self.known_servers.contains_key(&target_url) {
                         let unknown_name = "Unknown Agent".to_string();
                         self.known_servers.insert(target_url.clone(), unknown_name.clone());
                         info!(url = %target_url, "Added URL to known servers as 'Unknown Agent' after failed connection attempt.");
                    }
                    Err(anyhow!("Failed to connect to agent at {}: {}. Please check the server is running and the URL is correct. URL added to known servers as 'Unknown Agent'.", target_url, e))
                }
            }
        }
    }

    /// Handles the ':disconnect' REPL command logic.
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    fn handle_disconnect(&mut self) -> Result<String> {
        debug!("Handling disconnect command.");
        if self.client.is_some() {
            let url = self.client_url().unwrap_or_else(|| "unknown".to_string());
            self.client = None;
            info!(disconnected_from = %url, "Disconnected from remote agent.");
            Ok(format!("🔌 Disconnected from {}", url))
        } else {
            debug!("Attempted disconnect but not connected.");
            Ok("⚠️ Not connected to any server".to_string()) // Return as Ok string
        }
    }

    /// Handles the ':servers' REPL command logic.
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    fn handle_list_servers(&self) -> Result<String> {
        debug!("Handling list_servers command.");
        if self.known_servers.is_empty() {
            info!("No known servers found.");
            Ok("📡 No known servers. Connect to a server or wait for background connection attempt.".to_string())
        } else {
            info!(count = %self.known_servers.len(), "Formatting known servers list.");
            let mut output = String::from("\n📡 Known Servers:\n");
            let mut server_list: Vec<(String, String)> = self.known_servers.iter()
                .map(|entry| (entry.value().clone(), entry.key().clone())) // (Name, URL)
                .collect();
            server_list.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by name

            for (i, (name, url)) in server_list.iter().enumerate() {
                let marker = if Some(url) == self.client_url().as_ref() { "*" } else { " " };
                output.push_str(&format!("  {}{}: {} - {}\n", marker, i + 1, name, url));
            }
            output.push_str("\nUse :connect N to connect to a server by number\n");
            Ok(output)
        }
    }

     /// Handles the ':remote' REPL command logic.
    #[instrument(skip(self, message), fields(agent_id = %self.agent_id))]
    async fn handle_remote_message(&mut self, message: &str) -> Result<String> {
        debug!(message = %message, "Handling remote message command.");
        if message.is_empty() {
            error!("No message provided for :remote command.");
            return Err(anyhow!("No message provided. Use :remote MESSAGE"));
        }
        if self.client.is_none() {
            error!("Cannot send remote task: Not connected to a remote agent.");
            return Err(anyhow!("Not connected to a remote agent. Use :connect URL first."));
        }

        info!(message = %message, "Sending task to remote agent.");
        let task = self.send_task_to_remote(message).await?; // send_task_to_remote logs internally now
        info!(task_id = %task.id, status = ?task.status.state, "Remote task sent successfully.");

        let mut response = format!("✅ Task sent successfully!\n   Task ID: {}\n   Initial state reported by remote: {:?}",
                                   task.id, task.status.state);

        if task.status.state == TaskState::Completed {
            let task_response = self.extract_text_from_task(&task);
            if task_response != "No response text available." {
                 debug!(task_id = %task.id, "Appending immediate response from completed remote task.");
                 response.push_str("\n\n📥 Response from remote agent:\n");
                 response.push_str(&task_response);
            }
        }
        Ok(response)
    }

    /// Handles the ':session new' REPL command logic.
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    fn handle_new_session(&mut self) -> Result<String> {
        let session_id = self.create_new_session(); // create_new_session logs internally now
        info!("Created new session: {}", session_id);
        Ok(format!("✅ Created new session: {}", session_id))
    }

    /// Handles the ':session show' REPL command logic.
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    fn handle_show_session(&self) -> Result<String> {
        if let Some(session_id) = &self.current_session_id {
            debug!(session_id = %session_id, "Displaying current session ID.");
            Ok(format!("🔍 Current session: {}", session_id))
        } else {
            info!("No active session.");
            Ok("⚠️ No active session. Use :session new to create one.".to_string())
        }
    }

    /// Handles the ':history' REPL command logic.
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    async fn handle_history(&self) -> Result<String> {
        if let Some(session_id) = &self.current_session_id {
            debug!(session_id = %session_id, "Fetching history for current session.");
            let tasks = self.get_current_session_tasks().await?; // get_current_session_tasks logs internally
            if tasks.is_empty() {
                info!(session_id = %session_id, "No tasks found in current session history.");
                Ok("📭 No messages in current session.".to_string())
            } else {
                info!(session_id = %session_id, task_count = %tasks.len(), "Formatting session history.");
                let mut output = format!("\n📝 Session History ({} Tasks):\n", tasks.len());
                for task in tasks.iter() {
                    output.push_str(&format!("--- Task ID: {} (Status: {:?}) ---\n", task.id, task.status.state));
                    if let Some(history) = &task.history {
                        for message in history {
                            let role_icon = match message.role { Role::User => "👤", Role::Agent => "🤖", _ => "➡️" };
                            let text = message.parts.iter()
                                .filter_map(|p| match p {
                                    Part::TextPart(tp) => Some(tp.text.as_str()),
                                    Part::FilePart(_) => Some("[File Part]"),
                                    Part::DataPart(_) => Some("[Data Part]"),
                                })
                                .collect::<Vec<_>>()
                                .join(" ");
                            let display_text = if text.len() > 100 { format!("{}...", &text[..97]) } else { text.to_string() };
                            output.push_str(&format!("  {} {}: {}\n", role_icon, message.role, display_text));
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
            Err(anyhow!("No active session. Use :session new to create one."))
        }
    }

    /// Handles the ':tasks' REPL command logic.
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    async fn handle_list_tasks(&self) -> Result<String> {
         if let Some(session_id) = &self.current_session_id {
            debug!(session_id = %session_id, "Fetching task list for current session.");
            if let Some(task_ids) = self.session_tasks.get(session_id) {
                if task_ids.is_empty() {
                    info!(session_id = %session_id, "No tasks found in current session map.");
                    Ok("📭 No tasks recorded in current session.".to_string())
                } else {
                    info!(session_id = %session_id, task_count = %task_ids.len(), "Formatting task list.");
                    let mut output = format!("\n📋 Tasks Recorded in Current Session ({}):\n", task_ids.len());
                    for (i, task_id) in task_ids.iter().enumerate() {
                        let params = TaskQueryParams { id: task_id.clone(), history_length: Some(0), metadata: None };
                        let status_str = match self.task_service.get_task(params).await {
                            Ok(task) => format!("{:?}", task.status.state),
                            Err(_) => "Status Unknown".to_string(),
                        };
                        output.push_str(&format!("  {}. {} - Status: {}\n", i + 1, task_id, status_str));
                    }
                    Ok(output)
                }
            } else {
                 info!(session_id = %session_id, "Session ID not found in task map.");
                 Ok("📭 No tasks recorded for current session (session ID not found).".to_string())
            }
        } else {
            info!("Cannot list tasks: No active session.");
            Err(anyhow!("No active session. Use :session new to create one."))
        }
    }

    /// Handles the ':task ID' REPL command logic.
    #[instrument(skip(self, task_id), fields(agent_id = %self.agent_id))]
    async fn handle_show_task(&self, task_id: &str) -> Result<String> {
        debug!(task_id = %task_id, "Handling show task command.");
        if task_id.is_empty() {
            error!("No task ID provided for :task command.");
            return Err(anyhow!("No task ID provided. Use :task TASK_ID"));
        }

        info!(task_id = %task_id, "Fetching details for task.");
        let params = TaskQueryParams { id: task_id.to_string(), history_length: None, metadata: None };
        match self.task_service.get_task(params).await {
            Ok(task) => {
                debug!(task_id = %task.id, "Formatting details for task.");
                let mut output = String::from("\n🔍 Task Details:\n");
                output.push_str(&format!("  ID: {}\n", task.id));
                output.push_str(&format!("  Status: {:?}\n", task.status.state));
                output.push_str(&format!("  Session: {}\n", task.session_id.as_deref().unwrap_or("None")));
                output.push_str(&format!("  Timestamp: {}\n", task.status.timestamp.map(|t| t.to_rfc3339()).as_deref().unwrap_or("None")));
                let artifact_count = task.artifacts.as_ref().map_or(0, |a| a.len());
                output.push_str(&format!("  Artifacts: {} (use :artifacts {} to view)\n", artifact_count, task.id));

                if let Some(message) = &task.status.message {
                     let text = message.parts.iter().filter_map(|p| match p { Part::TextPart(tp) => Some(tp.text.as_str()), _ => None }).collect::<Vec<_>>().join("\n");
                    output.push_str(&format!("\n  Status Message: {}\n", text));
                }
                if let Some(history) = &task.history {
                     output.push_str(&format!("\n  History Preview ({} messages):\n", history.len()));
                     for msg in history.iter().take(5) {
                         let role_icon = match msg.role { Role::User => "👤", Role::Agent => "🤖", _ => "➡️" };
                         let text = msg.parts.iter().filter_map(|p| match p { Part::TextPart(tp) => Some(tp.text.as_str()), _ => None }).collect::<Vec<_>>().join(" ");
                         let display_text = if text.len() > 60 { format!("{}...", &text[..57]) } else { text.to_string() };
                         output.push_str(&format!("    {} {}: {}\n", role_icon, msg.role, display_text));
                     }
                     if history.len() > 5 { output.push_str("    ...\n"); }
                } else {
                     output.push_str("\n  History: Not available or requested.\n");
                }
                Ok(output)
            },
            Err(e) => {
                error!(task_id = %task_id, error = %e, "Failed to get task details.");
                Err(anyhow!("Failed to get task: {}", e))
            }
        }
    }

     /// Handles the ':artifacts ID' REPL command logic.
    #[instrument(skip(self, task_id), fields(agent_id = %self.agent_id))]
    async fn handle_show_artifacts(&self, task_id: &str) -> Result<String> {
        debug!(task_id = %task_id, "Handling show artifacts command.");
        if task_id.is_empty() {
            error!("No task ID provided for :artifacts command.");
            return Err(anyhow!("No task ID provided. Use :artifacts TASK_ID"));
        }

        info!(task_id = %task_id, "Fetching artifacts for task.");
        let params = TaskQueryParams { id: task_id.to_string(), history_length: Some(0), metadata: None };
        match self.task_service.get_task(params).await {
            Ok(task) => {
                if let Some(artifacts) = &task.artifacts {
                    if artifacts.is_empty() {
                        info!(task_id = %task.id, "No artifacts found for task.");
                        Ok(format!("📦 No artifacts for task {}", task.id))
                    } else {
                        info!(task_id = %task.id, count = %artifacts.len(), "Formatting artifacts.");
                        let mut output = format!("\n📦 Artifacts for Task {} ({}):\n", task.id, artifacts.len());
                        for (i, artifact) in artifacts.iter().enumerate() {
                            output.push_str(&format!("  {}. Name: '{}', Desc: '{}', Index: {}, Parts: {}\n",
                                                     i + 1,
                                                     artifact.name.as_deref().unwrap_or("N/A"),
                                                     artifact.description.as_deref().unwrap_or("N/A"),
                                                     artifact.index,
                                                     artifact.parts.len()));
                        }
                        Ok(output)
                    }
                } else {
                    info!(task_id = %task.id, "Task found, but has no artifacts field.");
                    Ok(format!("📦 No artifacts found for task {}", task.id))
                }
            },
            Err(e) => {
                error!(task_id = %task_id, error = %e, "Failed to get task for artifacts.");
                Err(anyhow!("Failed to get task: {}", e))
            }
        }
    }

    /// Handles the ':cancelTask ID' REPL command logic.
    #[instrument(skip(self, task_id), fields(agent_id = %self.agent_id))]
    async fn handle_cancel_task(&self, task_id: &str) -> Result<String> {
        debug!(task_id = %task_id, "Handling cancel task command.");
        if task_id.is_empty() {
            error!("No task ID provided for :cancelTask command.");
            return Err(anyhow!("No task ID provided. Use :cancelTask TASK_ID"));
        }

        info!(task_id = %task_id, "Attempting to cancel task.");
        let params = TaskIdParams { id: task_id.to_string(), metadata: None };
        match self.task_service.cancel_task(params).await {
            Ok(task) => {
                info!(task_id = %task.id, status = ?task.status.state, "Successfully canceled task.");
                Ok(format!("✅ Successfully canceled task {}\n   Current state: {:?}", task.id, task.status.state))
            },
            Err(e) => {
                error!(task_id = %task_id, error = %e, "Failed to cancel task.");
                Err(anyhow!("Failed to cancel task: {}", e))
            }
        }
    }

    /// Handles the ':tool NAME [PARAMS]' REPL command logic.
    #[instrument(skip(self, tool_name, params_str), fields(agent_id = %self.agent_id))]
    async fn handle_tool_command(&self, tool_name: &str, params_str: &str) -> Result<String> {
        debug!(tool_name = %tool_name, params_str = %params_str, "Handling tool command.");
        if tool_name.is_empty() {
            error!("No tool name provided for :tool command.");
            return Err(anyhow!("No tool name provided. Use :tool TOOL_NAME [JSON_PARAMS]"));
        }

        let params_json: Value = match serde_json::from_str(params_str) {
            Ok(json) => json,
            Err(e) => {
                error!(error = %e, input_params = %params_str, "Failed to parse JSON parameters for :tool command.");
                return Err(anyhow!("Invalid JSON parameters: {}", e));
            }
        };

        if let Some(executor) = &self.task_service.tool_executor {
            match executor.execute_tool(tool_name, params_json).await {
                Ok(result) => {
                    info!(tool_name = %tool_name, "Tool executed successfully via REPL.");
                    let result_pretty = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
                    Ok(format!("\n🛠️ Tool Result ({}):\n{}\n", tool_name, result_pretty))
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

    // Add handle_listen and handle_stop, needing access to server_running and server_shutdown_token
    // These cannot be easily refactored without passing the state, which we are avoiding.
    // They will remain directly in run_repl for now.

    /// Handles the ':card' REPL command logic.
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    fn handle_show_card(&self) -> Result<String> {
        debug!("Handling show card command.");
        let card = self.create_agent_card(); // Logs internally
        info!(agent_name = %card.name, "Formatting agent card.");
        let mut output = String::from("\n📇 Agent Card:\n");
        output.push_str(&format!("  Name: {}\n", card.name));
        output.push_str(&format!("  Description: {}\n", card.description.as_deref().unwrap_or("None")));
        output.push_str(&format!("  URL: {}\n", card.url));
        output.push_str(&format!("  Version: {}\n", card.version));
        output.push_str("  Capabilities:\n");
        output.push_str(&format!("    - Streaming: {}\n", card.capabilities.streaming));
        output.push_str(&format!("    - Push Notifications: {}\n", card.capabilities.push_notifications));
        output.push_str(&format!("    - State Transition History: {}\n", card.capabilities.state_transition_history));
        output.push_str(&format!("  Input Modes: {}\n", card.default_input_modes.join(", ")));
        output.push_str(&format!("  Output Modes: {}\n", card.default_output_modes.join(", ")));
        output.push_str("\n");
        Ok(output)
    }

    /// Handles the ':help' REPL command logic.
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    fn handle_help(&self) -> Result<String> {
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
             You can also type many commands without the ':' prefix (e.g., 'connect URL', 'list servers').\n"
        ))
    }

    /// Central handler for REPL commands (called by run_repl and process_message_locally)
    /// Needs mutable access to self for state changes.
    /// server_running and server_shutdown_token need to be passed in/out or managed differently.
    /// For now, :listen and :stop are omitted from this refactoring.
    #[instrument(skip(self, command, args), fields(agent_id = %self.agent_id))]
    async fn handle_repl_command(&mut self, command: &str, args: &str) -> Result<String> {
        debug!(%command, %args, "Handling REPL command via central handler.");
        match command {
            "help" => self.handle_help(),
            "card" => self.handle_show_card(),
            "servers" => self.handle_list_servers(),
            "connect" => self.handle_connect(args).await,
            "disconnect" => self.handle_disconnect(),
            "remote" => self.handle_remote_message(args).await,
            "session" => {
                match args {
                    "new" => self.handle_new_session(),
                    "show" => self.handle_show_session(),
                    _ => Err(anyhow!("Unknown session command. Use ':session new' or ':session show'"))
                }
            }
            "history" => self.handle_history().await,
            "tasks" => self.handle_list_tasks().await,
            "task" => self.handle_show_task(args).await,
            "artifacts" => self.handle_show_artifacts(args).await,
            "cancelTask" => self.handle_cancel_task(args).await,
            "tool" => {
                let (tool_name, params_str) = match args.split_once(' ') {
                    Some((name, params)) => (name, params),
                    None => (args, "{}"), // Assume only tool name if no space
                };
                self.handle_tool_command(tool_name, params_str).await
            }
            // :listen and :stop are NOT handled here due to state access issues
            "listen" | "stop" => {
                 warn!(":listen and :stop commands must be handled directly in run_repl due to server state management.");
                 Err(anyhow!("Command '{}' cannot be handled by this central handler.", command))
            }
            _ => {
                warn!(%command, "Attempted to handle unknown command centrally.");
                Err(anyhow!("Unknown command: '{}'", command))
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize)]

pub struct ServerConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_agent_id")]
    pub agent_id: String,
    // Optional name for the agent card, defaults to agent_id if not set
    pub agent_name: Option<String>,
}

/// Client configuration section
#[derive(Clone, Debug, Deserialize)]
pub struct ClientConfig {
    pub target_url: Option<String>,
}

/// LLM configuration section 
#[derive(Clone, Debug, Deserialize)]
pub struct LlmConfig {
    pub claude_api_key: Option<String>,
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
}

/// Tool configuration section
#[derive(Clone, Debug, Deserialize, Default)]
pub struct ToolsConfig {
    #[serde(default)]
    pub enabled: Vec<String>,
    
    /// Path to store/load the agent directory as JSON
    #[serde(default)]
    pub agent_directory_path: Option<String>,
}

/// Configuration for the bidirectional agent
#[derive(Clone, Debug, Deserialize)]
pub struct BidirectionalAgentConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)] // <-- Add default back
    pub client: ClientConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    #[serde(default)]
    pub tools: ToolsConfig, // <-- new
    
    // Mode configuration
    #[serde(default)]
    pub mode: ModeConfig,
    
    // Path to the config file (for reference)
    #[serde(skip)]
    pub config_file_path: Option<String>,
}

/// Operation mode configuration
#[derive(Clone, Debug, Deserialize, Default)]
pub struct ModeConfig {
    // Interactive REPL mode
    #[serde(default)]
    pub repl: bool,
    
    // Direct message to process (non-interactive mode)
    pub message: Option<String>,

    // Remote agent operations
    #[serde(default)] // Make this optional, defaults to false if missing
    pub get_agent_card: bool,
    pub remote_task: Option<String>,

    // Auto-listen on server port at startup
    #[serde(default)]
    pub auto_listen: bool,

    // Optional file to append REPL interactions (input/output)
    #[serde(default)]
    pub repl_log_file: Option<String>,
}

// Default functions
fn default_port() -> u16 {
    DEFAULT_PORT
}

fn default_bind_address() -> String {
    DEFAULT_BIND_ADDRESS.to_string()
}

fn default_agent_id() -> String {
    format!("bidirectional-{}", Uuid::new_v4())
}

fn default_system_prompt() -> String {
    SYSTEM_PROMPT.to_string()
}

// Default implementations
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            bind_address: default_bind_address(),
            agent_id: default_agent_id(),
            agent_name: None, // Initialize the optional agent_name field
        }
    }
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            target_url: None,
        }
    }
}

impl Default for LlmConfig {
    fn default() -> Self {
        Self {
            claude_api_key: None,
            system_prompt: default_system_prompt(),
        }
    }
}

impl Default for BidirectionalAgentConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            client: ClientConfig::default(),
            llm: LlmConfig::default(),
            tools: ToolsConfig::default(), // <-- new
            mode: ModeConfig::default(),
            config_file_path: None,
        }
    }
}

impl BidirectionalAgentConfig {
    /// Load configuration from a TOML file
    #[instrument(skip(path), fields(path = %path.as_ref().display()))]
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        debug!("Loading configuration from file.");
        let config_str = fs::read_to_string(path.as_ref())
            .map_err(|e| anyhow!("Failed to read config file: {}", e))?;
        trace!(content_len = config_str.len(), "Read config file content.");

        debug!("Parsing TOML configuration.");
        let mut config: BidirectionalAgentConfig = toml::from_str(&config_str)
            .map_err(|e| anyhow!("Failed to parse config file: {}", e))?;
        trace!("TOML parsing successful.");

        // Check for environment variable override for API key
        debug!("Checking for CLAUDE_API_KEY environment variable override.");
        if config.llm.claude_api_key.is_none() {
            if let Ok(env_key) = std::env::var("CLAUDE_API_KEY") {
                info!("Using Claude API key from environment variable.");
                config.llm.claude_api_key = Some(env_key);
            } else {
                debug!("CLAUDE_API_KEY environment variable not found.");
            }
        } else {
            debug!("Claude API key already set in config file.");
        }

        debug!("Configuration loaded successfully from file.");
        Ok(config)
    }
}

// REMOVED unused TaskSendParamsInput struct
// REMOVED unused extract_base_url_from_client function
// REMOVED unused client_url_hack function

/// Main entry point
#[tokio::main]
pub async fn main() -> Result<()> {
    // --- Early setup: Parse args, load config ---
    eprintln!("[PRE-LOG] Starting agent setup...");
    let args: Vec<String> = std::env::args().collect();
    eprintln!("[PRE-LOG] Args: {:?}", args);
    let mut config = BidirectionalAgentConfig::default();
    config.mode.repl = true; // Default to REPL
    eprintln!("[PRE-LOG] Initial default config created.");

    // Check for environment variable API key *before* loading config file
    eprintln!("[PRE-LOG] Checking for CLAUDE_API_KEY env var...");
    if config.llm.claude_api_key.is_none() {
        if let Ok(env_key) = std::env::var("CLAUDE_API_KEY") {
             eprintln!("[PRE-LOG] Found CLAUDE_API_KEY in environment.");
             config.llm.claude_api_key = Some(env_key);
        } else {
             eprintln!("[PRE-LOG] CLAUDE_API_KEY not found in environment.");
        }
    }

    // Process command line arguments to potentially load a config file or override settings
    eprintln!("[PRE-LOG] Processing command line arguments...");
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        eprintln!("[PRE-LOG] Processing arg [{}]: '{}'", i, arg);

        if arg == "--listen" || arg == "-l" {
            eprintln!("[PRE-LOG] Found --listen flag.");
            config.mode.auto_listen = true;
            std::env::set_var("AUTO_LISTEN", "true"); // Keep env var for REPL auto-start check
            // Check for optional port after --listen
            if i + 1 < args.len() && !args[i + 1].starts_with('-') && !args[i + 1].ends_with(".toml") {
                if let Ok(port) = args[i + 1].parse::<u16>() {
                    eprintln!("[PRE-LOG] Overriding server port from arg after --listen: {}", port);
                    config.server.port = port;
                    i += 1; // Consume the port argument
                } else {
                     eprintln!("[PRE-LOG] WARN: Argument after --listen ('{}') is not a valid port. Ignoring.", args[i+1]);
                }
            }
        } else if arg.starts_with("--port=") {
             let port_str = arg.trim_start_matches("--port=");
            if let Ok(port) = port_str.parse::<u16>() {
                eprintln!("[PRE-LOG] Overriding server port from --port= arg: {}", port);
                config.server.port = port;
            } else {
                 eprintln!("[PRE-LOG] WARN: Invalid port format in '{}'. Ignoring.", arg);
            }
        } else if arg.contains(':') && !arg.starts_with(":") && !Path::new(arg).exists() { // Heuristic: host:port vs file path
            eprintln!("[PRE-LOG] Argument '{}' looks like host:port.", arg);
            let parts: Vec<&str> = arg.split(':').collect();
            if parts.len() == 2 {
                let server = parts[0];
                if let Ok(port) = parts[1].parse::<u16>() {
                    let url = format!("http://{}:{}", server, port);
                    eprintln!("[PRE-LOG] Setting client target_url from arg: {}", url);
                    config.client.target_url = Some(url);
                } else {
                    eprintln!("[PRE-LOG] WARN: Invalid port in '{}'. Treating as config file path.", arg);
                    load_config_from_path(&mut config, arg)?; // Fallback to config file
                }
            } else {
                 eprintln!("[PRE-LOG] WARN: Invalid host:port format '{}'. Treating as config file path.", arg);
                 load_config_from_path(&mut config, arg)?; // Fallback to config file
            }
        } else {
            // Assume it's a config file path
            eprintln!("[PRE-LOG] Argument '{}' assumed to be config file path.", arg);
            load_config_from_path(&mut config, arg)?;
        }
        i += 1;
    }
    eprintln!("[PRE-LOG] Finished processing arguments.");

    // --- Initialize Logging ---
    eprintln!("[PRE-LOG] Initializing logging subsystem...");
    // Determine log level (e.g., from RUST_LOG env var, default to info)
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|e| {
            eprintln!("[PRE-LOG] WARN: Failed to parse RUST_LOG ('{}'). Using default 'info'.", e);
            EnvFilter::new("info") // Default to info if RUST_LOG not set or invalid
        });
    eprintln!("[PRE-LOG] Log filter determined: {:?}", env_filter);

    // Setup console logging layer
    eprintln!("[PRE-LOG] Setting up console logging layer.");
    let console_layer = fmt::layer()
        .with_writer(io::stderr) // Log to stderr
        .with_ansi(true) // Enable colors
        .with_target(true) // Show module path
        .with_level(true); // Show log level

    // Setup file logging layer (if configured)
    eprintln!("[PRE-LOG] Setting up file logging layer (if configured).");
    let file_layer = if let Some(log_path_str) = &config.mode.repl_log_file {
        eprintln!("[PRE-LOG] File logging configured to: {}", log_path_str);
        let log_path = PathBuf::from(log_path_str);

        // 1. Validate path and create directory, resulting in Option<PathBuf>
        let maybe_valid_path = if let Some(parent_dir) = log_path.parent() {
            eprintln!("[PRE-LOG] Checking/creating parent directory: {}", parent_dir.display());
            if !parent_dir.exists() {
                match std::fs::create_dir_all(parent_dir) {
                    Ok(_) => {
                        eprintln!("[PRE-LOG] Created log directory: {}", parent_dir.display());
                        Some(log_path) // Dir created, path is valid
                    }
                    Err(e) => {
                        eprintln!("[PRE-LOG] ERROR: Failed to create log directory '{}': {}", parent_dir.display(), e);
                        None // Dir creation failed, path is invalid
                    }
                }
            } else {
                 eprintln!("[PRE-LOG] Log directory already exists: {}", parent_dir.display());
                Some(log_path) // Dir already exists, path is valid
            }
        } else {
             eprintln!("[PRE-LOG] Log path has no parent directory (relative path?). Assuming valid.");
            Some(log_path) // No parent dir (e.g., relative path), assume valid
        };

        // 2. If path is valid, attempt to create the file appender layer
        if let Some(path) = maybe_valid_path {
            eprintln!("[PRE-LOG] Attempting to open/create log file: {}", path.display());
            // Use a non-rolling file appender that opens the file in append mode
            let file = match std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path) {
                Ok(file) => {
                    eprintln!("[PRE-LOG] File appender created successfully for {}", path.display());
                    // Create a more straightforward approach using tracing fields
                    let agent_id_for_log = config.server.agent_id.clone(); // Clone for the layer

                    // Setup a simpler format for the file
                    let format = fmt::format()
                        .with_level(true)
                        .with_target(true) // Keep target for file logs
                        .with_thread_ids(false) // Maybe disable thread IDs for cleaner file logs
                        .with_file(false) // Disable source file location
                        .with_line_number(false) // Disable source line number
                        .with_ansi(false) // No colors in file
                        .compact(); // Use compact format

                   // Simpler file logger layer without custom field mapping for now
                   let file_layer = fmt::layer()
                       .with_writer(file) // Use the opened file
                       .with_ansi(false) // No colors in file
                       .event_format(format) // Use the compact format
                       .with_span_events(FmtSpan::NONE) // Don't include span events like enter/exit
                       .with_target(true) // Keep target (module path)
                       .with_level(true); // Keep log level

                   Some(file_layer.boxed()) // Box the layer
                },
                Err(e) => {
                    eprintln!("[PRE-LOG] ERROR: Failed to open log file '{}': {}", path.display(), e);
                    None
                }
            };

            file
        } else {
            eprintln!("[PRE-LOG] Log file path was invalid, file logging disabled.");
            None // Path was invalid (directory creation failed)
        }
    } else {
        eprintln!("[PRE-LOG] File logging not configured.");
        None // No log file path configured
    };


    // Combine layers and initialize the global subscriber
    eprintln!("[PRE-LOG] Combining logging layers and initializing subscriber.");
    let subscriber_builder = tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer);

    // Initialize differently based on whether file_layer is Some or None
    if let Some(file_layer) = file_layer {
         eprintln!("[PRE-LOG] Initializing logging with console and file layers.");
         subscriber_builder.with(file_layer).init(); // Pass the unwrapped Box<dyn Layer...>
    } else {
         eprintln!("[PRE-LOG] Initializing logging with console layer only.");
         subscriber_builder.init(); // Initialize without the file layer
    }
    eprintln!("[PRE-LOG] Logging initialization complete.");

    // --- Logging is now fully initialized ---
    info!("✅ Logging initialized successfully."); // Added emoji
    info!("🚀 Starting Bidirectional Agent main function."); // Added emoji

    // Log a prominent line with agent info for better separation in shared logs
    let agent_id = config.server.agent_id.clone();
    let agent_name = config.server.agent_name.clone().unwrap_or_else(|| "Unnamed Agent".to_string());
    info!(agent_id = %agent_id, agent_name = %agent_name, port = %config.server.port, "AGENT_STARTED"); // Keep this prominent

    // Log the final effective configuration *after* logging is initialized
    // Use trace level for potentially sensitive full config dump
    trace!(?config, "Effective configuration loaded.");
    // Log key config items at info level
    info!(
        agent_id = %config.server.agent_id,
        agent_name = %config.server.agent_name.as_deref().unwrap_or("N/A"),
        server_port = %config.server.port,
        bind_address = %config.server.bind_address,
        client_target = %config.client.target_url.as_deref().unwrap_or("None"),
        llm_configured = config.llm.claude_api_key.is_some(),
        tools_enabled = ?config.tools.enabled,
        agent_dir_path = %config.tools.agent_directory_path.as_deref().unwrap_or("None"),
        log_file = %config.mode.repl_log_file.as_deref().unwrap_or("None"),
        "Key configuration values" // Keep this info log
    );


    // Determine final mode (logic remains similar, but logging works now)
    debug!("Determining final execution mode.");
    if config.mode.message.is_none()
        && config.mode.remote_task.is_none()
        && !config.mode.get_agent_card
        && !config.mode.repl
    {
        // No specific mode set in config or args, default to REPL
        if args.len() <= 1 {
             debug!("No arguments provided and no mode set in config. Defaulting to REPL mode."); // Changed to debug
             config.mode.repl = true;
        } else {
             // Args were provided, but didn't set a mode (e.g., just a config file path)
             debug!("Arguments provided but no specific action mode requested. Defaulting to REPL mode."); // Changed to debug
             config.mode.repl = true;
        }
    } else if args.len() <= 1 && !config.mode.repl && config.mode.message.is_none() && config.mode.remote_task.is_none() && !config.mode.get_agent_card {
         // Handles case where only a config file was specified, and it didn't set any mode
         debug!("Config file loaded but no specific mode set. Defaulting to REPL mode."); // Changed to debug
         config.mode.repl = true;
    }
    debug!(?config.mode, "Final execution mode determined.");

    // --- Execute based on mode ---
    if config.mode.repl {
        info!("Starting agent in REPL mode."); // Keep info for mode start
        let mut agent = BidirectionalAgent::new(config.clone())?; // new() logs internally
        agent.run_repl().await // run_repl() logs internally
    }
    // Process a single message
    else if let Some(message) = &config.mode.message {
        info!(message_len = message.len(), "Starting agent to process single message."); // Keep info for mode start
        let mut agent = BidirectionalAgent::new(config.clone())?;
        println!("💬 Processing message: '{}'", message); // Added emoji
        let response = agent.process_message_locally(message).await?; // Logs internally
        println!("Response:\n{}", response); // Keep console output
        info!("Finished processing single message.");
        Ok(())
    }
    // Send task to remote agent
    else if let Some(task_message) = &config.mode.remote_task {
        if config.client.target_url.is_none() {
            error!("Cannot send remote task: No target URL configured.");
            return Err(anyhow!("No target URL configured. Add target_url to the [client] section in config file or provide via argument."));
        }
        info!(message_len = task_message.len(), "Starting agent to send remote task."); // Keep info for mode start
        let mut agent = BidirectionalAgent::new(config.clone())?;
        println!("📤 Sending task to remote agent: '{}'", task_message); // Added emoji
        let task = agent.send_task_to_remote(task_message).await?; // Logs internally
        println!("✅ Task sent successfully!"); // Added emoji
        println!("   Task ID: {}", task.id); // Indented
        println!("   Initial state reported by remote: {:?}", task.status.state); // Indented
        info!("Finished sending remote task."); // Keep info for mode end
        Ok(())
    }
    // Get remote agent card
    else if config.mode.get_agent_card {
        if config.client.target_url.is_none() {
             error!("Cannot get remote card: No target URL configured.");
            return Err(anyhow!("No target URL configured. Add target_url to the [client] section in config file or provide via argument."));
        }
        info!("Starting agent to get remote agent card."); // Keep info for mode start
        let mut agent = BidirectionalAgent::new(config.clone())?;
        println!("🔎 Retrieving agent card from remote agent..."); // Added emoji
        let card = agent.get_remote_agent_card().await?; // Logs internally
        println!("📇 Remote Agent Card:"); // Added emoji
        println!("  Name: {}", card.name);
        println!("  Version: {}", card.version);
        println!("  Description: {}", card.description.as_deref().unwrap_or("None"));
        println!("  URL: {}", card.url);
        // TODO: Print more card details if needed
        println!("  Capabilities: {:?}", card.capabilities);
        println!("  Skills: {}", card.skills.len());
        info!("Finished getting remote agent card."); // Keep info for mode end
        Ok(())
    }
    // Default mode: Run the server (if not REPL and no other specific action)
    else {
        info!("Starting agent in default server mode (no REPL or specific action requested)."); // Keep info for mode start
        let agent = BidirectionalAgent::new(config)?; // Logs internally
        agent.run().await // Logs internally
    }
}

/// Helper function to load config from path, used in main arg parsing.
/// Logs using eprintln because tracing might not be initialized yet.
fn load_config_from_path(config: &mut BidirectionalAgentConfig, config_path: &str) -> Result<()> {
    eprintln!("[PRE-LOG] Attempting to load configuration from path: {}", config_path);
    match BidirectionalAgentConfig::from_file(config_path) { // from_file now uses debug/trace internally if logging is up
        Ok(mut loaded_config) => {
            eprintln!("[PRE-LOG] Config file '{}' loaded successfully. Merging with existing/default config.", config_path);
            // Preserve env var API key if it was set and config file doesn't have one
            if config.llm.claude_api_key.is_some() && loaded_config.llm.claude_api_key.is_none() {
                eprintln!("[PRE-LOG] Preserving Claude API key from environment variable over config file.");
                loaded_config.llm.claude_api_key = config.llm.claude_api_key.clone();
            }
            // Preserve command-line overrides if they were set before loading the file
            // Example: Preserve port if set via --port= or --listen before the config file path
             if config.server.port != default_port() && loaded_config.server.port == default_port() {
                 eprintln!("[PRE-LOG] Preserving server port override ({}) from command line over config file.", config.server.port);
                 loaded_config.server.port = config.server.port;
             }
             // Example: Preserve target_url if set via host:port before the config file path
             if config.client.target_url.is_some() && loaded_config.client.target_url.is_none() {
                 eprintln!("[PRE-LOG] Preserving target URL override ('{}') from command line over config file.", config.client.target_url.as_deref().unwrap_or("N/A"));
                 loaded_config.client.target_url = config.client.target_url.clone();
             }
             // Example: Preserve auto_listen if set via --listen before the config file path
             if config.mode.auto_listen && !loaded_config.mode.auto_listen {
                 eprintln!("[PRE-LOG] Preserving auto-listen override from command line over config file.");
                 loaded_config.mode.auto_listen = true;
             }
             // Preserve REPL log file if set via command line? (Less common, maybe not needed)

            // Preserve the config file path itself
            loaded_config.config_file_path = Some(config_path.to_string());
            eprintln!("[PRE-LOG] Setting config_file_path reference to: {}", config_path);

            *config = loaded_config; // Overwrite existing config with loaded, potentially merged, config
            eprintln!("[PRE-LOG] Successfully loaded and applied configuration from {}", config_path);
        },
        Err(e) => {
            // If a config file was specified but failed to load, it's a fatal error.
            eprintln!("[PRE-LOG] ERROR: Failed to load configuration from '{}': {}", config_path, e);
            eprintln!("[PRE-LOG] Please check the configuration file path and syntax.");
            // Use context to chain the error
            return Err(anyhow!("Configuration file loading failed for path: {}", config_path).context(e));
        }
    }
    Ok(())
}
