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

/// Entry in the agent directory (Used locally for routing decisions)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct AgentDirectoryEntry {
    /// Agent card with capabilities
    card: AgentCard,
    /// Last time this agent was seen
    #[serde(with = "chrono::serde::ts_seconds")]
    last_seen: DateTime<Utc>,
    /// Whether the agent is currently active
    active: bool,
}

/// Simplified Agent Directory (Used locally for routing decisions)
#[derive(Debug, Clone)]
pub struct AgentDirectory {
    agents: Arc<DashMap<String, AgentDirectoryEntry>>,
}

impl AgentDirectory {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
        }
    }

    // Note: This uses the AgentCard from crate::types
    // We need the agent_id separately as AgentCard doesn't have it
    #[instrument(skip(self, card), fields(agent_id = %agent_id, card_name = %card.name))]
    pub fn add_or_update_agent(&self, agent_id: String, card: AgentCard) {
        debug!("Adding or updating agent in local directory.");
        let entry = AgentDirectoryEntry {
            card: card.clone(), // Clone card for logging if needed later
            last_seen: Utc::now(),
            active: true,
        };
        self.agents.insert(agent_id.clone(), entry);
        trace!(agent_id = %agent_id, ?card, "Agent details added/updated.");
    } // Removed extra closing brace here

    #[instrument(skip(self))]
    fn get_agent(&self, agent_id: &str) -> Option<AgentDirectoryEntry> {
        debug!(%agent_id, "Getting agent from local directory.");
        let result = self.agents.get(agent_id).map(|e| e.value().clone());
        trace!(%agent_id, found = result.is_some(), "Agent lookup result.");
        result
    }

    #[instrument(skip(self))]
    fn list_active_agents(&self) -> Vec<AgentDirectoryEntry> {
        debug!("Listing active agents from local directory.");
        let agents: Vec<_> = self.agents
            .iter()
            .filter(|e| e.value().active)
            .map(|e| e.value().clone())
            .collect();
        trace!(count = agents.len(), "Found active agents.");
        agents
    }
    
    /// Get a list of all agents with their cards, including inactive ones
    #[instrument(skip(self))]
    pub fn list_all_agents(&self) -> Vec<(String, AgentCard)> {
        debug!("Listing all agents from local directory.");
        let agents: Vec<_> = self.agents
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().card.clone()))
            .collect();
        trace!(count = agents.len(), "Found total agents.");
        agents
    }
    
    /// Save the current agent directory to a JSON file
    #[instrument(skip(self, path), fields(path = %path.as_ref().display()))]
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        debug!("Saving agent directory to file.");
        // Convert DashMap to a regular HashMap for serialization
        let agents_map: HashMap<String, AgentDirectoryEntry> = self.agents.iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();
        trace!(count = agents_map.len(), "Collected agents for serialization.");
        
        // Serialize to JSON
        let json = serde_json::to_string_pretty(&agents_map)
            .map_err(|e| anyhow!("Failed to serialize agent directory: {}", e))?;
        trace!(json_len = json.len(), "Serialized agent directory to JSON.");
        
        // Create parent directory if it doesn't exist
        if let Some(parent) = path.as_ref().parent() {
            if !parent.exists() {
                trace!("Creating parent directory for agent directory file.");
                fs::create_dir_all(parent)
                    .map_err(|e| anyhow!("Failed to create directory for agent directory file: {}", e))?;
            }
        }
        
        // Write to file
        trace!("Writing agent directory to file.");
        fs::write(path.as_ref(), json)
            .map_err(|e| anyhow!("Failed to write agent directory file: {}", e))?;
        
        debug!("Successfully saved agent directory.");
        Ok(())
    }
    
    /// Load agent directory from a JSON file
    #[instrument(skip(self, path), fields(path = %path.as_ref().display()))]
    pub fn load_from_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        debug!("Attempting to load agent directory from file.");
        // Skip if file doesn't exist (start with empty directory)
        if !path.as_ref().exists() {
            info!("Agent directory file not found, starting with empty directory.");
            return Ok(());
        }
        
        // Read file content
        trace!("Reading agent directory file content.");
        let file_content = fs::read_to_string(path.as_ref())
            .map_err(|e| anyhow!("Failed to read agent directory file: {}", e))?;
        trace!(content_len = file_content.len(), "Read agent directory file content.");
        
        // Deserialize from JSON
        trace!("Deserializing agent directory from JSON.");
        let loaded_agents: HashMap<String, AgentDirectoryEntry> = serde_json::from_str(&file_content)
            .map_err(|e| anyhow!("Failed to deserialize agent directory: {}", e))?;
        trace!(count = loaded_agents.len(), "Deserialized agents.");
        
        // Update the directory
        debug!("Updating agent directory with loaded data.");
        for (id, entry) in loaded_agents {
            trace!(agent_id = %id, "Loading agent into directory.");
            self.agents.insert(id, entry);
        }
        
        debug!("Successfully loaded agent directory.");
        Ok(())
    }
    
}


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
    directory: Arc<AgentDirectory>, // Use the local directory for routing logic
    enabled_tools: Arc<Vec<String>>, // <-- new: Store enabled tool names
}

