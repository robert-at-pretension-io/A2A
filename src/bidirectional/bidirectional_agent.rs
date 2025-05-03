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
}; // Import StdError for error mapping and IO
use tokio::{
    fs::OpenOptions, // Import OpenOptions for async file I/O
    io::AsyncWriteExt, // Import AsyncWriteExt for write_all
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use toml;
use tracing::{debug, error, info, warn, instrument, Level, Instrument}; // Import Instrument trait
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer}; // For subscriber setup
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
#[derive(Debug, Clone)]
struct AgentDirectoryEntry {
    /// Agent card with capabilities
    card: AgentCard,
    /// Last time this agent was seen
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
    pub fn add_or_update_agent(&self, agent_id: String, card: AgentCard) {
        let entry = AgentDirectoryEntry {
            card,
            last_seen: Utc::now(),
            active: true,
        };
        self.agents.insert(agent_id, entry);
    }

    fn get_agent(&self, agent_id: &str) -> Option<AgentDirectoryEntry> {
        self.agents.get(agent_id).map(|e| e.value().clone())
    }

    fn list_active_agents(&self) -> Vec<AgentDirectoryEntry> {
        self.agents
            .iter()
            .filter(|e| e.active)
            .map(|e| e.value().clone())
            .collect()
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
    async fn complete(&self, prompt: &str) -> Result<String> {
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

        // Send the request to the Claude API
        let response = client.post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&payload)
            .send()
            .await
            .context("Failed to send request to Claude API")?;

        // Check if the request was successful
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(anyhow!("Claude API error ({}): {}", status, error_text));
        }

        // Parse the response
        let response_json: Value = response.json().await
            .context("Failed to parse Claude API response")?;
        