impl BidirectionalTaskRouter {
     pub fn new(
         llm: Arc<dyn LlmClient>,
         directory: Arc<AgentDirectory>,
         enabled_tools: Arc<Vec<String>>, // <-- new parameter
     ) -> Self {
        // Ensure "echo" is always considered enabled internally for fallback, even if not in config
        let mut tools = enabled_tools.as_ref().clone();
        if !tools.contains(&"echo".to_string()) {
            tools.push("echo".to_string());
        }
        Self {
            llm,
            directory,
            enabled_tools: Arc::new(tools), // Store the potentially modified list
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
 
        // Get the list of available agents from the local directory
        debug!("Fetching available agents for routing prompt.");
        let available_agents = self.directory.list_active_agents();
        trace!(count = available_agents.len(), "Found available agents.");
        let agent_descriptions = available_agents.iter()
            .map(|a| {
                trace!(agent_name = %a.card.name, "Formatting agent description for prompt.");
                // Construct capabilities string manually
                let mut caps = Vec::new();
                if a.card.capabilities.push_notifications { caps.push("pushNotifications"); }
                if a.card.capabilities.state_transition_history { caps.push("stateTransitionHistory"); }
                if a.card.capabilities.streaming { caps.push("streaming"); }
                // Add other capabilities fields if they exist in AgentCapabilities struct

                // Find the agent_id (key) associated with this card's URL
                let agent_id = self.directory.agents.iter()
                    .find(|entry| entry.value().card.url == a.card.url)
                    .map(|e| e.key().clone())
                    .unwrap_or_else(|| "unknown-id".to_string()); // Fallback if not found

                format!("ID: {}\nName: {}\nDescription: {}\nCapabilities: {}",
                    agent_id,
                    a.card.name.as_str(), // name is String
                    a.card.description.as_ref().unwrap_or(&"".to_string()),
                    caps.join(", "))
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        trace!(agent_descriptions = %agent_descriptions, "Formatted agent descriptions for prompt.");

        // Build a prompt for the LLM to decide routing
        debug!("Building routing prompt for LLM.");
        let routing_prompt = format!(r#"
You need to decide whether to handle a task locally, delegate it to another agent, or reject it entirely.

AVAILABLE AGENTS:
{}

TASK:
{}

Please analyze the task and decide whether to:
1. Handle it locally (respond with "LOCAL")
2. Delegate to a specific agent (respond with "REMOTE: [agent-id]")
3. Reject the task (respond with "REJECT: [reason]")
   - Reject tasks that are inappropriate, harmful, impossible, or outside your capabilities
   - Provide a brief explanation for why you're rejecting the task

Your response should be exactly one of those formats, with no additional text.
"#, agent_descriptions, task_text);
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
        if decision.starts_with("LOCAL") { // Check prefix
            info!("LLM decided LOCAL execution. Proceeding to tool selection.");
            // --- LLM Tool Selection & Parameter Extraction Logic ---
            // Get descriptions of locally enabled tools again for the choice prompt
            debug!("Fetching descriptions of locally enabled tools for tool choice prompt.");
            let local_tool_descriptions_for_choice = self.enabled_tools.iter()
                .filter_map(|tool_name| {
                    // TODO: Refactor to pass ToolExecutor or tool descriptions (same as above)
                    let description = match tool_name.as_str() {
                        "llm" => "General purpose LLM request. Good for questions, generation, analysis if no specific tool fits.",
                        "summarize" => "Summarizes the input text.",
                        "list_agents" => "Lists known agents. Use only if the task is explicitly about listing agents.",
                        "remember_agent" => "Stores information about another agent given its URL. Expects JSON parameter: {\"agent_base_url\": \"http://...\"}", // Add param info
                        "echo" => "Simple echo tool for testing. Use only if the task is explicitly to echo. Expects JSON parameter: {\"text\": \"...\"}", // Add param info
                        _ => "A custom tool.",
                    };
                    Some(format!("- {}: {}", tool_name, description))
                })
                .collect::<Vec<_>>()
                .join("\n");
            trace!(local_tool_descriptions = %local_tool_descriptions_for_choice, "Formatted local tool descriptions for choice.");

            let tool_choice_prompt = format!(
r#"You have decided to handle the following TASK locally:
{}

Choose the SINGLE most appropriate tool from the list below to execute this task. Consider the tool descriptions carefully.

AVAILABLE LOCAL TOOLS:
{}

Respond ONLY with the exact name of the chosen tool (e.g., 'llm', 'summarize', 'echo')."#,
                task_text, // Include the task text again for context
                local_tool_descriptions_for_choice
            );
            trace!(tool_choice_prompt = %tool_choice_prompt, "Constructed tool choice prompt.");

            info!("Asking LLM to choose the specific local tool.");
            let tool_choice_result = self.llm.complete(&tool_choice_prompt).await;

            let chosen_tool_name = match tool_choice_result {
                 Ok(tool_name_raw) => {
                    let tool_name = tool_name_raw.trim().to_string();
                    trace!(raw_tool_choice = %tool_name_raw, trimmed_tool_choice = %tool_name, "Received tool choice from LLM.");
                    // Validate the LLM's choice against the enabled tools
                    if self.enabled_tools.contains(&tool_name) {
                         info!(tool_name = %tool_name, "LLM chose valid tool.");
                         tool_name // Use the valid tool chosen by LLM
                    } else {
                         warn!(chosen_tool = %tool_name, enabled_tools = ?self.enabled_tools, "LLM chose an unknown/disabled tool. Falling back to 'llm' tool.");
                         "llm".to_string() // Fallback to llm tool if choice is invalid
                    }
                 },
                 Err(e) => {
                     warn!(error = %e, "LLM failed to choose a tool. Falling back to 'llm' tool.");
                     "llm".to_string() // Fallback to llm tool on error
                 }
            };
            trace!(chosen_tool_name = %chosen_tool_name, "Final tool name selected.");

            // --- Parameter Extraction ---
            info!(tool_name = %chosen_tool_name, "Asking LLM to extract parameters for the chosen tool.");
            let param_extraction_prompt = format!(
r#"Based on the original TASK and the chosen TOOL, extract the necessary parameters as a valid JSON object.

ORIGINAL TASK:
{}

CHOSEN TOOL: {}
(Refer to tool descriptions provided previously if needed, especially for expected parameter names like 'agent_base_url' or 'text')

Respond ONLY with the JSON object containing the parameters. For example:
- For 'remember_agent': {{"agent_base_url": "http://..."}}
- For 'echo' or 'llm': {{"text": "..."}}
- For 'list_agents': {{}} (or {{"format": "detailed"}} if specified in task)

If no specific parameters are needed or mentioned in the task, respond with an empty JSON object: {{}}."#,
                task_text,
                chosen_tool_name
            );
            trace!(param_extraction_prompt = %param_extraction_prompt, "Constructed parameter extraction prompt.");

            let params_result = self.llm.complete(&param_extraction_prompt).await;
            let extracted_params: Value = match params_result {
                Ok(params_str_raw) => {
                    let params_str = params_str_raw.trim();
                    trace!(raw_params_str = %params_str_raw, trimmed_params_str = %params_str, "Received raw parameters string from LLM.");
                    match serde_json::from_str(params_str) {
                        Ok(json_value) => {
                            info!(?json_value, "Successfully parsed JSON parameters extracted by LLM.");
                            json_value
                        },
                        Err(e) => {
                            // Escape the braces in the format string
                            warn!(error = %e, params_str = %params_str, "LLM returned invalid JSON for parameters. Falling back to {{\"text\": \"original_task_text\"}}.");
                            // Fallback: Use original text if JSON parsing fails
                            json!({"text": task_text})
                        }
                    }
                },
                Err(e) => {
                    // Escape the braces in the format string
                    warn!(error = %e, "LLM failed to extract parameters. Falling back to {{\"text\": \"original_task_text\"}}.");
                    // Fallback: Use original text if LLM fails
                    json!({"text": task_text})
                }
            };
            trace!(?extracted_params, "Final parameters for tool execution.");

            info!(tool_name = %chosen_tool_name, "Final routing decision: Local execution.");
            Ok(RoutingDecision::Local { tool_name: chosen_tool_name, params: extracted_params })
            // --- End LLM Tool Selection & Parameter Extraction Logic ---

        } else if decision.starts_with("REMOTE: ") {
            let agent_id = decision.strip_prefix("REMOTE: ").unwrap().trim().to_string();
            info!(remote_agent_id = %agent_id, "LLM decided REMOTE execution.");

            // Verify the agent exists in the local directory
            trace!(remote_agent_id = %agent_id, "Verifying remote agent existence in directory.");
            if self.directory.get_agent(&agent_id).is_none() {
                 warn!(remote_agent_id = %agent_id, "LLM decided to delegate to unknown agent, falling back to local execution with 'llm' tool.");
                 // Fall back to local if agent not found, using llm tool
                 // Use tool_name and add default params
                 Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({}) })
            } else {
                 info!(remote_agent_id = %agent_id, "Routing decision: Remote delegation confirmed.");
                 Ok(RoutingDecision::Remote { agent_id })
            }
        } else if decision.starts_with("REJECT: ") {
            let reason = decision.strip_prefix("REJECT: ").unwrap().trim().to_string();
            info!(reason = %reason, "LLM decided to REJECT the task.");
            Ok(RoutingDecision::Reject { reason })
        } else {
            warn!(llm_decision = %decision, "LLM routing decision was unclear, falling back to local execution with 'llm' tool.");
            // Default to local llm tool if the decision isn't clear
            // Use tool_name and add default params
            Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({}) })
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
        // For now, always route follow-ups locally as a simple default.
        // A real implementation would likely involve the LLM again.
        info!("Defaulting follow-up routing to LOCAL execution.");
        // Use tool_name and provide default empty params for follow-up
        // Use "echo" as the default tool for follow-ups for now
        Ok(RoutingDecision::Local { tool_name: "echo".to_string(), params: json!({}) })
    }
 
    // Implement the decide method required by LlmTaskRouterTrait
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
    agent_directory: Arc<AgentDirectory>, // Local directory for routing decisions
    agent_directory_path: Option<PathBuf>, // Path to persist agent directory
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
        info!("Creating new BidirectionalAgent instance.");
        trace!(?config, "Agent configuration provided.");
        // Create the agent directory (local helper)
        debug!("Initializing AgentDirectory.");
        let agent_directory = Arc::new(AgentDirectory::new());
        // Load agent directory from file if configured
        if let Some(dir_path_str) = &config.tools.agent_directory_path {
            let dir_path = PathBuf::from(dir_path_str);
            debug!(path = %dir_path.display(), "Attempting to load agent directory from configured path.");
            if let Err(e) = agent_directory.load_from_file(&dir_path) {
                // Log warning but continue, agent will start with empty directory
                warn!(error = %e, path = %dir_path.display(), "Failed to load agent directory from file. Continuing with empty directory.");
            } else {
                info!(path = %dir_path.display(), "Successfully loaded agent directory from file.");
            }
        } else {
            debug!("No agent directory path configured. Starting with empty directory.");
        }

        // Create the LLM client (local helper)
        debug!("Initializing LLM client.");
        let llm: Arc<dyn LlmClient> = if let Some(api_key) = &config.llm.claude_api_key {
            info!("Using Claude API key from configuration/environment.");
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
        
        let bidirectional_tool_executor = Arc::new(ToolExecutor::with_enabled_tools(
            &config.tools.enabled,            // Pass slice of enabled tool names
            Some(llm.clone()),                // Pass LLM client (as Option)
            Some(agent_directory.clone()),    // Pass agent directory (as Option)
            Some(agent_registry.clone()),     // <-- Pass canonical AgentRegistry (as Option)
            Some(known_servers.clone()),      // Pass known_servers map for synchronization
        ));
        trace!("ToolExecutor created.");

        // Create the task router, passing the enabled tools list
        let bidirectional_task_router: Arc<dyn LlmTaskRouterTrait> =
            Arc::new(BidirectionalTaskRouter::new(
                llm.clone(),
                agent_directory.clone(),
                enabled_tools.clone(), // Pass Arc<Vec<String>>
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
        info!("TaskService created.");

        // Create the streaming service
        debug!("Initializing StreamingService.");
        let streaming_service = Arc::new(StreamingService::new(task_repository.clone()));
        info!("StreamingService created.");

        // Create the notification service (pass repository)
        debug!("Initializing NotificationService.");
        let notification_service = Arc::new(NotificationService::new(task_repository.clone()));
        info!("NotificationService created.");

        // Initialize an A2A client if target URL is provided
        debug!("Checking for client target URL.");
        let client = if let Some(target_url) = &config.client.target_url {
            info!(target_url = %target_url, "Initializing A2A client for target URL.");
            Some(A2aClient::new(target_url))
        } else {
            info!("No target URL provided, A2A client not initialized.");
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
            agent_directory: agent_directory.clone(), // Keep local directory for routing logic
            agent_directory_path: agent_directory_path.clone(), // Store path for saving
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
        
        // Set up periodic saving of agent directory if path is configured
        if let Some(dir_path) = agent_directory_path.clone() { // Clone path for the task
            let dir_clone = agent_directory.clone();
            let agent_id_clone = agent.agent_id.clone(); // Clone agent_id for tracing span
            let save_interval_secs = 300; // 5 minutes

            info!(path = %dir_path.display(), interval_secs = save_interval_secs, "Spawning background task for periodic agent directory saving.");
            // Spawn a background task for saving
            tokio::spawn(async move {
                // Create a tracing span for the background task
                let span = tracing::info_span!(
                    "bg_save_dir",
                    agent_id = %agent_id_clone,
                    save_path = %dir_path.display()
                );
                // Enter the span
                let _enter = span.enter();
                debug!("Background directory save task started.");

                loop {
                    // Save every 5 minutes
                    trace!(seconds = save_interval_secs, "Sleeping before next directory save.");
                    tokio::time::sleep(std::time::Duration::from_secs(save_interval_secs)).await;

                    debug!("Attempting periodic save of agent directory.");
                    if let Err(e) = dir_clone.save_to_file(&dir_path) {
                        warn!(error = %e, "Periodic save of agent directory failed.");
                    } else {
                        debug!("Successfully saved agent directory periodically.");
                    }
                }
            });
        } else {
            debug!("Periodic agent directory saving is disabled (no path configured).");
        }

        // Logging initialization happens in main() now.
        // We can log that the agent struct itself is constructed.
        info!(agent_id = %agent.agent_id, bind_address = %agent.bind_address, port = %agent.port, "BidirectionalAgent struct created successfully.");

        Ok(agent)
    }

    /// Create a session on demand so remote tasks can be grouped
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    async fn ensure_session(&mut self) {
        trace!("Ensuring session exists.");
        if self.current_session_id.is_none() {
            debug!("No current session ID found.");
            let session_id = self.create_new_session(); // existing helper logs internally
            info!(session_id = %session_id, "Created new session on demand.");
        } else {
            trace!(session_id = %self.current_session_id.as_deref().unwrap_or("None"), "Session already exists.");
        }
    }


    /// Process a message locally (e.g., from REPL input that isn't a command)
    #[instrument(skip(self, message_text), fields(agent_id = %self.agent_id, message_len = message_text.len()))]
    pub async fn process_message_locally(&mut self, message_text: &str) -> Result<String> {
        info!("Processing message locally.");
        trace!(message = %message_text, "Message content.");

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
                                info!(task_id = %last_task_id, "Found task in InputRequired state. Continuing this task.");
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
            info!(task_id = %id, "Continuing existing task.");
            id
        } else {
            let new_id = Uuid::new_v4().to_string();
            info!(task_id = %new_id, "Creating new task ID.");
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

        info!(task_id = %task_id, "Calling task_service.process_task.");
        // Use task_service to process the task
        // Instrument the call to task_service
        let task_result = async {
            self.task_service.process_task(params).await
        }.instrument(tracing::info_span!("task_service_process", task_id = %task_id_clone)).await;

        let task = match task_result {
            Ok(t) => {
                info!(task_id = %t.id, status = ?t.status.state, "Task processing completed by task_service.");
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
            info!(task_id = %task.id, "Task requires more input. Appending message to response.");
            response.push_str("\n\n[The agent needs more information. Your next message will continue this task.]");
        }

        info!(task_id = %task.id, "Local message processing complete. Returning response.");
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
        info!("Entering REPL mode.");
        // Print the initial help message
        self.print_repl_help();

        // --- Spawn Background Task for Initial Connection ---
        debug!("Checking if background connection task should be spawned.");
        if let Some(initial_url) = self.client_url() {
            info!(url = %initial_url, "Spawning background task for initial connection attempt.");
            println!("Configured target URL: {}. Attempting connection in background...", initial_url);
            // Clone necessary data for the background task
            // Clone necessary data for the background task's tracing span
            let bg_agent_id = self.agent_id.clone();
            let bg_initial_url = initial_url.clone();
            let agent_directory = self.agent_directory.clone(); // Local directory for routing
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

                info!("Background connection task started.");

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
                            info!(remote_agent_name = %remote_agent_name, "Background connection successful.");
                            println!("✅ Background connection successful to {} ({})", remote_agent_name, initial_url); // Print to console as well

                            // Update shared state
                            debug!(url = %initial_url, name = %remote_agent_name, "Updating known servers map.");
                            known_servers.insert(initial_url.clone(), remote_agent_name.clone());
                            debug!(name = %remote_agent_name, "Updating agent directory.");
                            agent_directory.add_or_update_agent(remote_agent_name.clone(), card.clone()); // Update local directory
                            info!(remote_agent_name = %remote_agent_name, "Added/updated agent in local directory.");

                            // Also update the canonical AgentRegistry
                            debug!(url = %initial_url, "Attempting to update canonical AgentRegistry.");
                            match agent_registry.discover(&initial_url).await {
                                Ok(agent_id) => {
                                    info!(url = %initial_url, agent_id = %agent_id, "Successfully updated canonical AgentRegistry.");
                                    // No need to update known_servers here as it's already done above
                                },
                                Err(reg_err) => {
                                    // Log warning if updating canonical registry fails, but don't fail the connection
                                    warn!(error = %reg_err, url = %initial_url, "Failed to update canonical AgentRegistry after successful connection.");
                                }
                            }

                            connected = true;
                            break; // Exit retry loop on success
                        }
                        Err(e) => {
                            warn!(attempt, max_retries, error = %e, "Background connection attempt failed.");
                            if attempt < max_retries {
                                info!(delay_seconds = %retry_delay.as_secs(), "Retrying background connection...");
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
                    info!("Background connection task finished successfully.");
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
            info!(port = %self.port, bind_address = %self.bind_address, "Auto-starting server.");
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
                info!("Auto-started server task running.");

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
                        info!("Server task successfully started.");
                        // Send success status back to REPL
                        trace!("Sending success status back to REPL via channel.");
                        let _ = tx.send(Ok(())); // Ignore error if REPL already timed out

                        // Wait for the server to complete or be cancelled
                        info!("Waiting for server task to complete or be cancelled.");
                        match handle.await {
                            Ok(()) => info!("Server task shut down gracefully."),
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
                    info!(bind_address = %self.bind_address, port = %self.port, "Auto-started server successfully confirmed by REPL.");
                    println!("✅ Server started on http://{}:{}", self.bind_address, self.port);
                    println!("The server will run until you exit the REPL or send :stop");
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
                // Save agent directory before exiting
                if let Some(dir_path) = &self.agent_directory_path {
                    debug!("Saving agent directory on EOF shutdown.");
                    if let Err(e) = self.agent_directory.save_to_file(dir_path) {
                        warn!(error = %e, "Failed to save agent directory on EOF shutdown.");
                    } else {
                        info!("Successfully saved agent directory on EOF shutdown.");
                    }
                }

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
            info!(user_input = %input_trimmed, "REPL input received.");

            if input_trimmed.is_empty() {
                trace!("Empty input received, continuing loop.");
                continue;
            }

            if input_trimmed.starts_with(":") {
                info!(command = %input_trimmed, "Processing REPL command.");
                
                // Parse command to extract command name and arguments
                let parts: Vec<&str> = input_trimmed.splitn(2, ' ').collect();
                let command = parts[0].trim_start_matches(':');
                
                // Handle special commands using input_trimmed and parsed command
                if command == "help" || input_trimmed == ":help" {
                    self.print_repl_help(); // Logs internally
                } else if command == "quit" || input_trimmed == ":quit" {
                    info!("User initiated :quit command.");
                    println!("Exiting REPL. Goodbye!");

                    // Save agent directory before exiting if path is configured
                    if let Some(dir_path) = &self.agent_directory_path {
                         debug!("Saving agent directory on :quit command.");
                        if let Err(e) = self.agent_directory.save_to_file(dir_path) {
                            warn!(error = %e, "Failed to save agent directory on shutdown.");
                        } else {
                            info!("Successfully saved agent directory on shutdown.");
                        }
                    }

                    // Shutdown server if running
                    if let Some(token) = server_shutdown_token.take() {
                        info!("Shutting down server via :quit command.");
                        println!("Shutting down server...");
                        token.cancel();
                        trace!("Server cancellation token cancelled.");
                    } else {
                        trace!("No server running or token available to cancel on :quit.");
                    }
                    break; // Exit loop
                } else if input_trimmed == ":card" {
                    debug!("Processing :card command.");
                    let card = self.create_agent_card(); // Logs internally
                    info!(agent_name = %card.name, "Displaying agent card.");
                    println!("\n📇 Agent Card:");
                    println!("  Name: {}", card.name);
                    println!("  Description: {}", card.description.as_deref().unwrap_or("None"));
                    println!("  URL: {}", card.url);
                    println!("  Version: {}", card.version);
                    println!("  Capabilities:");
                    println!("    - Streaming: {}", card.capabilities.streaming);
                    println!("    - Push Notifications: {}", card.capabilities.push_notifications);
                    println!("    - State Transition History: {}", card.capabilities.state_transition_history);
                    println!("  Input Modes: {}", card.default_input_modes.join(", "));
                    println!("  Output Modes: {}", card.default_output_modes.join(", "));
                    println!("");
                } else if input_trimmed == ":servers" {
                    info!("Listing known servers.");
                    // List known servers from the shared map
                    if self.known_servers.is_empty() {
                        info!("No known servers found.");
                        println!("No known servers. Connect to a server or wait for background connection attempt.");
                    } else {
                        info!(count = %self.known_servers.len(), "Displaying known servers.");
                        println!("\n📋 Known Servers:");
                        // Collect servers into a Vec and sort for consistent display order
                        let mut server_list: Vec<(String, String)> = self.known_servers.iter()
                            .map(|entry| (entry.value().clone(), entry.key().clone())) // (Name, URL)
                            .collect();
                        server_list.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by name

                        for (i, (name, url)) in server_list.iter().enumerate() {
                            // Mark the currently connected server with an asterisk
                            let marker = if Some(url) == self.client_url().as_ref() { "*" } else { " " };
                            println!("  {}{}: {} - {}", marker, i + 1, name, url);
                        }
                        println!("\nUse :connect N to connect to a server by number");
                        println!("");
                    }
                } else if input_trimmed == ":disconnect" {
                    info!("Processing :disconnect command.");
                    // Disconnect from current server
                    if self.client.is_some() {
                        let url = self.client_url().unwrap_or_else(|| "unknown".to_string());
                        self.client = None;
                        info!(disconnected_from = %url, "Disconnected from remote agent.");
                        println!("🔌 Disconnected from {}", url);
                    } else {
                        info!("Attempted disconnect but not connected.");
                        println!("Not connected to any server");
                    }
                } else if input_trimmed.starts_with(":connect ") {
                    let target = input_trimmed.trim_start_matches(":connect ").trim();
                    info!(target = %target, "Processing :connect command.");

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
                            println!("🔗 Connecting to {}: {}", name, url_clone);

                            // Attempt to get card after connecting (spawn to avoid blocking REPL)
                            let mut agent_client = self.client.as_mut().unwrap().clone(); // Clone client for task
                            let agent_directory_clone = self.agent_directory.clone();
                            let agent_id_clone = self.agent_id.clone(); // For tracing span

                            tokio::spawn(async move {
                                let span = tracing::info_span!("connect_get_card", agent_id = %agent_id_clone, remote_url = %url_clone);
                                let _enter = span.enter();
                                match agent_client.get_agent_card().await {
                                    Ok(card) => {
                                        info!(remote_agent_name = %card.name, "Successfully got card after connecting.");
                                        agent_directory_clone.add_or_update_agent(card.name.clone(), card.clone());
                                        info!(remote_agent_name = %card.name, "Added/updated agent in directory.");
                                        println!("📇 Remote agent verified: {}", card.name);
                                    }
                                    Err(e) => {
                                        warn!(error = %e, "Connected, but failed to get card.");
                                        println!("⚠️ Could not retrieve agent card after connecting: {}", e);
                                    }
                                }
                            });
                        } else {
                            error!(index = %server_idx, "Invalid server number provided.");
                            println!("❌ Error: Invalid server number. Use :servers to see available servers.");
                        }
                    } else {
                        // Treat as URL
                        let target_url = target.to_string(); // Use target_url consistently
                        if target_url.is_empty() {
                            error!("No URL provided for :connect command.");
                            println!("❌ Error: No URL provided. Use :connect URL");
                            continue;
                        }

                        info!(url = %target_url, "Attempting connection to URL.");
                        // Create a new client with the provided URL, but don't assume connection will succeed
                        let mut client = A2aClient::new(&target_url);

                        // Try to get the agent card to verify connection
                        match client.get_agent_card().await {
                            Ok(card) => {
                                let remote_agent_name = card.name.clone();
                                info!(remote_agent_name = %remote_agent_name, url = %target_url, "Successfully connected to agent.");
                                println!("✅ Successfully connected to agent: {}", remote_agent_name);

                                // Add/Update known servers map
                                self.known_servers.insert(target_url.clone(), remote_agent_name.clone());

                                // Store the client since connection was successful
                                self.client = Some(client);

                                // IMPORTANT: Add the agent to the agent directory for routing
                                self.agent_directory.add_or_update_agent(remote_agent_name.clone(), card.clone());
                                info!(remote_agent_name = %remote_agent_name, "Added agent to local directory for routing.");
                                println!("🔄 Added agent to directory for task routing");

                            },
                            Err(e) => {
                                error!(url = %target_url, error = %e, "Failed to connect to agent via URL.");
                                println!("❌ Failed to connect to agent at {}: {}", target_url, e);
                                println!("Please check that the server is running and the URL is correct.");

                                // Ask to add to known servers even if connection failed
                                if !self.known_servers.contains_key(&target_url) {
                                    println!("Connection failed. Add this URL to the known servers list anyway? (y/n)");
                                    let mut answer = String::new();
                                    let stdin_temp = io::stdin();
                                    let mut reader_temp = stdin_temp.lock();
                                    reader_temp.read_line(&mut answer).unwrap_or_default();

                                    if answer.trim().to_lowercase() == "y" {
                                        let unknown_name = "Unknown Agent".to_string();
                                        self.known_servers.insert(target_url.clone(), unknown_name.clone());
                                        info!(url = %target_url, "Added URL to known servers as 'Unknown Agent'.");
                                        println!("Added URL to known servers list as '{}'.", unknown_name);
                                    }
                                } else {
                                     info!(url = %target_url, "Connection failed, but URL is already in the known servers list.");
                                     println!("Connection failed, but URL is already in the known servers list.");
                                }
                            }
                        };
                    }
                } else if input_trimmed.starts_with(":listen ") {
                    let port_str = input_trimmed.trim_start_matches(":listen ").trim();
                    info!(port_str = %port_str, "Processing :listen command.");

                    // Check if already running
                    if server_running {
                        warn!("Attempted to listen while server already running.");
                        println!("⚠️ Server already running. Stop it first with :stop");
                        continue;
                    }

                    // Parse port
                    match port_str.parse::<u16>() {
                        Ok(port) => {
                            // Update the port in the agent
                            self.port = port;

                            // Create a cancellation token
                            let token = CancellationToken::new();
                            server_shutdown_token = Some(token.clone());

                            // Start server in background task
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
                                        info!(%port, "Server task started.");
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
                                    server_running = true;
                                    info!(%port, "Server started successfully via REPL command.");
                                    println!("✅ Server started on http://{}:{}", self.bind_address, port);
                                    println!("The server will run until you exit the REPL or send :stop");
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
                                }
                            }
                        },
                        Err(parse_err) => {
                            error!(input = %port_str, error = %parse_err, "Invalid port format for :listen command.");
                            println!("❌ Error: Invalid port number. Please provide a valid port.");
                        }
                    }
                } else if input_trimmed == ":stop" {
                    info!("Processing :stop command.");
                    // Stop the server if running
                    if let Some(token) = server_shutdown_token.take() {
                        info!("Stopping server.");
                        println!("Shutting down server...");
                        token.cancel();
                        server_running = false; // Assume stop is successful for REPL state
                        println!("✅ Server stop signal sent.");
                    } else {
                        warn!("Attempted to stop server, but none was running or token missing.");
                        println!("⚠️ No server currently running or already stopped.");
                    }
                } else if input_trimmed.starts_with(":remote ") {
                    let message = input_trimmed.trim_start_matches(":remote ").trim();
                    info!(message = %message, "Processing :remote command.");
                    if message.is_empty() {
                        error!("No message provided for :remote command.");
                        println!("❌ Error: No message provided. Use :remote MESSAGE");
                        continue;
                    }

                    // Check if we're connected to a remote agent
                    if self.client.is_none() {
                        error!("Cannot send remote task: Not connected to a remote agent.");
                        println!("❌ Error: Not connected to a remote agent. Use :connect URL first.");
                        continue;
                    }

                    // Send task to remote agent
                    info!(message = %message, "Sending task to remote agent.");
                    println!("📤 Sending task to remote agent: '{}'", message);
                    match self.send_task_to_remote(message).await { // send_task_to_remote logs internally now
                        Ok(task) => {
                            info!(task_id = %task.id, status = ?task.status.state, "Remote task sent successfully.");
                            println!("✅ Task sent successfully!");
                            println!("Task ID: {}", task.id);
                            println!("Initial state reported by remote: {:?}", task.status.state);

                            // If we have a completed task with history, show the response
                            if task.status.state == TaskState::Completed {
                                let response = self.extract_text_from_task(&task); // Use helper
                                if response != "No response text available." {
                                     info!(task_id = %task.id, "Displaying immediate response from completed remote task.");
                                     println!("\n📥 Response from remote agent:");
                                     println!("{}", response);
                                } else {
                                     info!(task_id = %task.id, "Remote task completed but no text response found in status/history.");
                                }
                            }
                        },
                        Err(e) => {
                            error!(error = %e, "Error sending remote task.");
                            println!("❌ Error sending task: {}", e);
                        }
                    }
                } else if input_trimmed == ":session new" {
                    let session_id = self.create_new_session(); // create_new_session logs internally now
                    println!("✅ Created new session: {}", session_id);
                } else if input_trimmed == ":session show" {
                    if let Some(session_id) = &self.current_session_id {
                        info!(session_id = %session_id, "Displaying current session ID.");
                        println!("🔍 Current session: {}", session_id);
                    } else {
                        info!("No active session.");
                        println!("⚠️ No active session. Use :session new to create one.");
                    }
                } else if input_trimmed == ":history" {
                    if let Some(session_id) = &self.current_session_id {
                        info!(session_id = %session_id, "Fetching history for current session.");
                        let tasks = self.get_current_session_tasks().await?; // get_current_session_tasks logs internally
                        if tasks.is_empty() {
                            info!(session_id = %session_id, "No tasks found in current session history.");
                            println!("📭 No messages in current session.");
                        } else {
                            info!(session_id = %session_id, task_count = %tasks.len(), "Displaying session history.");
                            println!("\n📝 Session History (Tasks):");
                            for task in tasks.iter() {
                                println!("--- Task ID: {} (Status: {:?}) ---", task.id, task.status.state);
                                if let Some(history) = &task.history {
                                    for message in history {
                                        let role_icon = match message.role {
                                            Role::User => "👤",
                                            Role::Agent => "🤖",
                                            // Role::System => "⚙️", // If you add System role
                                            // Role::Tool => "🛠️", // If you add Tool role
                                            _ => "➡️", // Fallback
                                        };
                                        let text = message.parts.iter()
                                            .filter_map(|p| match p {
                                                Part::TextPart(tp) => Some(tp.text.as_str()),
                                                Part::FilePart(_) => Some("[File Part]"),
                                                Part::DataPart(_) => Some("[Data Part]"),
                                                // _ => None, // Handle potential future Part variants
                                            })
                                            .collect::<Vec<_>>()
                                            .join(" "); // Join parts with space

                                        // Truncate long messages for display
                                        let display_text = if text.len() > 100 {
                                            format!("{}...", &text[..97])
                                        } else {
                                            text.to_string()
                                        };
                                        println!("  {} {}: {}", role_icon, message.role, display_text);
                                    }
                                } else {
                                    println!("  (No message history available for this task)");
                                }
                                println!("--------------------------------------");
                            }
                        }
                    } else {
                        info!("Cannot show history: No active session.");
                        println!("⚠️ No active session. Use :session new to create one.");
                    }
                } else if input_trimmed == ":tasks" {
                     if let Some(session_id) = &self.current_session_id {
                        info!(session_id = %session_id, "Fetching task list for current session.");
                        // Use the DashMap directly for listing IDs, avoid fetching full tasks unless needed
                        if let Some(task_ids) = self.session_tasks.get(session_id) {
                            if task_ids.is_empty() {
                                info!(session_id = %session_id, "No tasks found in current session map.");
                                println!("📭 No tasks recorded in current session.");
                            } else {
                                info!(session_id = %session_id, task_count = %task_ids.len(), "Displaying task list.");
                                println!("\n📋 Tasks Recorded in Current Session:");
                                // Fetch status for each task to display
                                for (i, task_id) in task_ids.iter().enumerate() {
                                    let params = TaskQueryParams { id: task_id.clone(), history_length: Some(0), metadata: None };
                                    let status_str = match self.task_service.get_task(params).await {
                                        Ok(task) => format!("{:?}", task.status.state),
                                        Err(_) => "Status Unknown".to_string(),
                                    };
                                    println!("  {}. {} - Status: {}", i + 1, task_id, status_str);
                                }
                            }
                        } else {
                             info!(session_id = %session_id, "Session ID not found in task map.");
                             println!("📭 No tasks recorded for current session (session ID not found).");
                        }
                    } else {
                        info!("Cannot list tasks: No active session.");
                        println!("⚠️ No active session. Use :session new to create one.");
                    }
                } else if input_trimmed.starts_with(":task ") {
                    let task_id = input_trimmed.trim_start_matches(":task ").trim();
                    info!(task_id = %task_id, "Processing :task command.");
                    if task_id.is_empty() {
                        error!("No task ID provided for :task command.");
                        println!("❌ Error: No task ID provided. Use :task TASK_ID");
                    } else {
                        info!(task_id = %task_id, "Fetching details for task.");
                        // Get task details
                        let params = TaskQueryParams {
                            id: task_id.to_string(),
                            history_length: None, // Get full history for details view
                            metadata: None,
                        };

                        match self.task_service.get_task(params).await {
                            Ok(task) => {
                                info!(task_id = %task.id, "Displaying details for task.");
                                println!("\n🔍 Task Details:");
                                println!("  ID: {}", task.id);
                                println!("  Status: {:?}", task.status.state);
                                println!("  Session: {}", task.session_id.as_deref().unwrap_or("None"));
                                println!("  Timestamp: {}", task.status.timestamp.map(|t| t.to_rfc3339()).as_deref().unwrap_or("None"));

                                // Show artifacts count if any
                                let artifact_count = task.artifacts.as_ref().map_or(0, |a| a.len());
                                println!("  Artifacts: {} (use :artifacts {} to view)", artifact_count, task.id);

                                // Show last status message if any
                                if let Some(message) = &task.status.message {
                                     let text = message.parts.iter()
                                        .filter_map(|p| match p {
                                            Part::TextPart(tp) => Some(tp.text.as_str()),
                                            _ => None,
                                        })
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    println!("\n  Status Message: {}", text);
                                }

                                // Show history preview
                                if let Some(history) = &task.history {
                                     println!("\n  History Preview ({} messages):", history.len());
                                     for msg in history.iter().take(5) { // Show first 5 messages
                                         let role_icon = match msg.role { Role::User => "👤", Role::Agent => "🤖", _ => "➡️" };
                                         let text = msg.parts.iter().filter_map(|p| match p { Part::TextPart(tp) => Some(tp.text.as_str()), _ => None }).collect::<Vec<_>>().join(" ");
                                         let display_text = if text.len() > 60 { format!("{}...", &text[..57]) } else { text.to_string() };
                                         println!("    {} {}: {}", role_icon, msg.role, display_text);
                                     }
                                     if history.len() > 5 { println!("    ..."); }
                                } else {
                                     println!("\n  History: Not available or requested.");
                                }


                            },
                            Err(e) => {
                                error!(task_id = %task_id, error = %e, "Failed to get task details.");
                                println!("❌ Error: Failed to get task: {}", e);
                            }
                        }
                    }
                } else if input_trimmed.starts_with(":artifacts ") {
                    let task_id = input_trimmed.trim_start_matches(":artifacts ").trim();
                    info!(task_id = %task_id, "Processing :artifacts command.");
                    if task_id.is_empty() {
                        error!("No task ID provided for :artifacts command.");
                        println!("❌ Error: No task ID provided. Use :artifacts TASK_ID");
                    } else {
                        info!(task_id = %task_id, "Fetching artifacts for task.");
                        // Get task details with artifacts
                        let params = TaskQueryParams {
                            id: task_id.to_string(),
                            history_length: Some(0), // Don't need history here
                            metadata: None,
                        };

                        match self.task_service.get_task(params).await {
                            Ok(task) => {
                                if let Some(artifacts) = &task.artifacts {
                                    if artifacts.is_empty() {
                                        info!(task_id = %task.id, "No artifacts found for task.");
                                        println!("📦 No artifacts for task {}", task.id);
                                    } else {
                                        info!(task_id = %task.id, count = %artifacts.len(), "Displaying artifacts.");
                                        println!("\n📦 Artifacts for Task {}:", task.id);
                                        for (i, artifact) in artifacts.iter().enumerate() {
                                            println!("  {}. Name: '{}', Desc: '{}', Index: {}, Parts: {}",
                                                     i + 1,
                                                     artifact.name.as_deref().unwrap_or("N/A"),
                                                     artifact.description.as_deref().unwrap_or("N/A"),
                                                     artifact.index,
                                                     artifact.parts.len());
                                            // Optionally print part details
                                            // for part in &artifact.parts { ... }
                                        }
                                    }
                                } else {
                                    info!(task_id = %task.id, "Task found, but has no artifacts field.");
                                    println!("📦 No artifacts found for task {}", task.id);
                                }
                            },
                            Err(e) => {
                                error!(task_id = %task_id, error = %e, "Failed to get task for artifacts.");
                                println!("❌ Error: Failed to get task: {}", e);
                            }
                        }
                    }
                } else if input_trimmed.starts_with(":cancelTask ") {
                    let task_id = input_trimmed.trim_start_matches(":cancelTask ").trim();
                    info!(task_id = %task_id, "Processing :cancelTask command.");
                    if task_id.is_empty() {
                        error!("No task ID provided for :cancelTask command.");
                        println!("❌ Error: No task ID provided. Use :cancelTask TASK_ID");
                    } else {
                        info!(task_id = %task_id, "Attempting to cancel task.");
                        // Cancel the task
                        let params = TaskIdParams {
                            id: task_id.to_string(),
                            metadata: None,
                        };

                        match self.task_service.cancel_task(params).await {
                            Ok(task) => {
                                info!(task_id = %task.id, status = ?task.status.state, "Successfully canceled task.");
                                println!("✅ Successfully canceled task {}", task.id);
                                println!("  Current state: {:?}", task.status.state);
                            },
                            Err(e) => {
                                error!(task_id = %task_id, error = %e, "Failed to cancel task.");
                                println!("❌ Error: Failed to cancel task: {}", e);
                            }
                        }
                    }
                } else if command == "tool" {
                    if parts.len() < 2 {
                        error!("No tool name provided for :tool command.");
                        println!("❌ Error: No tool name provided. Use :tool TOOL_NAME [JSON_PARAMS]");
                        continue;
                    }
                    
                    let tool_args = parts[1]; // Get everything after ":tool "
                    let tool_parts: Vec<&str> = tool_args.splitn(2, ' ').collect();
                    
                    if tool_parts.is_empty() {
                        error!("No tool name provided for :tool command.");
                        println!("❌ Error: No tool name provided. Use :tool TOOL_NAME [JSON_PARAMS]");
                        continue;
                    }
                    
                    let tool_name = tool_parts[0];
                    let params_str = if tool_parts.len() > 1 { tool_parts[1] } else { "{}" }; // Default to empty JSON object if no params
                    info!(tool_name = %tool_name, params_str = %params_str, "Processing :tool command.");

                    // Parse parameters as JSON
                    let params_json: Value = match serde_json::from_str(params_str) {
                        Ok(json) => json,
                        Err(e) => {
                            error!(error = %e, input_params = %params_str, "Failed to parse JSON parameters for :tool command.");
                            println!("❌ Error: Invalid JSON parameters: {}", e);
                            continue; // Skip processing this command
                        }
                    };

                    // Get the ToolExecutor instance
                    if let Some(executor) = &self.task_service.tool_executor { // Access executor via task_service
                        // Execute the tool
                        match executor.execute_tool(tool_name, params_json).await {
                            Ok(result) => {
                                info!(tool_name = %tool_name, "Tool executed successfully via REPL.");
                                // Pretty print the JSON result
                                let result_pretty = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
                                println!("\n🛠️ Tool Result ({}):\n{}\n", tool_name, result_pretty);
                            }
                            Err(e) => {
                                error!(tool_name = %tool_name, error = %e, "Error executing tool via REPL.");
                                println!("❌ Error executing tool '{}': {}", tool_name, e);
                            }
                        }
                    } else {
                        error!("ToolExecutor not available in TaskService for :tool command.");
                        println!("❌ Error: Tool execution is not configured for this agent.");
                    }
                } else {
                    error!("Invalid format for :tool command.");
                    println!("❌ Error: Invalid format. Use :tool <tool_name> [json_params]");
                }
                } else {
                // Process the message locally using the agent's capabilities
                debug!(message = %input_trimmed, "Processing input as local message.");
                match self.process_message_locally(input_trimmed).await { // Renamed function logs internally
                    Ok(response) => {
                        println!("\n🤖 Agent response:\n{}\n", response);
                        info!(response_length = %response.len(), "Local processing successful, displayed response.");
                    },
                    Err(e) => {
                        let error_msg = format!("Error processing message locally: {}", e);
                        error!(error = %e, "Error processing message locally.");
                        println!("❌ {}", error_msg);
                    }
                }
            }
        }

        info!("Exiting REPL mode.");
        Ok(())
    }

    /// Create a new session ID and store it.
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    pub fn create_new_session(&mut self) -> String {
        let session_id = format!("session-{}", Uuid::new_v4());
        info!(new_session_id = %session_id, "Creating new session.");
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
                info!(session_id = %session_id, "Adding task to session history.");
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
            info!(session_id = %session_id, "Fetching full tasks for current session.");
            if let Some(task_ids) = self.session_tasks.get(session_id) {
                 info!(session_id = %session_id, count = %task_ids.len(), "Found task IDs in session map.");
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
        info!("Starting agent server run sequence.");
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
            info!("Received Ctrl+C shutdown signal.");
            println!("\nReceived shutdown signal, stopping server..."); // Keep console message, add newline
            shutdown_token_clone.cancel();
            debug!("Cancellation token cancelled by signal handler.");
        });

        // Create the agent card for this server
        debug!("Creating agent card for server endpoint.");
        let agent_card = self.create_agent_card(); // Logs internally
        info!(agent_name = %agent_card.name, "Generated agent card for server.");

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
                info!(bind_address = %self.bind_address, port = %self.port, "Server started successfully.");
                handle
            },
            Err(e) => {
                error!(bind_address = %self.bind_address, port = %self.port, error = %e, "Failed to start server.");
                // Return an error that can be handled by the caller (e.g., main)
                return Err(anyhow!("Failed to start server: {}", e));
            }
        };

        println!("Server running on http://{}:{}", self.bind_address, self.port); // Keep console message

        // Wait for the server to complete or be cancelled
        info!("Waiting for server task to complete or be cancelled.");
        match server_handle.await {
            Ok(()) => {
                info!("Server shut down gracefully.");
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
        info!("Attempting to send task to remote agent.");
        trace!(message = %message, "Message content for remote task.");
        // ① Ensure a session exists locally
        debug!("Ensuring local session exists.");
        self.ensure_session().await; // Logs internally if new session created
        let session_id = self.current_session_id.clone(); // Clone session_id for sending
        trace!(?session_id, "Using session ID for remote task.");

        let remote_url = self.client_url().unwrap_or_else(|| "unknown".to_string());
        info!(remote_url = %remote_url, session_id = ?session_id, "Attempting to send task to remote agent.");

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
                info!(remote_url = %remote_url, task_id = %t.id, status = ?t.status.state, "Successfully sent task and received response.");
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
                info!(task_id = %task_id_clone, "Cached remote task locally.");
            }
            Err(e) => {
                // Log warning if caching fails, but don't fail the whole operation
                warn!(task_id = %task_id_clone, error = %e, "Could not cache remote task locally.");
            }
        }

        // ④ Add the task (local mirror) to the current session history
        debug!(task_id = %task.id, "Saving imported remote task to session history.");
        self.save_task_to_history(task.clone()).await?; // save_task_to_history logs internally

        info!(task_id = %task.id, session_id = ?session_id, "Remote task processing initiated and linked to session.");
        Ok(task) // Return the original task received from the remote agent
    }