        // Extract the completion
        let completion = response_json["content"][0]["text"].as_str()
            .ok_or_else(|| anyhow!("Failed to extract completion from response"))?;
        
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
    pub async fn decide_execution_mode(&self, task: &Task) -> Result<RoutingDecision, ServerError> {
         // Extract the task content for analysis
        // Use history if available, otherwise maybe the top-level message if applicable
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


        if task_text.is_empty() {
            // If still empty after checking history and initial message
            warn!(task_id = %task.id, "Task text is empty after checking history and message, defaulting to local execution with 'echo' tool.");
            return Ok(RoutingDecision::Local { tool_names: vec!["echo".to_string()] });
        }

        // Get the list of available agents from the local directory
        let available_agents = self.directory.list_active_agents();
        let agent_descriptions = available_agents.iter()
            .map(|a| {
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

        // Build a prompt for the LLM to decide routing
        let routing_prompt = format!(r#"
You need to decide whether to handle a task locally or delegate it to another agent.

AVAILABLE AGENTS:
{}

TASK:
{}

Please analyze the task and decide whether to:
1. Handle it locally (respond with "LOCAL")
2. Delegate to a specific agent (respond with "REMOTE: [agent-id]")

Your response should be exactly one of those formats, with no additional text.
"#, agent_descriptions, task_text);

        info!(task_id = %task.id, "Routing prompt for LLM:\n{}", routing_prompt);

        // Get the routing decision from the LLM
        let decision_result = self.llm.complete(&routing_prompt).await;

        // Map anyhow::Error from LLM to ServerError::Internal
        let decision = match decision_result {
            Ok(d) => {
                let trimmed_decision = d.trim().to_string();
                info!(task_id = %task.id, llm_response = %trimmed_decision, "LLM routing decision response received.");
                trimmed_decision
            }
            Err(e) => {
                error!(task_id = %task.id, error = %e, "LLM routing decision failed.");
                // Fallback to local echo on LLM error
                return Ok(RoutingDecision::Local { tool_names: vec!["echo".to_string()] });
            }
        };

        // Parse the decision
        if decision == "LOCAL" {
            info!(task_id = %task.id, "LLM decided LOCAL execution. Proceeding to tool selection.");
            // --- LLM Tool Selection Logic ---
            let tool_list_str = self.enabled_tools.join(", ");
            let tool_choice_prompt = format!(
                "You have the following local tools available:\n{}\n\n\
                Based on the previous TASK description, choose the single best tool (exact name) to handle it.\n\
                Respond ONLY with the tool name (e.g., 'llm', 'summarize', 'echo').",
                tool_list_str
            );

            debug!("Asking LLM to choose tool with prompt: {}", tool_choice_prompt); // Use tracing debug

            let tool_choice_result = self.llm.complete(&tool_choice_prompt).await;

            let chosen_tool_name = match tool_choice_result {
                 Ok(tool_name_raw) => {
                    let tool_name = tool_name_raw.trim().to_string();
                    // Validate the LLM's choice against the enabled tools
                    if self.enabled_tools.contains(&tool_name) {
                         info!(task_id = %task.id, tool_name = %tool_name, "LLM chose tool.");
                         tool_name // Use the valid tool chosen by LLM
                    } else {
                         warn!(task_id = %task.id, chosen_tool = %tool_name, enabled_tools = ?self.enabled_tools, "LLM chose an unknown/disabled tool. Falling back to 'echo'.");
                         "echo".to_string() // Fallback to echo if choice is invalid
                    }
                 },
                 Err(e) => {
                     warn!(task_id = %task.id, error = %e, "LLM failed to choose a tool. Falling back to 'echo'.");
                     "echo".to_string() // Fallback to echo on error
                 }
            };

            info!(task_id = %task.id, final_tool = %chosen_tool_name, "Final tool decision for local execution.");
            Ok(RoutingDecision::Local { tool_names: vec![chosen_tool_name] })
            // --- End LLM Tool Selection Logic ---

        } else if decision.starts_with("REMOTE: ") {
            let agent_id = decision.strip_prefix("REMOTE: ").unwrap().trim().to_string();
            info!(task_id = %task.id, remote_agent_id = %agent_id, "LLM decided REMOTE execution.");

            // Verify the agent exists in the local directory
            if self.directory.get_agent(&agent_id).is_none() {
                 warn!(task_id = %task.id, remote_agent_id = %agent_id, "LLM decided to delegate to unknown agent, falling back to local execution with 'echo' tool.");
                 // Fall back to local if agent not found, using echo tool
                 Ok(RoutingDecision::Local { tool_names: vec!["echo".to_string()] })
            } else {
                 info!(task_id = %task.id, remote_agent_id = %agent_id, "Routing decision: Remote delegation.");
                 Ok(RoutingDecision::Remote { agent_id })
            }
        } else {
            warn!(task_id = %task.id, llm_decision = %decision, "LLM routing decision was unclear, falling back to local execution with 'echo' tool.");
            // Default to local echo if the decision isn't clear
            Ok(RoutingDecision::Local { tool_names: vec!["echo".to_string()] })
        }
    }
}

#[async_trait]
impl LlmTaskRouterTrait for BidirectionalTaskRouter {
    // Match the trait signature: takes TaskSendParams, returns Result<RoutingDecision, ServerError>
    async fn route_task(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError> {
        // We need a Task object to make the decision based on history/message.
        // Construct a temporary Task from TaskSendParams.
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

        // Call the updated decide_execution_mode which now returns RoutingDecision
        self.decide_execution_mode(&task).await
    }

    // Add the required process_follow_up method
    async fn process_follow_up(&self, _task_id: &str, _message: &Message) -> Result<RoutingDecision, ServerError> {
        // For now, always route follow-ups locally as a simple default.
        // A real implementation would likely involve the LLM again.
        Ok(RoutingDecision::Local {
            tool_names: vec!["default_local_tool".to_string()]
        })
    }
    
    // Implement the decide method required by LlmTaskRouterTrait
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError> {
        // Delegate to route_task, which now handles the full decision including tool choice
        self.route_task(params).await
    }

    // Implement should_decompose method required by LlmTaskRouterTrait
    async fn should_decompose(&self, _params: &TaskSendParams) -> Result<bool, ServerError> {
        // Simple implementation that never decomposes tasks
        Ok(false)
    }
    
    // Implement decompose_task method required by LlmTaskRouterTrait
    async fn decompose_task(&self, _params: &TaskSendParams) -> Result<Vec<crate::server::task_router::SubtaskDefinition>, ServerError> {
        // Simple implementation that returns an empty list (no decomposition)
        Ok(Vec::new())
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
    pub fn new(config: BidirectionalAgentConfig) -> Result<Self> {
        // Create the agent directory (local helper)
        let agent_directory = Arc::new(AgentDirectory::new());

        // Create the LLM client (local helper)
        let llm: Arc<dyn LlmClient> = if let Some(api_key) = &config.llm.claude_api_key {
            Arc::new(ClaudeLlmClient::new(api_key.clone(), config.llm.system_prompt.clone()))
        } else {
            // Allow running without LLM if only acting as server/client without local processing
            warn!("No Claude API key provided. Local LLM processing will not be available.");
            // Provide a dummy LLM client or handle this case appropriately
            error!("No LLM configuration provided. Set CLAUDE_API_KEY environment variable or add claude_api_key to config file.");
            return Err(anyhow!("No LLM configuration provided.")); // Or handle differently
        };

        // Create the task repository (needed by services)
        let task_repository: Arc<dyn TaskRepository> = Arc::new(InMemoryTaskRepository::new());

        // Create the canonical agent registry (from server module)
        let agent_registry = Arc::new(AgentRegistry::new()); // Assuming AgentRegistry::new() exists

        // Create the canonical client manager (from server module)
        // It needs the registry
        let client_manager = Arc::new(ClientManager::new(agent_registry.clone()));

        // Create our custom task router implementation
        // Create our custom task router implementation (pass enabled tools later)
        // We need the list of enabled tools first for the executor
        let enabled_tools_list = config.tools.enabled.clone();
        let enabled_tools = Arc::new(enabled_tools_list); // Arc for sharing

        // Create a tool executor using the new constructor with enabled tools
        let bidirectional_tool_executor = Arc::new(ToolExecutor::with_enabled_tools(
            &config.tools.enabled, // Pass slice of enabled tool names
            llm.clone(),           // Pass LLM client for tools that need it
        ));

        // Create the task router, passing the enabled tools list
        let bidirectional_task_router: Arc<dyn LlmTaskRouterTrait> =
            Arc::new(BidirectionalTaskRouter::new(
                llm.clone(),
                agent_directory.clone(),
                enabled_tools.clone(), // Pass Arc<Vec<String>>
            ));

        // Create the task service using the canonical components and our trait implementations
        let task_service = Arc::new(TaskService::bidirectional(
            task_repository.clone(),
            bidirectional_task_router, // Pass our LlmTaskRouterTrait implementation
            bidirectional_tool_executor, // Pass the ToolExecutor
            client_manager.clone(), // Pass canonical ClientManager
            agent_registry.clone(), // Pass canonical AgentRegistry
            config.server.agent_id.clone(), // Pass agent_id
        ));
        info!("TaskService created.");

        // Create the streaming service
        let streaming_service = Arc::new(StreamingService::new(task_repository.clone()));
        info!("StreamingService created.");

        // Create the notification service (pass repository)
        let notification_service = Arc::new(NotificationService::new(task_repository.clone()));
        info!("NotificationService created.");

        // Initialize an A2A client if target URL is provided
        let client = if let Some(target_url) = &config.client.target_url {
            info!(target_url = %target_url, "Initializing A2A client for target URL.");
            Some(A2aClient::new(target_url))
        } else {
            info!("No target URL provided, A2A client not initialized.");
            None
        };

        let agent = Self {
            task_service,
            streaming_service,
            notification_service,

            // Store canonical versions needed by the agent itself (if any)
            agent_registry,
            client_manager,

            // Store local helper components
            agent_directory, // Keep local directory for routing logic
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

            // Initialize known servers map
            known_servers: Arc::new(DashMap::new()),
        };

        // Logging initialization happens in main() now.
        // We can log that the agent struct itself is constructed.
        info!(agent_id = %agent.agent_id, bind_address = %agent.bind_address, port = %agent.port, "BidirectionalAgent struct created.");

        Ok(agent)
    }

    /// Create a session on demand so remote tasks can be grouped
    fn ensure_session(&mut self) {
        if self.current_session_id.is_none() {
            let session_id = self.create_new_session(); // existing helper
            // Log session creation (moved from create_new_session due to async context)
            // Use tokio::spawn for fire-and-forget logging if needed, or make ensure_session async
            // For simplicity, we'll log synchronously here, assuming it's called from an async context
            // where blocking briefly is acceptable (like the REPL loop).
            // Logging is handled by the calling function or tracing infrastructure
            info!(session_id = %session_id, "Created new session on demand.");
        }
    }


    /// Process a message locally (e.g., from REPL input that isn't a command)
    #[instrument(skip(self), fields(agent_id = %self.agent_id, message_text))]
    pub async fn process_message_locally(&self, message_text: &str) -> Result<String> {
        info!("Processing message locally.");

        // Check if we're continuing an existing task that is in InputRequired state
        let mut continue_task_id: Option<String> = None;
        if let Some(session_id) = &self.current_session_id {
            if let Some(task_ids) = self.session_tasks.get(session_id) {
                // Check the last task in the session
                if let Some(last_task_id) = task_ids.iter().last() {
                    // Get the task to check its state
                    let params = TaskQueryParams {
                        id: last_task_id.clone(),
                        history_length: None,
                        metadata: None,
                    };
                    
                    match self.task_service.get_task(params).await {
                        Ok(task) => {
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
                    debug!(session_id = %session_id, "No previous tasks found in session.");
                }
            }
        }
    
        // Create a unique task ID if not continuing an existing task
        let task_id = if let Some(id) = continue_task_id {
            info!(task_id = %id, "Continuing existing task.");
            id
        } else {
            let new_id = Uuid::new_v4().to_string();
            info!(task_id = %new_id, "Creating new task.");
            new_id
        };

        let task_id_clone = task_id.clone(); // Clone for instrument span

        // Create the message
        let initial_message = Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                text: message_text.to_string(),
                metadata: None,
                type_: "text".to_string(),
            })],
            metadata: None,
        };
        
        // Create TaskSendParams
        let params = TaskSendParams {
            id: task_id.clone(),
            message: initial_message,
            session_id: self.current_session_id.clone(),
            metadata: None,
            history_length: None,
            push_notification: None,
        };

        info!(task_id = %task_id, "Calling task_service.process_task.");
        // Use task_service to process the task
        // Instrument the call to task_service
        let task = async {
            self.task_service.process_task(params).await
        }.instrument(tracing::info_span!("task_service_process", task_id = %task_id_clone)).await?;

        info!(task_id = %task.id, status = ?task.status.state, "Task processing completed by task_service.");

        // Save task to history
        self.save_task_to_history(task.clone()).await?; // save_task_to_history logs internally now
        // Extract response from task
        let mut response = self.extract_text_from_task(&task);

        // If the task is in InputRequired state, indicate that in the response
        if task.status.state == TaskState::InputRequired {
            info!(task_id = %task.id, "Task requires more input.");
            response.push_str("\n\n[The agent needs more information. Your next message will continue this task.]");
        }

        info!(task_id = %task.id, "Responding.");
        Ok(response)
    }

    /// Prints the REPL help message to the console.
    fn print_repl_help(&self) {
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
        println!("  :quit            - Exit the REPL");
        println!("========================================\n");
    }

    // Helper to extract text from task
    fn extract_text_from_task(&self, task: &Task) -> String {
        // First check the status message
        if let Some(ref message) = task.status.message {
            let text = message.parts.iter()
                .filter_map(|p| match p {
                    Part::TextPart(tp) => Some(tp.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            
            if !text.is_empty() {
                return text;
            }
        }
        
        // Then check history if available
        if let Some(history) = &task.history {
            let agent_messages = history.iter()
                .filter(|m| m.role == Role::Agent)
                .flat_map(|m| m.parts.iter())
                .filter_map(|p| match p {
                    Part::TextPart(tp) => Some(tp.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            
            if !agent_messages.is_empty() {
                return agent_messages;
            }
        }
        
        // Fallback
        "No response text available.".to_string()
    }

    /// Run an interactive REPL (Read-Eval-Print Loop)
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    pub async fn run_repl(&mut self) -> Result<()> {
        info!("Entering REPL mode.");
        // Print the initial help message
        self.print_repl_help();

        // --- Spawn Background Task for Initial Connection ---
        if let Some(initial_url) = self.client_url() {
            println!("Configured target URL: {}. Attempting connection in background...", initial_url);
            // Clone necessary data for the background task
            // Clone necessary data for the background task's tracing span
            let bg_agent_id = self.agent_id.clone();
            let bg_initial_url = initial_url.clone();
            let agent_directory = self.agent_directory.clone();
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

                info!("Starting background connection attempt.");

                let max_retries = 5;
                let retry_delay = std::time::Duration::from_secs(2);
                let mut connected = false;

                // Create a *new* client instance specifically for this background task
                let mut bg_client = A2aClient::new(&initial_url);

                for attempt in 1..=max_retries {
                    match bg_client.get_agent_card().await {
                        Ok(card) => {
                            let remote_agent_name = card.name.clone();
                            info!(remote_agent_name = %remote_agent_name, "Background connection successful.");
                            println!("✅ Background connection successful to {} ({})", remote_agent_name, initial_url); // Print to console as well

                            // Update shared state
                            known_servers.insert(initial_url.clone(), remote_agent_name.clone());
                            agent_directory.add_or_update_agent(remote_agent_name.clone(), card.clone()); // Use name as ID here
                            info!(remote_agent_name = %remote_agent_name, "Added/updated agent in directory.");

                            connected = true;
                            break; // Exit retry loop on success
                        }
                        Err(e) => {
                            warn!(attempt = %attempt, max_retries = %max_retries, error = %e, "Background connection attempt failed.");
                            if attempt < max_retries {
                                info!(delay_seconds = %retry_delay.as_secs(), "Retrying background connection...");
                                tokio::time::sleep(retry_delay).await;
                            }
                        }
                    }
                }

                if !connected {
                    error!(attempts = %max_retries, "Background connection failed after all attempts.");
                    println!("❌ Background connection to {} failed after {} attempts.", initial_url, max_retries); // Print final failure to console
                }
            }); // Background task ends here
        }
        // --- End Background Task Spawn ---


        // Flag to track if we have a listening server running
        let mut server_running = false;
        let mut server_shutdown_token: Option<CancellationToken> = None;
        // Removed misplaced match block and duplicate variable declarations here

        // Check if auto-listen flag is set in environment variable
        let auto_listen = std::env::var("AUTO_LISTEN").map(|v| v == "1" || v.to_lowercase() == "true").unwrap_or(false);
        
        // Start server automatically if auto-listen is enabled
        if auto_listen {
            println!("🚀 Auto-starting server on port {}...", self.port);
            
            // Create a cancellation token
            let token = CancellationToken::new();
            server_shutdown_token = Some(token.clone());
            
            // Create a channel to communicate server start status back to REPL
            let (tx, rx) = tokio::sync::oneshot::channel();
            
            // Clone what we need for the task
            let task_service = self.task_service.clone();
            let streaming_service = self.streaming_service.clone();
            let notification_service = self.notification_service.clone();
            let bind_address = self.bind_address.clone();
            let port = self.port;
            let agent_card = serde_json::to_value(self.create_agent_card()).unwrap_or_else(|e| {
                warn!("Failed to serialize agent card: {}", e);
                serde_json::json!({})
            });
            
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
                        // Send success status back to REPL
                        let _ = tx.send(Ok(()));
                        
                        // Wait for the server to complete or be cancelled
                        match handle.await {
                            Ok(()) => info!("Server shut down gracefully."),
                            Err(e) => error!("Server error: {}", e),
                        }
                    },
                    Err(e) => {
                        // Send error status back to REPL
                        let _ = tx.send(Err(format!("{}", e)));
                        error!("Failed to start server: {}", e);
                    }
                }
            });
            
            // Wait for the server start status
            // Use a short timeout to avoid blocking the REPL if something goes wrong
            match tokio::time::timeout(std::time::Duration::from_secs(3), rx).await { // Increased timeout slightly
                Ok(Ok(Ok(()))) => {
                    server_running = true;
                    info!(bind_address = %self.bind_address, %port, "Auto-started server successfully.");
                    println!("✅ Server started on http://{}:{}", self.bind_address, port);
                    println!("The server will run until you exit the REPL or send :stop");
                },
                Ok(Ok(Err(e))) => {
                    error!(bind_address = %self.bind_address, %port, error = %e, "Error auto-starting server.");
                    println!("❌ Error starting server: {}", e);
                    println!("The server could not be started. Try a different port or check for other services using this port.");
                    server_shutdown_token = None; // Clean up token
                },
                Ok(Err(channel_err)) => {
                    error!(bind_address = %self.bind_address, %port, error = %channel_err, "Server init channel error during auto-start.");
                    println!("❌ Error: Server initialization failed due to channel error");
                    server_shutdown_token = None; // Clean up token
                },
                Err(timeout_err) => {
                    error!(bind_address = %self.bind_address, %port, error = %timeout_err, "Timeout waiting for auto-start server.");
                    println!("❌ Timeout waiting for server to start");
                    println!("The server is taking too long to start. It might be starting in the background or could have failed.");
                    println!("You can try :stop to cancel any server processes that might be running.");
                    // Keep running flag true so user can try :stop
                    server_running = true;
                }
            }
        }

        let stdin = io::stdin();
        let mut reader = stdin.lock();
        let mut input = String::new();
        
        // Flag to track if we have a listening server running
        let mut server_running = false;
        let mut server_shutdown_token: Option<CancellationToken> = None;
        
        loop {
            // Display prompt (with connected agent information if available)
            if let Some(url) = self.client_url() {
                print!("agent@{} > ", url);
            } else {
                print!("agent > ");
            }
            io::stdout().flush().ok();
            
            input.clear();
            if reader.read_line(&mut input)? == 0 {
                // EOF reached (e.g., Ctrl+D)
                info!("EOF detected. Exiting REPL.");
                println!("EOF detected. Exiting REPL.");
                // Perform cleanup similar to :quit
                if let Some(token) = server_shutdown_token.take() {
                    info!("Shutting down server due to EOF.");
                    println!("Shutting down server...");
                    token.cancel();
                }
                break; // Exit loop
            }

            let input_trimmed = input.trim();
            info!(user_input = %input_trimmed, "REPL input received.");

            if input_trimmed.is_empty() {
                continue;
            }

            if input_trimmed.starts_with(":") {
                info!(command = %input_trimmed, "Processing REPL command.");
                // Handle special commands using input_trimmed
                if input_trimmed == ":help" {
                    self.print_repl_help();
                } else if input_trimmed == ":quit" {
                    info!("User initiated quit.");
                    println!("Exiting REPL. Goodbye!");

                    // Shutdown server if running
                    if let Some(token) = server_shutdown_token.take() {
                        info!("Shutting down server.");
                        println!("Shutting down server...");
                        token.cancel();
                    }
                    break; // Exit loop
                } else if input_trimmed == ":card" {
                    let card = self.create_agent_card();
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
                } else {
                    warn!(command = %input_trimmed, "Unknown REPL command entered.");
                    println!("❌ Unknown command: {}", input_trimmed);
                    println!("Type :help for a list of commands");
                }
            } else {
                // Process the message locally using the agent's capabilities
                match self.process_message_locally(input_trimmed).await { // Renamed function
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
        self.session_tasks.insert(session_id.clone(), Vec::new());
        session_id
    }

    /// Add task ID to the current session's task list.
    #[instrument(skip(self, task), fields(agent_id = %self.agent_id, task_id = %task.id))]
    async fn save_task_to_history(&self, task: Task) -> Result<()> {
        if let Some(session_id) = &self.current_session_id {
            if let Some(mut tasks) = self.session_tasks.get_mut(session_id) {
                info!(session_id = %session_id, "Adding task to session history.");
                tasks.push(task.id.clone());
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
                for task_id in task_ids.iter() {
                    debug!(task_id = %task_id, "Fetching details for task.");
                    // Use TaskQueryParams to get task with history
                    let params = TaskQueryParams {
                        id: task_id.clone(),
                        history_length: None, // Get full history
                        metadata: None,
                    };

                    match self.task_service.get_task(params).await {
                        Ok(task) => tasks.push(task),
                        Err(e) => {
                            error!(task_id = %task_id, session_id = %session_id, error = %e, "Failed to get task details for session history.");
                            // Continue trying to get other tasks, but log the error
                        }
                    }
                }
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
    fn client_url(&self) -> Option<String> {
        // Use the target_url stored in the agent's own configuration
        self.client_config.target_url.clone()
    }

    /// Run the agent server
    pub async fn run(&self) -> Result<()> {
        // Create a cancellation token for graceful shutdown
        let shutdown_token = CancellationToken::new();
        let shutdown_token_clone = shutdown_token.clone();

        // Set up signal handlers for graceful shutdown
        let agent_id_clone = self.agent_id.clone(); // Clone for the signal handler task
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.expect("Failed to install CTRL+C handler");
            // Use tracing within the signal handler task
            let span = tracing::info_span!("signal_handler", agent_id = %agent_id_clone);
            let _enter = span.enter();
            info!("Received Ctrl+C shutdown signal.");
            println!("Received shutdown signal, stopping server..."); // Keep console message
            shutdown_token_clone.cancel();
        });

        // Create the agent card for this server
        let agent_card = self.create_agent_card();
        info!(agent_name = %agent_card.name, "Generated agent card for server.");

        // Convert to JSON value for the server
        let agent_card_json = match serde_json::to_value(&agent_card) {
             Ok(json) => json,
             Err(e) => {
                 error!(error = %e, "Failed to serialize agent card to JSON. Using empty object.");
                 serde_json::json!({})
             }
        };

        info!(bind_address = %self.bind_address, port = %self.port, "Attempting to start server.");
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
    #[instrument(skip(self, message), fields(agent_id = %self.agent_id, message))]
    pub async fn send_task_to_remote(&mut self, message: &str) -> Result<Task> {
        // ① Ensure a session exists locally
        self.ensure_session(); // Logs internally if new session created
        let session_id = self.current_session_id.clone(); // Clone session_id for sending

        let remote_url = self.client_url().unwrap_or_else(|| "unknown".to_string());
        info!(remote_url = %remote_url, session_id = ?session_id, "Attempting to send task to remote agent.");

        // Get mutable reference to the client
        let client = match self.client.as_mut() {
             Some(c) => c,
             None => {
                 error!("No remote client configured. Use :connect first.");
                 return Err(anyhow!("No remote client configured. Use :connect first."));
             }
        };

        // ② Send the task with the current session ID
        let task_result = client.send_task(message, session_id.clone()).await; // Pass cloned session_id

        let task = match task_result {
            Ok(t) => {
                info!(remote_url = %remote_url, task_id = %t.id, status = ?t.status.state, "Successfully sent task and received response.");
                t
            }
            Err(e) => {
                error!(remote_url = %remote_url, error = %e, "Error sending task to remote agent.");
                return Err(anyhow!("Error sending task to {}: {}", remote_url, e));
            }
        };

        // ③ Mirror the returned task locally (read-only copy)
        let task_id_clone = task.id.clone(); // Clone ID for logging messages
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
        if let Some(client) = &mut self.client {
            match client.get_agent_card().await {
                 Ok(agent_card) => {
                    info!(remote_agent_name = %agent_card.name, remote_url = %remote_url, "Retrieved remote agent card successfully.");
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
        debug!(agent_id = %self.agent_id, "Creating agent card.");
        // Construct AgentCapabilities based on actual capabilities
        // TODO: Make these capabilities configurable or dynamically determined
        let capabilities = AgentCapabilities {
            push_notifications: true, // Example: Assuming supported
            state_transition_history: true, // Example: Assuming supported
            streaming: false, // Example: Assuming not supported
            // Add other fields from AgentCapabilities if they exist
        };

        AgentCard {
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
        }
    }
}

/// Server configuration section
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
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config_str = fs::read_to_string(path)
            .map_err(|e| anyhow!("Failed to read config file: {}", e))?;
        
        let mut config: BidirectionalAgentConfig = toml::from_str(&config_str)
            .map_err(|e| anyhow!("Failed to parse config file: {}", e))?;
        
        // Check for environment variable override for API key
        if config.llm.claude_api_key.is_none() {
            config.llm.claude_api_key = std::env::var("CLAUDE_API_KEY").ok();
        }
        
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
    let args: Vec<String> = std::env::args().collect();
    let mut config = BidirectionalAgentConfig::default();
    config.mode.repl = true; // Default to REPL

    // Check for environment variable API key *before* loading config file
    if config.llm.claude_api_key.is_none() {
        config.llm.claude_api_key = std::env::var("CLAUDE_API_KEY").ok();
    }

    // Process command line arguments to potentially load a config file or override settings
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        // Use temporary logging before full setup
        eprintln!("[PRE-LOG] Processing arg: {}", arg);

        if arg == "--listen" || arg == "-l" {
            config.mode.auto_listen = true;
            std::env::set_var("AUTO_LISTEN", "true"); // Keep env var for REPL auto-start check
            if i + 1 < args.len() {
                if let Ok(port) = args[i + 1].parse::<u16>() {
                    config.server.port = port;
                    i += 1;
                } else {
                     eprintln!("[PRE-LOG] WARN: Argument after --listen ('{}') is not a valid port.", args[i+1]);
                }
            }
        } else if arg.starts_with("--port=") {
            if let Ok(port) = arg.trim_start_matches("--port=").parse::<u16>() {
                config.server.port = port;
            } else {
                 eprintln!("[PRE-LOG] WARN: Invalid port format in '{}'. Using default.", arg);
            }
        } else if arg.contains(':') && !arg.starts_with(":") {
            let parts: Vec<&str> = arg.split(':').collect();
            if parts.len() == 2 {
                let server = parts[0];
                if let Ok(port) = parts[1].parse::<u16>() {
                    config.client.target_url = Some(format!("http://{}:{}", server, port));
                } else {
                    eprintln!("[PRE-LOG] WARN: Invalid port in '{}'. Treating as config file.", arg);
                    load_config_from_path(&mut config, arg)?;
                }
            } else {
                 eprintln!("[PRE-LOG] WARN: Invalid arg format '{}'. Treating as config file.", arg);
                 load_config_from_path(&mut config, arg)?;
            }
        } else {
            load_config_from_path(&mut config, arg)?;
        }
        i += 1;
    }

    // --- Initialize Logging ---
    // Determine log level (e.g., from RUST_LOG env var, default to info)
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info")); // Default to info if RUST_LOG not set

    // Setup console logging layer
    let console_layer = fmt::layer()
        .with_writer(io::stderr) // Log to stderr
        .with_ansi(true) // Enable colors
        .with_target(true) // Show module path
        .with_level(true); // Show log level

    // Setup file logging layer (if configured)
    let file_layer = if let Some(log_path_str) = &config.mode.repl_log_file {
        eprintln!("[PRE-LOG] Configuring file logging to: {}", log_path_str);
        let log_path = PathBuf::from(log_path_str);

        // 1. Validate path and create directory, resulting in Option<PathBuf>
        let maybe_valid_path = if let Some(parent_dir) = log_path.parent() {
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
                Some(log_path) // Dir already exists, path is valid
            }
        } else {
            Some(log_path) // No parent dir (e.g., relative path), assume valid
        };

        // 2. If path is valid, attempt to create the file appender layer
        if let Some(path) = maybe_valid_path {
            match tracing_appender::rolling::daily(
                path.parent().unwrap_or_else(|| Path::new(".")), // Use validated path's parent
                path.file_name().unwrap_or_else(|| std::ffi::OsStr::new("agent.log")) // Use validated path's filename
            ) {
                Ok(file_appender) => {
                    eprintln!("[PRE-LOG] File appender created successfully for {}", path.display());
                    Some(
                        fmt::layer()
                            .with_writer(file_appender)
                            .with_ansi(false)
                            .with_target(true)
                            .with_level(true)
                            .boxed()
                    )
                },
                Err(e) => {
                    eprintln!("[PRE-LOG] ERROR: Failed to create file appender for '{}': {}", path.display(), e);
                    None // Appender creation failed
                }
            }
        } else {
            None // Path was invalid (directory creation failed)
        }
    } else {
        eprintln!("[PRE-LOG] File logging not configured.");
        None // No log file path configured
    };


    // Combine layers and initialize the global subscriber
    let subscriber_builder = tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer);

    if let Some(file_layer) = file_layer {
    // Initialize differently based on whether file_layer is Some or None
    if let Some(file_layer) = file_layer {
         subscriber_builder.with(file_layer).init(); // Pass the unwrapped Box<dyn Layer...>
    } else {
         subscriber_builder.init(); // Initialize without the file layer
    }

    // --- Logging is now fully initialized ---
    info!("Logging initialized.");
    info!("Starting Bidirectional Agent main function.");

    // Log the final effective configuration *after* logging is initialized
    info!(?config, "Effective configuration loaded."); // Use debug formatting for the whole struct

    // Determine final mode (logic remains similar, but logging works now)
    if config.mode.message.is_none()
        && config.mode.remote_task.is_none()
        && !config.mode.get_agent_card
        && !config.mode.repl
    {
        if args.len() <= 1 {
             info!("No arguments provided and no mode set in config. Defaulting to REPL mode.");
             config.mode.repl = true;
        } else {
             info!("Arguments provided but no specific action mode requested. Defaulting to REPL mode.");
             config.mode.repl = true;
        }
    } else if args.len() <= 1 && !config.mode.repl && config.mode.message.is_none() && config.mode.remote_task.is_none() && !config.mode.get_agent_card {
         // Handles case where only a config file was specified, and it didn't set any mode
         info!("Config file loaded but no specific mode set. Defaulting to REPL mode.");
         config.mode.repl = true;
    }

    // --- Execute based on mode ---
    if config.mode.repl {
        info!("Starting agent in REPL mode.");
        let mut agent = BidirectionalAgent::new(config.clone())?;
        agent.run_repl().await // Return the result directly
    }
    // Process a single message
    else if let Some(message) = &config.mode.message {
        info!(message = %message, "Starting agent to process single message.");
        let agent = BidirectionalAgent::new(config.clone())?;
        println!("Processing message: '{}'", message); // Keep console output
        let response = agent.process_message_locally(message).await?; // Use renamed function
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
        info!(task_message = %task_message, "Starting agent to send remote task.");
        let mut agent = BidirectionalAgent::new(config.clone())?;
        println!("Sending task to remote agent: '{}'", task_message); // Keep console output
        let task = agent.send_task_to_remote(task_message).await?;
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
        let card = agent.get_remote_agent_card().await?;
        println!("Remote Agent Card:"); // Keep console output
        println!("  Name: {}", card.name);
        println!("  Version: {}", card.version);
        println!("  Description: {}", card.description.as_deref().unwrap_or("None"));
        println!("  URL: {}", card.url);
        // ... (print other card details as before) ...
        println!("  Skills: {}", card.skills.len());
        info!("Finished getting remote agent card.");
        Ok(())
    }
    // Default mode: Run the server (if not REPL and no other specific action)
    else {
        info!("Starting agent in default server mode.");
        let agent = BidirectionalAgent::new(config)?;
        agent.run().await // Return the result directly
    }
}

/// Helper function to load config from path, used in main arg parsing.
/// Logs using eprintln because tracing might not be initialized yet.
fn load_config_from_path(config: &mut BidirectionalAgentConfig, config_path: &str) -> Result<()> {
    eprintln!("[PRE-LOG] Attempting to load configuration from path: {}", config_path);
    match BidirectionalAgentConfig::from_file(config_path) {
        Ok(mut loaded_config) => {
            // Preserve env var API key if it was set and config file doesn't have one
            if config.llm.claude_api_key.is_some() && loaded_config.llm.claude_api_key.is_none() {
                loaded_config.llm.claude_api_key = config.llm.claude_api_key.clone();
                eprintln!("[PRE-LOG] Preserved Claude API key from environment variable.");
            }
            // Preserve command-line overrides if they were set before loading the file
            // Example: Preserve port if set via --port= before the config file path
             if config.server.port != default_port() && loaded_config.server.port == default_port() {
                 loaded_config.server.port = config.server.port;
                 eprintln!("[PRE-LOG] Preserved server port override from command line: {}", loaded_config.server.port);
             }
             // Example: Preserve target_url if set via host:port before the config file path
             if config.client.target_url.is_some() && loaded_config.client.target_url.is_none() {
                 loaded_config.client.target_url = config.client.target_url.clone();
                 eprintln!("[PRE-LOG] Preserved target URL override from command line: {}", loaded_config.client.target_url.as_deref().unwrap_or("N/A"));
             }
             // Example: Preserve auto_listen if set via --listen before the config file path
             if config.mode.auto_listen && !loaded_config.mode.auto_listen {
                 loaded_config.mode.auto_listen = true;
                 eprintln!("[PRE-LOG] Preserved auto-listen override from command line.");
             }


            *config = loaded_config; // Overwrite existing config with loaded, potentially merged, config
            config.config_file_path = Some(config_path.to_string());
            eprintln!("[PRE-LOG] Successfully loaded and applied configuration from {}", config_path);
        },
        Err(e) => {
            // If a config file was specified but failed to load, it's a fatal error.
            eprintln!("[PRE-LOG] ERROR: Failed to load configuration from '{}': {}", config_path, e);
            eprintln!("[PRE-LOG] Please check the configuration file path and syntax.");
            return Err(anyhow!("Configuration file loading failed for path: {}", config_path).context(e));
        }
    }
    Ok(())
}