    /// Get capabilities of a remote agent (using A2A client)
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    pub async fn get_remote_agent_card(&mut self) -> Result<AgentCard> {
        let remote_url = self.client_url().unwrap_or_else(|| "unknown".to_string());
        info!(remote_url = %remote_url, "Attempting to get remote agent card.");

        // Check if we have a client configured
        debug!("Checking for A2A client configuration.");
        if let Some(client) = &mut self.client {
            trace!("A2A client found, calling get_agent_card.");
            match client.get_agent_card().await {
                 Ok(agent_card) => {
                    info!(remote_agent_name = %agent_card.name, remote_url = %remote_url, "Retrieved remote agent card successfully.");
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
            streaming: false, // Example: Assuming not supported by default bidirectional agent server? Check run_server.
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
    } // Added missing closing brace for impl BidirectionalAgent
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
    info!("Logging initialized successfully."); // This should now appear in logs
    info!("Starting Bidirectional Agent main function.");

    // Log a prominent line with agent info for better separation in shared logs
    let agent_id = config.server.agent_id.clone();
    let agent_name = config.server.agent_name.clone().unwrap_or_else(|| "Unnamed Agent".to_string());
    info!(agent_id = %agent_id, agent_name = %agent_name, port = %config.server.port, "AGENT_STARTED");

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
        "Key configuration values"
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
             info!("No arguments provided and no mode set in config. Defaulting to REPL mode.");
             config.mode.repl = true;
        } else {
             // Args were provided, but didn't set a mode (e.g., just a config file path)
             info!("Arguments provided but no specific action mode requested. Defaulting to REPL mode.");
             config.mode.repl = true;
        }
    } else if args.len() <= 1 && !config.mode.repl && config.mode.message.is_none() && config.mode.remote_task.is_none() && !config.mode.get_agent_card {
         // Handles case where only a config file was specified, and it didn't set any mode
         info!("Config file loaded but no specific mode set. Defaulting to REPL mode.");
         config.mode.repl = true;
    }
    debug!(?config.mode, "Final execution mode determined.");

    // --- Execute based on mode ---
    if config.mode.repl {
        info!("Starting agent in REPL mode.");
        let mut agent = BidirectionalAgent::new(config.clone())?; // new() logs internally
        agent.run_repl().await // run_repl() logs internally
    }
    // Process a single message
    else if let Some(message) = &config.mode.message {
        info!(message_len = message.len(), "Starting agent to process single message.");
        let mut agent = BidirectionalAgent::new(config.clone())?;
        println!("Processing message: '{}'", message); // Keep console output
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
        info!(message_len = task_message.len(), "Starting agent to send remote task.");
        let mut agent = BidirectionalAgent::new(config.clone())?;
        println!("Sending task to remote agent: '{}'", task_message); // Keep console output
        let task = agent.send_task_to_remote(task_message).await?; // Logs internally
        println!("Task sent successfully!"); // Keep console output
        println!("Task ID: {}", task.id);
        println!("Initial state reported by remote: {:?}", task.status.state);
        info!("Finished sending remote task.");
        Ok(())
    }
    // Get remote agent card
    else if config.mode.get_agent_card {
        if config.client.target_url.is_none() {
             error!("Cannot get remote card: No target URL configured.");
            return Err(anyhow!("No target URL configured. Add target_url to the [client] section in config file or provide via argument."));
        }
        info!("Starting agent to get remote agent card.");
        let mut agent = BidirectionalAgent::new(config.clone())?;
        println!("Retrieving agent card from remote agent..."); // Keep console output
        let card = agent.get_remote_agent_card().await?; // Logs internally
        println!("Remote Agent Card:"); // Keep console output
        println!("  Name: {}", card.name);
        println!("  Version: {}", card.version);
        println!("  Description: {}", card.description.as_deref().unwrap_or("None"));
        println!("  URL: {}", card.url);
        // TODO: Print more card details if needed
        println!("  Capabilities: {:?}", card.capabilities);
        println!("  Skills: {}", card.skills.len());
        info!("Finished getting remote agent card.");
        Ok(())
    }
    // Default mode: Run the server (if not REPL and no other specific action)
    else {
        info!("Starting agent in default server mode (no REPL or specific action requested).");
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
