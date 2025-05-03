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
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// Lightweight struct for passing logging context to spawned tasks
#[derive(Clone)]
struct AgentLogContext {
    repl_log_file: Option<std::path::PathBuf>,
    agent_id: String,
    bind_address: String,
    port: u16,
    // Add fields needed by methods called within the log context
    current_session_id: Option<String>,
    session_tasks: Arc<DashMap<String, Vec<String>>>,
}

impl AgentLogContext {
    // Replicate log_agent_action for the context struct
    async fn log_agent_action(&self, action_type: &str, details: &str) {
         if let Some(log_path) = &self.repl_log_file {
            let timestamp = Utc::now().to_rfc3339();
            let log_entry = format!(
                "{} [{}@{}:{}] {}: {}\n",
                timestamp, self.agent_id, self.bind_address, self.port, action_type, details.trim()
            );
            match OpenOptions::new().create(true).append(true).open(log_path).await {
                Ok(mut file) => {
                    let _ = file.write_all(log_entry.as_bytes()).await;
                    let _ = file.flush().await;
                }
                Err(e) => eprintln!("⚠️ Error opening/writing log in AgentLogContext: {}", e),
            }
        }
    }

     // Replicate save_task_to_history for the context struct
     async fn save_task_to_history(&self, task: Task) -> Result<()> {
        if let Some(session_id) = &self.current_session_id {
            if let Some(mut tasks) = self.session_tasks.get_mut(session_id) {
                self.log_agent_action("SESSION_SAVE_TASK", &format!("Adding task {} to session {}", task.id, session_id)).await;
                tasks.push(task.id.clone());
            } else {
                 self.log_agent_action("SESSION_SAVE_TASK_WARN", &format!("Session {} not found in map while trying to save task {}", session_id, task.id)).await;
            }
        } else {
             self.log_agent_action("SESSION_SAVE_TASK_WARN", &format!("No active session while trying to save task {}", task.id)).await;
        }
        Ok(())
    }
}


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
        }).unwrap_or_default(); // Handle case where history is None


        if task_text.is_empty() {
            // Maybe check task.message if history is empty? Assuming Task might have a top-level message.
            // For now, default to Local execution with the 'echo' tool if history is empty/None.
            warn!("Task history is empty, defaulting to local execution with 'echo' tool.");
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

        // Get the routing decision from the LLM
        let decision_result = self.llm.complete(&routing_prompt).await;

        // Map anyhow::Error from LLM to ServerError::Internal
        let decision = decision_result
            .map_err(|e| ServerError::Internal(format!("LLM routing decision failed: {}", e)))?
            .trim()
            .to_string();


        // Parse the decision
        if decision == "LOCAL" {
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
                         debug!("LLM chose tool: {}", tool_name);
                         tool_name // Use the valid tool chosen by LLM
                    } else {
                         warn!("LLM chose an unknown/disabled tool ('{}'). Falling back to 'echo'. Enabled: [{}]", tool_name, tool_list_str);
                         "echo".to_string() // Fallback to echo if choice is invalid
                    }
                 },
                 Err(e) => {
                     warn!("LLM failed to choose a tool: {}. Falling back to 'echo'.", e);
                     "echo".to_string() // Fallback to echo on error
                 }
            };

            debug!("Final tool decision: {}", chosen_tool_name);
            Ok(RoutingDecision::Local { tool_names: vec![chosen_tool_name] })
            // --- End LLM Tool Selection Logic ---

        } else if decision.starts_with("REMOTE: ") {
            let agent_id = decision.strip_prefix("REMOTE: ").unwrap().trim().to_string();

            // Verify the agent exists in the local directory
            if self.directory.get_agent(&agent_id).is_none() {
                 warn!("LLM decided to delegate to unknown agent '{}', falling back to local execution with 'echo' tool.", agent_id);
                 // Fall back to local if agent not found, using echo tool
                 Ok(RoutingDecision::Local { tool_names: vec!["echo".to_string()] })
            } else {
                 debug!("Routing decision: Remote delegation to agent '{}'", agent_id);
                 Ok(RoutingDecision::Remote { agent_id })
            }
        } else {
            warn!("LLM routing decision was unclear ('{}'), falling back to local execution with 'echo' tool.", decision);
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
             return Err(anyhow!("No LLM configuration provided. Set CLAUDE_API_KEY environment variable or add claude_api_key to config file.")); // Or handle differently
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

        // Create the streaming service
        let streaming_service = Arc::new(StreamingService::new(task_repository.clone()));

        // Create the notification service (pass repository)
        let notification_service = Arc::new(NotificationService::new(task_repository.clone()));

        // Initialize an A2A client if target URL is provided
        let client = if let Some(target_url) = &config.client.target_url {
            info!("Initializing A2A client for target URL: {}", target_url);
            Some(A2aClient::new(target_url))
        } else {
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
        }; // <-- Construct the agent instance here

        // Log agent initialization attempt *after* construction, *before* returning Ok()
        if let Some(log_path) = &agent.repl_log_file {
             let timestamp = Utc::now().to_rfc3339();
             let log_entry = format!(
                 "{} [{}@{}:{}] {}: {}\n",
                 timestamp, agent.agent_id, agent.bind_address, agent.port,
                 "INIT", "Agent initialized."
             );
             // Best effort sync write during init
             let _ = std::fs::OpenOptions::new()
                 .create(true)
                 .append(true)
                 .open(log_path)
                 .and_then(|mut file| file.write_all(log_entry.as_bytes()));
        }

        Ok(agent) // Return the constructed agent
    }

    /// Create a session on demand so remote tasks can be grouped
    fn ensure_session(&mut self) {
        if self.current_session_id.is_none() {
            let session_id = self.create_new_session(); // existing helper
            // Log session creation (moved from create_new_session due to async context)
            // Use tokio::spawn for fire-and-forget logging if needed, or make ensure_session async
            // For simplicity, we'll log synchronously here, assuming it's called from an async context
            // where blocking briefly is acceptable (like the REPL loop).
            // If called from a highly concurrent context, consider spawning the log action.
            let log_path = self.repl_log_file.clone();
            let agent_id = self.agent_id.clone();
            let bind_address = self.bind_address.clone();
            let port = self.port;
            tokio::spawn(async move { // Spawn the logging to avoid blocking
                if let Some(log_path) = log_path {
                    let timestamp = Utc::now().to_rfc3339();
                    let log_entry = format!(
                        "{} [{}@{}:{}] {}: {}\n",
                        timestamp, agent_id, bind_address, port,
                        "SESSION_ENSURE", &format!("Created new session on demand: {}", session_id)
                    );
                    // Use async file operations within the spawned task
                    match OpenOptions::new().create(true).append(true).open(&log_path).await {
                        Ok(mut file) => {
                            let _ = file.write_all(log_entry.as_bytes()).await;
                            let _ = file.flush().await;
                        }
                        Err(e) => eprintln!("⚠️ Error opening/writing log in ensure_session: {}", e),
                    }
                }
            });
        }
    }


    /// Process a message (Example: could be used for direct interaction or testing)
    pub async fn process_message_directly(&self, message_text: &str) -> Result<String> {
        self.log_agent_action("PROCESS_MSG_START", &format!("Received message: '{}'", message_text)).await;

        // Check if we're continuing an existing task that is in InputRequired state
        let mut continue_task_id = None;
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
                                self.log_agent_action("PROCESS_MSG_INFO", &format!("Found task {} in InputRequired state. Continuing.", last_task_id)).await;
                                // We should continue this task instead of creating a new one
                                continue_task_id = Some(last_task_id.clone());
                            }
                        }
                        Err(e) => {
                             self.log_agent_action("PROCESS_MSG_WARN", &format!("Failed to get last task {}: {}", last_task_id, e)).await;
                        }
                    }
                }
            }
        }
    
        // Create a unique task ID if not continuing an existing task
        let task_id = if let Some(id) = continue_task_id {
            id
        } else {
            let new_id = Uuid::new_v4().to_string();
            self.log_agent_action("PROCESS_MSG_INFO", &format!("Creating new task with ID: {}", new_id)).await;
            new_id
        };

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
        self.log_agent_action("PROCESS_MSG_INFO", &format!("Calling task_service.process_task for task ID: {}", task_id)).await;
        // Use task_service to process the task
        let task_result = self.task_service.process_task(params).await;

        let task = match task_result {
             Ok(t) => {
                 self.log_agent_action("PROCESS_MSG_SUCCESS", &format!("Task {} processed. State: {:?}", t.id, t.status.state)).await;
                 t
             }
             Err(e) => {
                 self.log_agent_action("PROCESS_MSG_ERROR", &format!("Failed to process task {}: {}", task_id, e)).await;
                 return Err(anyhow!("Failed to process task: {}", e));
             }
        };

        // Save task to history
        self.save_task_to_history(task.clone()).await?; // save_task_to_history already logs
        // Extract response from task
        let mut response = self.extract_text_from_task(&task);
        
        // If the task is in InputRequired state, indicate that in the response
        if task.status.state == TaskState::InputRequired {
            self.log_agent_action("PROCESS_MSG_INFO", &format!("Task {} requires more input.", task.id)).await;
            response.push_str("\n\n[The agent needs more information. Your next message will continue this task.]");
        }

        self.log_agent_action("PROCESS_MSG_END", &format!("Responding for task {}: '{}'", task.id, response)).await;
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
    pub async fn run_repl(&mut self) -> Result<()> {
        self.log_agent_action("REPL_START", "Entering REPL mode.").await;
        // Print the initial help message
        self.print_repl_help();

        // --- Spawn Background Task for Initial Connection ---
        if let Some(initial_url) = self.client_url() {
            println!("Configured target URL: {}. Attempting connection in background...", initial_url);
            // Clone necessary data for the background task
            let agent_id = self.agent_id.clone();
            let bind_address = self.bind_address.clone();
            let port = self.port;
            let repl_log_file = self.repl_log_file.clone();
            let agent_directory = self.agent_directory.clone();
            let known_servers = self.known_servers.clone(); // Clone Arc<DashMap>

            tokio::spawn(async move {
                // Clone data needed by the closure *once* before defining it.
                let log_closure_repl_log_file = repl_log_file.clone();
                let log_closure_agent_id = agent_id.clone();
                let log_closure_bind_address = bind_address.clone();
                // port is Copy, no need to clone explicitly

                // Define the helper closure as a function that returns a Future
                // Make it Clone so we can use it multiple times
                let log_action = {
                    // Create a wrapper to create a new future each time
                    let log_closure_repl_log_file = log_closure_repl_log_file.clone();
                    let log_closure_agent_id = log_closure_agent_id.clone();
                    let log_closure_bind_address = log_closure_bind_address.clone();
                    let port = port;
                    
                    move |action_type: String, details: String| {
                        let log_closure_repl_log_file = log_closure_repl_log_file.clone();
                        let log_closure_agent_id = log_closure_agent_id.clone();
                        let log_closure_bind_address = log_closure_bind_address.clone();
                        
                        async move {
                            // Use the cloned variables
                            if let Some(log_path) = &log_closure_repl_log_file {
                                let timestamp = Utc::now().to_rfc3339();
                                // Use captured clones and moved String args in format!
                                let log_entry = format!(
                                    "{} [{}@{}:{}] {}: {}\n",
                                    timestamp, log_closure_agent_id, log_closure_bind_address, port, action_type, details.trim()
                                );
                                // Open the file asynchronously
                                match OpenOptions::new()
                                    .create(true)
                                    .append(true)
                                    .open(log_path)
                                    .await
                                {
                                    Ok(mut file) => {
                                        // Write and flush asynchronously
                                        if let Err(e) = file.write_all(log_entry.as_bytes()).await {
                                            eprintln!("⚠️ Background log write error: {}", e);
                                        } else if let Err(e) = file.flush().await {
                                            eprintln!("⚠️ Background log flush error: {}", e);
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("⚠️ Background log open error: {}", e);
                                    }
                                }
                            }
                        }
                    }
                };

                // Pass owned Strings to log_action
                log_action("BG_INIT_CONNECT".to_string(), format!("Starting background connection attempt to {}", initial_url)).await;

                let max_retries = 5;
                let retry_delay = std::time::Duration::from_secs(2);
                let mut connected = false;

                // Create a *new* client instance specifically for this background task
                let mut bg_client = A2aClient::new(&initial_url);

                for attempt in 1..=max_retries {
                    match bg_client.get_agent_card().await { // Use the new client
                        Ok(card) => {
                            let success_msg = format!("Background connection successful to {} ({})", card.name, initial_url);
                            println!("✅ {}", success_msg); // Print to console as well
                            // Pass owned Strings
                            log_action("BG_INIT_CONNECT_SUCCESS".to_string(), success_msg).await;

                            // Update shared state
                            known_servers.insert(initial_url.clone(), card.name.clone());
                            agent_directory.add_or_update_agent(card.name.clone(), card.clone()); // Use name as ID here

                            connected = true;
                            break; // Exit retry loop on success
                        }
                        Err(e) => {
                            let error_msg = format!("Background connection attempt {}/{} to {} failed: {}", attempt, max_retries, initial_url, e);
                            // Don't print every failure to console, only log it
                            // Pass owned Strings
                            log_action("BG_INIT_CONNECT_FAIL".to_string(), error_msg).await;
                            if attempt < max_retries {
                                // Pass owned Strings
                                log_action("BG_INIT_CONNECT_RETRY".to_string(), format!("Retrying in {} seconds...", retry_delay.as_secs())).await;
                                tokio::time::sleep(retry_delay).await;
                            }
                        }
                    }
                }

                if !connected {
                    let final_error_msg = format!("Background connection to {} failed after {} attempts.", initial_url, max_retries);
                    println!("❌ {}", final_error_msg); // Print final failure to console
                    // Pass owned Strings
                    log_action("BG_INIT_CONNECT_FAIL".to_string(), final_error_msg).await;
                }
            });
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
            match tokio::time::timeout(std::time::Duration::from_secs(2), rx).await {
                Ok(Ok(Ok(()))) => {
                    server_running = true;
                    println!("✅ Server started on http://{}:{}", self.bind_address, port);
                    println!("The server will run until you exit the REPL or send :stop");
                },
                Ok(Ok(Err(e))) => {
                    println!("❌ Error starting server: {}", e);
                    println!("The server could not be started. Try a different port or check for other services using this port.");
                    
                    // Clean up the token since the server didn't start
                    server_shutdown_token = None;
                },
                Ok(Err(_)) => {
                    println!("❌ Error: Server initialization failed due to channel error");
                    server_shutdown_token = None;
                },
                Err(_) => {
                    println!("❌ Timeout waiting for server to start");
                    println!("The server is taking too long to start. It might be starting in the background or could have failed.");
                    println!("You can try :stop to cancel any server processes that might be running.");
                    
                    // Since we're not sure if the server started, keep the running flag true
                    // so the user can try to stop it
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
                println!("EOF detected. Exiting REPL.");
                self.log_agent_action("REPL_EOF", "EOF detected.").await;
                // Perform cleanup similar to :quit
                if let Some(token) = server_shutdown_token.take() {
                    println!("Shutting down server...");
                    self.log_agent_action("REPL_CMD_QUIT", "Shutting down server due to EOF.").await;
                    token.cancel();
                }
                break; // Exit loop
            }

            let input_trimmed = input.trim();

            // Log user input before processing (use new function name)
            self.log_agent_action("REPL_INPUT", input_trimmed).await;

            if input_trimmed.is_empty() {
                continue;
            }

            if input_trimmed.starts_with(":") {
                // Log the command itself (use new function name)
                self.log_agent_action("REPL_COMMAND", input_trimmed).await;

                // Handle special commands using input_trimmed
                if input_trimmed == ":help" {
                    self.print_repl_help();
                } else if input_trimmed == ":quit" {
                    println!("Exiting REPL. Goodbye!");
                    self.log_agent_action("REPL_CMD_QUIT", "User initiated quit.").await;

                    // Shutdown server if running
                    if let Some(token) = server_shutdown_token.take() {
                        println!("Shutting down server...");
                        self.log_agent_action("REPL_CMD_QUIT", "Shutting down server.").await;
                        token.cancel();
                    }
                    break; // Exit loop
                } else if input_trimmed == ":card" {
                    let card = self.create_agent_card(); // create_agent_card logs internally now
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
                    // List known servers from the shared map
                    if self.known_servers.is_empty() {
                        println!("No known servers. Connect to a server or wait for background connection attempt.");
                    } else {
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
                    // Disconnect from current server
                    if self.client.is_some() {
                        let url = self.client_url().unwrap_or_else(|| "unknown".to_string());
                        self.client = None;
                        self.log_agent_action("REPL_CMD_DISCONNECT", &format!("Disconnected from {}", url)).await;
                        println!("🔌 Disconnected from {}", url);
                    } else {
                        self.log_agent_action("REPL_CMD_DISCONNECT", "Attempted disconnect but not connected.").await;
                        println!("Not connected to any server");
                    }
                } else if input_trimmed.starts_with(":connect ") {
                    let target = input_trimmed.trim_start_matches(":connect ").trim();

                    // Check if it's a number (referring to a server in the list)
                    if let Ok(server_idx) = target.parse::<usize>() {
                        // Read from shared known_servers map
                        let mut server_list: Vec<(String, String)> = self.known_servers.iter()
                            .map(|entry| (entry.value().clone(), entry.key().clone())) // (Name, URL)
                            .collect();
                        server_list.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by name to match display order

                        if server_idx > 0 && server_idx <= server_list.len() {
                            let (name, url) = &server_list[server_idx - 1];

                            // Create a new client with the selected URL
                            self.client = Some(A2aClient::new(url));
                            self.log_agent_action("REPL_CMD_CONNECT", &format!("Connecting to known server {} ({})", name, url)).await;
                            println!("🔗 Connected to {}: {}", name, url);
                            // Attempt to get card after connecting
                            match self.get_remote_agent_card().await {
                                Ok(card) => {
                                    self.log_agent_action("REPL_CMD_CONNECT", &format!("Successfully got card from {}", card.name)).await;
                                    self.agent_directory.add_or_update_agent(card.name.clone(), card.clone());
                                    println!("📇 Remote agent: {}", card.name);
                                }
                                Err(e) => {
                                    self.log_agent_action("REPL_CMD_CONNECT_WARN", &format!("Connected, but failed to get card from {}: {}", url, e)).await;
                                    println!("⚠️ Could not retrieve agent card after connecting: {}", e);
                                }
                            }
                        } else {
                            self.log_agent_action("REPL_CMD_CONNECT_FAIL", &format!("Invalid server number: {}", server_idx)).await;
                            println!("❌ Error: Invalid server number. Use :servers to see available servers.");
                        }
                    } else {
                        // Treat as URL
                        if target.is_empty() {
                            self.log_agent_action("REPL_CMD_CONNECT_FAIL", "No URL provided.").await;
                            println!("❌ Error: No URL provided. Use :connect URL");
                            continue;
                        }

                        self.log_agent_action("REPL_CMD_CONNECT", &format!("Attempting connection to URL: {}", target)).await;
                        // Create a new client with the provided URL, but don't assume connection will succeed
                        let client = A2aClient::new(target);

                        // Try to get the agent card to verify connection
                        let connect_result = match client.get_agent_card().await {
                            Ok(card) => {
                                self.log_agent_action("REPL_CMD_CONNECT_SUCCESS", &format!("Successfully connected to agent '{}' at {}", card.name, target)).await;
                                println!("✅ Successfully connected to agent: {}", card.name);

                                // Add/Update known servers map
                                self.known_servers.insert(target.to_string(), card.name.clone());

                                // Store the client since connection was successful
                                self.client = Some(client);
                                
                                // IMPORTANT: Add the agent to the agent directory for routing
                                // Use the agent name as the ID
                                self.agent_directory.add_or_update_agent(card.name.clone(), card.clone());
                                self.log_agent_action("REPL_CMD_CONNECT", &format!("Added agent '{}' to local directory for routing.", card.name)).await;

                                // This sets up the bidirectional agent so it can route tasks to this agent
                                println!("🔄 Added agent to directory for task routing");

                                true // Indicate success
                            },
                            Err(e) => {
                                self.log_agent_action("REPL_CMD_CONNECT_FAIL", &format!("Failed to connect to agent at {}: {}", target, e)).await;
                                println!("❌ Failed to connect to agent at {}: {}", target, e);
                                println!("Please check that the server is running and the URL is correct.");
                                false // Indicate failure
                            }
                        };
                        
                        // Ask to add to known servers only if connection failed
                        if !connect_result {
                            // Check if it's already in the list (maybe added by background task)
                            if !self.known_servers.contains_key(target) {
                                println!("Connection failed. Add this URL to the known servers list anyway? (y/n)");
                                let mut answer = String::new();
                                // Use a fresh stdin lock for this interaction
                                let stdin_temp = io::stdin();
                                let mut reader_temp = stdin_temp.lock();
                                reader_temp.read_line(&mut answer).unwrap_or_default();

                                if answer.trim().to_lowercase() == "y" {
                                    self.known_servers.insert(target.to_string(), "Unknown Agent".to_string());
                                    println!("Added URL to known servers list as 'Unknown Agent'.");
                                }
                            } else {
                                 println!("Connection failed, but URL is already in the known servers list.");
                            }
                        }
                    }
                } else if input_trimmed.starts_with(":listen ") {
                    let port_str = input_trimmed.trim_start_matches(":listen ").trim();

                    // Check if already running
                    if server_running {
                        self.log_agent_action("REPL_CMD_LISTEN_WARN", "Attempted to listen while server already running.").await;
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
                            self.log_agent_action("REPL_CMD_LISTEN", &format!("Attempting to start server on port {}.", port)).await;
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
                                        // Send success status back to REPL
                                        let _ = tx.send(Ok(()));
                                        
                                        // Wait for the server to complete or be cancelled
                                        match handle.await {
                                            Ok(()) => {
                                                info!("Server shut down gracefully.");
                                                // Cannot log using self.log_agent_action here as self might be dropped
                                            }
                                            Err(e) => {
                                                 error!("Server error: {}", e);
                                                 // Cannot log using self.log_agent_action here
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        // Send error status back to REPL
                                        let error_msg = format!("Failed to start server: {}", e);
                                        let _ = tx.send(Err(error_msg.clone()));
                                        error!("{}", error_msg); // Log to tracing
                                        // Cannot log using self.log_agent_action here
                                    }
                                }
                            });
                            
                            // Wait for the server start status
                            // Use a short timeout to avoid blocking the REPL if something goes wrong
                            match tokio::time::timeout(std::time::Duration::from_secs(3), rx).await { // Increased timeout slightly
                                Ok(Ok(Ok(()))) => {
                                    server_running = true;
                                    self.log_agent_action("REPL_CMD_LISTEN_SUCCESS", &format!("Server started successfully on port {}.", port)).await;
                                    println!("✅ Server started on http://{}:{}", self.bind_address, port);
                                    println!("The server will run until you exit the REPL or send :stop");
                                },
                                Ok(Ok(Err(e))) => {
                                    self.log_agent_action("REPL_CMD_LISTEN_FAIL", &format!("Error starting server on port {}: {}", port, e)).await;
                                    println!("❌ Error starting server: {}", e);
                                    println!("The server could not be started. Try a different port or check for other services using this port.");
                                    server_shutdown_token = None; // Clean up token
                                },
                                Ok(Err(channel_err)) => {
                                    self.log_agent_action("REPL_CMD_LISTEN_FAIL", &format!("Server init channel error on port {}: {}", port, channel_err)).await;
                                    println!("❌ Error: Server initialization failed due to channel error");
                                    server_shutdown_token = None; // Clean up token
                                },
                                Err(timeout_err) => {
                                    self.log_agent_action("REPL_CMD_LISTEN_TIMEOUT", &format!("Timeout waiting for server on port {}: {}", port, timeout_err)).await;
                                    println!("❌ Timeout waiting for server to start");
                                    println!("The server is taking too long to start. It might be starting in the background or could have failed.");
                                    println!("You can try :stop to cancel any server processes that might be running.");
                                    // Keep running flag true so user can try :stop
                                    server_running = true;
                                }
                            }
                        },
                        Err(parse_err) => {
                            self.log_agent_action("REPL_CMD_LISTEN_FAIL", &format!("Invalid port format '{}': {}", port_str, parse_err)).await;
                            println!("❌ Error: Invalid port number. Please provide a valid port.");
                        }
                    }
                } else if input_trimmed == ":stop" {
                    // Stop the server if running
                    if let Some(token) = server_shutdown_token.take() {
                        self.log_agent_action("REPL_CMD_STOP", "Stopping server.").await;
                        println!("Shutting down server...");
                        token.cancel();
                        server_running = false;
                        println!("✅ Server stopped");
                    } else {
                        self.log_agent_action("REPL_CMD_STOP_WARN", "Attempted to stop server, but none was running.").await;
                        println!("⚠️ No server currently running");
                    }
                } else if input_trimmed.starts_with(":remote ") {
                    let message = input_trimmed.trim_start_matches(":remote ").trim();
                    if message.is_empty() {
                        self.log_agent_action("REPL_CMD_REMOTE_FAIL", "No message provided.").await;
                        println!("❌ Error: No message provided. Use :remote MESSAGE");
                        continue;
                    }

                    // Check if we're connected to a remote agent
                    if self.client.is_none() {
                        self.log_agent_action("REPL_CMD_REMOTE_FAIL", "Not connected to a remote agent.").await;
                        println!("❌ Error: Not connected to a remote agent. Use :connect URL first.");
                        continue;
                    }

                    // Send task to remote agent
                    self.log_agent_action("REPL_CMD_REMOTE", &format!("Sending task to remote: '{}'", message)).await;
                    println!("📤 Sending task to remote agent: '{}'", message);
                    match self.send_task_to_remote(message).await { // send_task_to_remote logs internally
                        Ok(task) => {
                            self.log_agent_action("REPL_CMD_REMOTE_SUCCESS", &format!("Task {} sent successfully. State: {:?}", task.id, task.status.state)).await;
                            println!("✅ Task sent successfully!");
                            println!("Task ID: {}", task.id);
                            println!("Initial state: {:?}", task.status.state);
                            
                            // If we have a completed task with history, show the response
                            if task.status.state == TaskState::Completed && task.history.is_some() {
                                if let Some(history) = task.history {
                                    let response = history.iter()
                                        .filter(|m| m.role == Role::Agent)
                                        .flat_map(|m| m.parts.iter())
                                        .filter_map(|p| match p {
                                            Part::TextPart(tp) => Some(tp.text.clone()),
                                            _ => None,
                                        })
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    
                                    if !response.is_empty() {
                                        println!("\n📥 Response from remote agent:");
                                        println!("{}", response);
                                    }
                                }
                            }
                        },
                        Err(e) => {
                            self.log_agent_action("REPL_CMD_REMOTE_FAIL", &format!("Error sending task: {}", e)).await;
                            println!("❌ Error sending task: {}", e);
                        }
                    }
                } else if input_trimmed == ":session new" {
                    let session_id = self.create_new_session(); // create_new_session logs internally
                    println!("✅ Created new session: {}", session_id);
                } else if input_trimmed == ":session show" {
                    if let Some(session_id) = &self.current_session_id {
                        self.log_agent_action("REPL_CMD_SESSION_SHOW", &format!("Current session ID: {}", session_id)).await;
                        println!("🔍 Current session: {}", session_id);
                    } else {
                        self.log_agent_action("REPL_CMD_SESSION_SHOW", "No active session.").await;
                        println!("⚠️ No active session. Use :session new to create one.");
                    }
                } else if input_trimmed == ":history" {
                    if let Some(session_id) = &self.current_session_id {
                        self.log_agent_action("REPL_CMD_HISTORY", &format!("Fetching history for session {}", session_id)).await;
                        let tasks = self.get_current_session_tasks().await?; // get_current_session_tasks logs internally
                        if tasks.is_empty() {
                            self.log_agent_action("REPL_CMD_HISTORY", "No messages in current session.").await;
                            println!("📭 No messages in current session.");
                        } else {
                            self.log_agent_action("REPL_CMD_HISTORY", &format!("Displaying {} tasks for session {}", tasks.len(), session_id)).await;
                            println!("\n📝 Session History:");
                            for (i, task) in tasks.iter().enumerate() {
                                if let Some(history) = &task.history {
                                    for message in history {
                                        let role_icon = match message.role {
                                            Role::User => "👤",
                                            Role::Agent => "🤖",
                                            _ => "➡️",
                                        };
                                        
                                        // Extract text from parts
                                        let text = message.parts.iter()
                                            .filter_map(|p| match p {
                                                Part::TextPart(tp) => Some(tp.text.clone()),
                                                _ => None,
                                            })
                                            .collect::<Vec<_>>()
                                            .join("\n");
                                        
                                        // Truncate long messages for display
                                        let display_text = if text.len() > 100 {
                                            format!("{}...", &text[..97])
                                        } else {
                                            text
                                        };
                                        
                                        println!("{} {}: {}", role_icon, message.role, display_text);
                                    }
                                }
                            }
                        }
                    } else {
                        self.log_agent_action("REPL_CMD_HISTORY", "No active session.").await;
                        println!("⚠️ No active session. Use :session new to create one.");
                    }
                } else if input_trimmed == ":tasks" {
                    if let Some(session_id) = &self.current_session_id {
                        self.log_agent_action("REPL_CMD_TASKS", &format!("Fetching tasks for session {}", session_id)).await;
                        let tasks = self.get_current_session_tasks().await?; // get_current_session_tasks logs internally
                        if tasks.is_empty() {
                            self.log_agent_action("REPL_CMD_TASKS", "No tasks in current session.").await;
                            println!("📭 No tasks in current session.");
                        } else {
                            self.log_agent_action("REPL_CMD_TASKS", &format!("Displaying {} tasks for session {}", tasks.len(), session_id)).await;
                            println!("\n📋 Tasks in Current Session:");
                            for (i, task) in tasks.iter().enumerate() {
                                println!("  {}. {} - Status: {:?}", i + 1, task.id, task.status.state);
                            }
                        }
                    } else {
                        self.log_agent_action("REPL_CMD_TASKS", "No active session.").await;
                        println!("⚠️ No active session. Use :session new to create one.");
                    }
                } else if input_trimmed.starts_with(":task ") {
                    let task_id = input_trimmed.trim_start_matches(":task ").trim();
                    if task_id.is_empty() {
                        self.log_agent_action("REPL_CMD_TASK_FAIL", "No task ID provided.").await;
                        println!("❌ Error: No task ID provided. Use :task TASK_ID");
                    } else {
                        self.log_agent_action("REPL_CMD_TASK", &format!("Fetching details for task {}", task_id)).await;
                        // Get task details
                        let params = TaskQueryParams {
                            id: task_id.to_string(),
                            history_length: None,
                            metadata: None,
                        };
                        
                        match self.task_service.get_task(params).await {
                            Ok(task) => {
                                self.log_agent_action("REPL_CMD_TASK_SUCCESS", &format!("Displaying details for task {}", task.id)).await;
                                println!("\n🔍 Task Details:");
                                println!("  ID: {}", task.id);
                                println!("  Status: {:?}", task.status.state);
                                println!("  Session: {}", task.session_id.unwrap_or_else(|| "None".to_string()));
                                println!("  Timestamp: {}", task.status.timestamp.map(|t| t.to_rfc3339()).unwrap_or_else(|| "None".to_string()));
                                
                                // Show artifacts count if any
                                if let Some(artifacts) = &task.artifacts {
                                    println!("  Artifacts: {} (use :artifacts {} to view)", artifacts.len(), task.id);
                                } else {
                                    println!("  Artifacts: None");
                                }
                                
                                // Show last message if any
                                if let Some(message) = &task.status.message {
                                    let text = message.parts.iter()
                                        .filter_map(|p| match p {
                                            Part::TextPart(tp) => Some(tp.text.clone()),
                                            _ => None,
                                        })
                                        .collect::<Vec<_>>()
                                        .join("\n");
                                    
                                    println!("\n  Last Message: {}", text);
                                }
                            },
                            Err(e) => {
                                self.log_agent_action("REPL_CMD_TASK_FAIL", &format!("Failed to get task {}: {}", task_id, e)).await;
                                println!("❌ Error: Failed to get task: {}", e);
                            }
                        }
                    }
                } else if input_trimmed.starts_with(":artifacts ") {
                    let task_id = input_trimmed.trim_start_matches(":artifacts ").trim();
                    if task_id.is_empty() {
                        self.log_agent_action("REPL_CMD_ARTIFACTS_FAIL", "No task ID provided.").await;
                        println!("❌ Error: No task ID provided. Use :artifacts TASK_ID");
                    } else {
                        self.log_agent_action("REPL_CMD_ARTIFACTS", &format!("Fetching artifacts for task {}", task_id)).await;
                        // Get task details with artifacts
                        let params = TaskQueryParams {
                            id: task_id.to_string(),
                            history_length: None,
                            metadata: None,
                        };
                        
                        match self.task_service.get_task(params).await {
                            Ok(task) => {
                                if let Some(artifacts) = &task.artifacts {
                                    if artifacts.is_empty() {
                                        self.log_agent_action("REPL_CMD_ARTIFACTS", &format!("No artifacts found for task {}", task.id)).await;
                                        println!("📦 No artifacts for task {}", task.id);
                                    } else {
                                        self.log_agent_action("REPL_CMD_ARTIFACTS_SUCCESS", &format!("Displaying {} artifacts for task {}", artifacts.len(), task.id)).await;
                                        println!("\n📦 Artifacts for Task {}:", task.id);
                                        for (i, artifact) in artifacts.iter().enumerate() {
                                            println!("  {}. {} ({})", i + 1, 
                                                     artifact.name.clone().unwrap_or_else(|| format!("Artifact {}", artifact.index)),
                                                     artifact.description.clone().unwrap_or_else(|| "No description".to_string()));
                                        }
                                    }
                                } else {
                                    self.log_agent_action("REPL_CMD_ARTIFACTS", &format!("No artifacts found for task {}", task.id)).await;
                                    println!("📦 No artifacts for task {}", task.id);
                                }
                            },
                            Err(e) => {
                                self.log_agent_action("REPL_CMD_ARTIFACTS_FAIL", &format!("Failed to get task {} for artifacts: {}", task_id, e)).await;
                                println!("❌ Error: Failed to get task: {}", e);
                            }
                        }
                    }
                } else if input_trimmed.starts_with(":cancelTask ") {
                    let task_id = input_trimmed.trim_start_matches(":cancelTask ").trim();
                    if task_id.is_empty() {
                        self.log_agent_action("REPL_CMD_CANCEL_FAIL", "No task ID provided.").await;
                        println!("❌ Error: No task ID provided. Use :cancelTask TASK_ID");
                    } else {
                        self.log_agent_action("REPL_CMD_CANCEL", &format!("Attempting to cancel task {}", task_id)).await;
                        // Cancel the task
                        let params = TaskIdParams {
                            id: task_id.to_string(),
                            metadata: None,
                        };
                        
                        match self.task_service.cancel_task(params).await {
                            Ok(task) => {
                                self.log_agent_action("REPL_CMD_CANCEL_SUCCESS", &format!("Successfully canceled task {}. State: {:?}", task.id, task.status.state)).await;
                                println!("✅ Successfully canceled task {}", task.id);
                                println!("  Current state: {:?}", task.status.state);
                            },
                            Err(e) => {
                                self.log_agent_action("REPL_CMD_CANCEL_FAIL", &format!("Failed to cancel task {}: {}", task_id, e)).await;
                                println!("❌ Error: Failed to cancel task: {}", e);
                            }
                        }
                    }
                } else {
                    self.log_agent_action("REPL_UNKNOWN_COMMAND", input_trimmed).await;
                    println!("❌ Unknown command: {}", input_trimmed);
                    println!("Type :help for a list of commands");
                }
            } else {
                // Process the message directly with the agent (process_message_directly logs internally)
                match self.process_message_directly(input_trimmed).await {
                    Ok(response) => {
                        println!("\n🤖 Agent response:\n{}\n", response);
                        // Log agent response (use new function name)
                        self.log_agent_action("REPL_OUTPUT", &response).await;
                    },
                    Err(e) => {
                        let error_msg = format!("Error processing message: {}", e);
                        println!("❌ {}", error_msg);
                        // Log the error (use new function name)
                        self.log_agent_action("REPL_ERROR", &error_msg).await;
                    }
                }
            }
        }

        self.log_agent_action("REPL_END", "Exiting REPL mode.").await;
        Ok(())
    }

    /// Create a new session
    pub fn create_new_session(&mut self) -> String {
        let session_id = format!("session-{}", Uuid::new_v4());
        self.current_session_id = Some(session_id.clone());
        self.session_tasks.insert(session_id.clone(), Vec::new());
        // Logging removed from here due to lifetime issues with tokio::spawn in sync fn.
        // Logging can be added in the calling async code (e.g., REPL loop) after the call returns.
        session_id
    }

    /// Add task to current session
    async fn save_task_to_history(&self, task: Task) -> Result<()> {
        if let Some(session_id) = &self.current_session_id {
            if let Some(mut tasks) = self.session_tasks.get_mut(session_id) {
                self.log_agent_action("SESSION_SAVE_TASK", &format!("Adding task {} to session {}", task.id, session_id)).await;
                tasks.push(task.id.clone());
            } else {
                 self.log_agent_action("SESSION_SAVE_TASK_WARN", &format!("Session {} not found in map while trying to save task {}", session_id, task.id)).await;
            }
        } else {
             self.log_agent_action("SESSION_SAVE_TASK_WARN", &format!("No active session while trying to save task {}", task.id)).await;
        }
        Ok(())
    }

    /// Get tasks for current session
    pub async fn get_current_session_tasks(&self) -> Result<Vec<Task>> {
        let mut tasks = Vec::new();
        if let Some(session_id) = &self.current_session_id {
            self.log_agent_action("SESSION_GET_TASKS", &format!("Fetching tasks for session {}", session_id)).await;
            if let Some(task_ids) = self.session_tasks.get(session_id) {
                 self.log_agent_action("SESSION_GET_TASKS", &format!("Found {} task IDs in session {}", task_ids.len(), session_id)).await;
                for task_id in task_ids.iter() {
                    self.log_agent_action("SESSION_GET_TASKS_DETAIL", &format!("Fetching details for task {}", task_id)).await;
                    // Use TaskQueryParams to get task with history
                    let params = TaskQueryParams {
                        id: task_id.clone(),
                        history_length: None, // Get full history
                        metadata: None,
                    };

                    match self.task_service.get_task(params).await {
                        Ok(task) => tasks.push(task),
                        Err(e) => {
                            self.log_agent_action("SESSION_GET_TASKS_ERROR", &format!("Failed to get task {}: {}", task_id, e)).await;
                            // Continue trying to get other tasks
                        }
                    }
                }
            } else {
                 self.log_agent_action("SESSION_GET_TASKS_WARN", &format!("Session {} not found in map.", session_id)).await;
            }
        } else {
             self.log_agent_action("SESSION_GET_TASKS_WARN", "No active session.").await;
        }
        Ok(tasks)
    }

    /// Logs agent actions to the configured file
    async fn log_agent_action(&self, action_type: &str, details: &str) {
        if let Some(log_path) = &self.repl_log_file {
            let timestamp = Utc::now().to_rfc3339();
            // Format: Timestamp [AgentID@Host:Port] ACTION: Details
            let log_entry = format!(
                "{} [{}@{}:{}] {}: {}\n",
                timestamp, self.agent_id, self.bind_address, self.port, action_type, details.trim()
            );

            // Use tokio::fs for async file operations
            match OpenOptions::new()
                .create(true) // Create the file if it doesn't exist
                .append(true) // Append to the file
                .open(log_path)
                .await
            {
                Ok(mut file) => {
                    // Use async write_all
                    if let Err(e) = file.write_all(log_entry.as_bytes()).await {
                        // Log error to stderr, don't crash the agent, but also log it to the file if possible
                        let error_msg = format!("Error writing to log file '{}': {}", log_path.display(), e);
                        eprintln!("⚠️ [{}@{}:{}] {}", self.agent_id, self.bind_address, self.port, error_msg);
                        // Avoid infinite loop if logging the error itself fails
                        // We could try logging the error to the file itself, but let's keep it simple for now.
                    }
                    // Ensure buffer is flushed
                    if let Err(e) = file.flush().await {
                         let error_msg = format!("Error flushing log file '{}': {}", log_path.display(), e);
                         eprintln!("⚠️ [{}@{}:{}] {}", self.agent_id, self.bind_address, self.port, error_msg);
                    }
                }
                Err(e) => {
                     let error_msg = format!("Error opening log file '{}': {}", log_path.display(), e);
                     eprintln!("⚠️ [{}@{}:{}] {}", self.agent_id, self.bind_address, self.port, error_msg);
                }
            }
        }
    }

    /// Creates a lightweight clone containing only fields needed for logging within spawned tasks.
    /// This avoids lifetime issues when self is borrowed mutably.
    async fn clone_for_logging(&self) -> AgentLogContext {
        AgentLogContext {
            repl_log_file: self.repl_log_file.clone(),
            agent_id: self.agent_id.clone(),
            bind_address: self.bind_address.clone(),
            port: self.port,
            // Include fields needed by methods called within the log context (like save_task_to_history)
            current_session_id: self.current_session_id.clone(),
            session_tasks: self.session_tasks.clone(),
        }
    }


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
        let agent_id_clone = self.agent_id.clone(); // Clone for the signal handler
        let log_path_clone = self.repl_log_file.clone(); // Clone log path
        let bind_address_clone = self.bind_address.clone();
        let port_clone = self.port;
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.expect("Failed to install CTRL+C handler");
            println!("Received shutdown signal, stopping server...");
            // Log shutdown signal (best effort sync write)
            if let Some(log_path) = log_path_clone {
                 let timestamp = Utc::now().to_rfc3339();
                 let log_entry = format!(
                     "{} [{}@{}:{}] {}: {}\n",
                     timestamp, agent_id_clone, bind_address_clone, port_clone,
                     "SERVER_SHUTDOWN_SIGNAL", "Received Ctrl+C signal."
                 );
                 let _ = std::fs::OpenOptions::new().append(true).open(log_path)
                     .and_then(|mut file| file.write_all(log_entry.as_bytes()));
            }
            shutdown_token_clone.cancel();
        });

        // Create the agent card for this server
        let agent_card = self.create_agent_card();
        
        // Convert to JSON value for the server
        let agent_card_json = serde_json::to_value(agent_card).unwrap_or_else(|e| {
            warn!("Failed to serialize agent card: {}", e);
            warn!("Failed to serialize agent card: {}", e);
            serde_json::json!({})
        });

        self.log_agent_action("SERVER_START", &format!("Attempting to start server on {}:{}", self.bind_address, self.port)).await;
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
                self.log_agent_action("SERVER_START_SUCCESS", &format!("Server started successfully on {}:{}", self.bind_address, self.port)).await;
                handle
            },
            Err(e) => {
                self.log_agent_action("SERVER_START_FAIL", &format!("Failed to start server: {}", e)).await;
                return Err(anyhow!("Failed to start server: {}", e));
            }
        };

        println!("Server running on {}:{}", self.bind_address, self.port);

        // Wait for the server to complete
        let join_result = server_handle.await;
        match join_result {
            Ok(()) => {
                self.log_agent_action("SERVER_SHUTDOWN_GRACEFUL", "Server shut down gracefully.").await;
                info!("Server shut down gracefully."); // Keep tracing info
                Ok(())
            }
            Err(join_err) => {
                self.log_agent_action("SERVER_SHUTDOWN_ERROR", &format!("Server join error: {}", join_err)).await;
                error!("Failed to join server task: {}", join_err); // Keep tracing info
                Err(anyhow!("Server join error: {}", join_err))
            }
        }
    }

    /// Send a task to a remote agent using the A2A client, ensuring session consistency.
    pub async fn send_task_to_remote(&mut self, message: &str) -> Result<Task> {
        // ① Ensure a session exists locally
        self.ensure_session();
        let session_id = self.current_session_id.clone(); // Clone session_id for logging and sending

        let remote_url = self.client_url().unwrap_or_else(|| "unknown".to_string());
        self.log_agent_action(
            "REMOTE_SEND_START",
            &format!("→ {}: '{}' (Session: {:?})", remote_url, message, session_id),
        ).await;

        // Get mutable reference to the client
        let client = self.client.as_mut()
            .ok_or_else(|| {
                let err_msg = "No remote client configured. Use :connect first.";
                // Log the error before returning
                tokio::spawn(self.log_agent_action("REMOTE_SEND_ERROR", err_msg)); // Spawn log action
                anyhow!(err_msg) // Return anyhow error
            })?;

        // ② Send the task with the current session ID
        let task_result = client.send_task(message, session_id.clone()).await; // Pass cloned session_id

        let task = match task_result {
            Ok(t) => t,
            Err(e) => {
                // Log the specific client error
                let error_string = format!("Error sending task to {}: {}", remote_url, e);
                 // Spawn log action as self is borrowed mutably
                let log_clone = self.log_agent_action("REMOTE_SEND_ERROR", &error_string);
                tokio::spawn(log_clone);
                return Err(anyhow!(error_string)); // Return anyhow error
            }
        };

        // ③ Mirror the returned task locally (read-only copy)
        if let Err(e) = self.task_service.import_task(task.clone()).await {
            // Log warning if caching fails, but don't fail the whole operation
            let warn_msg = format!("Could not cache remote task {} locally: {}", task.id, e);
            warn!("{}", warn_msg); // Use tracing::warn
             // Spawn log action
            let log_clone = self.log_agent_action("REMOTE_SEND_CACHE_WARN", &warn_msg);
            tokio::spawn(log_clone);
        } else {
            let log_msg = format!("Cached remote task {} locally.", task.id);
             // Spawn log action
            let log_clone = self.log_agent_action("REMOTE_SEND_CACHE_SUCCESS", &log_msg);
            tokio::spawn(log_clone);
        }

        // ④ Add the task (local mirror) to the current session history
        // Use a clone of self for the logging closure inside the spawned task
        let self_clone_for_log = self.clone_for_logging().await; // Helper needed
        let task_clone_for_log = task.clone();
        tokio::spawn(async move {
            if let Err(e) = self_clone_for_log.save_task_to_history(task_clone_for_log).await {
                 let error_msg = format!("Failed to save remote task {} to session history: {}", task_clone_for_log.id, e);
                 eprintln!("⚠️ {}", error_msg); // Log error to stderr
                 self_clone_for_log.log_agent_action("REMOTE_SEND_HISTORY_ERROR", &error_msg).await;
            }
        });


        // Log final success
        self.log_agent_action(
            "REMOTE_SEND_SUCCESS",
            &format!("Stored remote task {} in session {:?}", task.id, session_id),
        ).await;

        info!("Remote task {} created and linked to session {:?}", task.id, session_id); // Keep tracing info

        Ok(task) // Return the original task received from the remote agent
    }
    
    /// Get capabilities of a remote agent (using A2A client)
    pub async fn get_remote_agent_card(&mut self) -> Result<AgentCard> {
        let remote_url = self.client_url().unwrap_or_else(|| "unknown".to_string());
        self.log_agent_action("REMOTE_GET_CARD_START", &format!("Attempting to get agent card from {}", remote_url)).await;

        // Check if we have a client configured
        if let Some(client) = &mut self.client {
            match client.get_agent_card().await {
                 Ok(agent_card) => {
                    self.log_agent_action("REMOTE_GET_CARD_SUCCESS", &format!("Retrieved agent card for '{}' from {}", agent_card.name, remote_url)).await;
                    info!("Retrieved agent card for: {}", agent_card.name); // Keep tracing info
                    Ok(agent_card)
                 }
                 Err(e) => {
                    self.log_agent_action("REMOTE_GET_CARD_ERROR", &format!("Error retrieving agent card from {}: {}", remote_url, e)).await;
                    Err(anyhow!("Error retrieving agent card: {}", e))
                 }
            }
        } else {
            self.log_agent_action("REMOTE_GET_CARD_ERROR", "No A2A client configured.").await;
            Err(anyhow!("No A2A client configured. Use --target-url to specify a remote agent."))
        }
    }

    /// Create an agent card (matching types.rs structure)
    pub fn create_agent_card(&self) -> AgentCard {
        // Logging removed from here due to lifetime issues with tokio::spawn in sync fn.
        // Logging can be added in the calling async code (e.g., REPL loop) after the call returns.

        // Construct AgentCapabilities based on actual capabilities
        let capabilities = AgentCapabilities {
            push_notifications: true, // Example: Assuming supported
            state_transition_history: true, // Example: Assuming supported
            streaming: false, // Example: Assuming not supported
            // Add other fields from AgentCapabilities if they exist
        };

        AgentCard {
            // id field does not exist on AgentCard in types.rs
            name: self.agent_name.clone(), // Use the configured agent name
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
    // Initialize tracing subscriber
    tracing_subscriber::fmt::init();
    info!("Starting Bidirectional Agent main function."); // Use tracing early

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    
    // Create a default configuration
    let mut config = BidirectionalAgentConfig::default();
    
    // Enable REPL mode by default
    config.mode.repl = true;
    
    // Check for environment variable API key
    if config.llm.claude_api_key.is_none() {
        config.llm.claude_api_key = std::env::var("CLAUDE_API_KEY").ok();
        if config.llm.claude_api_key.is_some() {
            info!("Using Claude API key from environment variable");
        }
    }
    
    // Process command line arguments
    info!("Processing command line arguments: {:?}", &args[1..]);
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        info!("Processing arg: {}", arg);

        if arg == "--listen" || arg == "-l" {
            info!("Found --listen flag.");
            // Enable auto-listen mode
            config.mode.auto_listen = true;
            std::env::set_var("AUTO_LISTEN", "true");
            
            // If a port is provided after the flag, use it
            if i + 1 < args.len() {
                if let Ok(port) = args[i + 1].parse::<u16>() {
                    info!("Overriding auto-listen port with: {}", port);
                    config.server.port = port;
                    i += 1; // Skip the port argument
                } else {
                     warn!("Argument after --listen ('{}') is not a valid port.", args[i+1]);
                }
            }
        } else if arg.starts_with("--port=") {
            info!("Found --port= argument: {}", arg);
            // Parse port from --port=NNNN format
            if let Ok(port) = arg.trim_start_matches("--port=").parse::<u16>() {
                info!("Setting server port to: {}", port);
                config.server.port = port;
            } else {
                warn!("Invalid port format in '{}'. Using default port.", arg);
            }
        } else if arg.contains(':') && !arg.starts_with(":") { // Avoid matching REPL commands like :listen
             info!("Found potential host:port argument: {}", arg);
            // Check if the argument is in the format "server:port"
            let parts: Vec<&str> = arg.split(':').collect();
            if parts.len() == 2 {
                let server = parts[0];
                if let Ok(port) = parts[1].parse::<u16>() {
                    info!("Setting target URL to http://{}:{}", server, port);
                    let server_url = format!("http://{}:{}", server, port);
                    config.client.target_url = Some(server_url);
                } else {
                    warn!("Invalid port number in '{}'. Treating as config file.", arg);
                    // Fall through to treat as config file
                    load_config_from_path(&mut config, arg)?;
                }
            } else {
                warn!("Invalid argument format '{}'. Treating as config file.", arg);
                 // Fall through to treat as config file
                 load_config_from_path(&mut config, arg)?;
            }
        } else {
            // If the argument doesn't match any flag, treat it as a config file path
            load_config_from_path(&mut config, arg)?;
        }
        
        i += 1;
    }

    // Determine final mode after processing all args and potential config files.
    // If no specific action mode was set (via args or config) and REPL wasn't explicitly disabled, default to REPL.
    if config.mode.message.is_none()
        && config.mode.remote_task.is_none()
        && !config.mode.get_agent_card
        && !config.mode.repl // Check if REPL was explicitly set to true in config/args
    {
        // If no arguments were given OR if arguments were given but didn't imply a mode, default to REPL.
        if args.len() <= 1 {
             info!("No arguments provided. Defaulting to REPL mode.");
             config.mode.repl = true;
        } else {
             // Check if any argument *implied* a mode other than REPL (like --listen without a specific action)
             // If arguments were present but didn't set message/remote_task/get_card, assume REPL.
             info!("No specific action mode requested via args/config. Defaulting to REPL mode.");
             config.mode.repl = true;
        }
    } else if args.len() <= 1 && !config.mode.repl {
        // Handles the case where a config file was loaded (args.len() > 1 is false)
        // but didn't specify repl=true or any other mode.
         info!("Config file loaded but no specific mode set. Defaulting to REPL mode.");
         config.mode.repl = true;
    }


    // Log final effective configuration before starting agent/action
    info!("Effective configuration: {:?}", config);

    // Process actions based on config.mode settings
    
    // REPL mode takes precedence
    if config.mode.repl {
        info!("Starting agent in REPL mode.");
        // Create the agent
        let mut agent = BidirectionalAgent::new(config.clone())?;
        
        // Run the REPL
        return agent.run_repl().await;
    }
    
    // Process a single message
    if let Some(message) = &config.mode.message {
        info!("Starting agent to process single message: '{}'", message);
        // Create the agent
        let agent = BidirectionalAgent::new(config.clone())?;

        // Process the message
        println!("Processing message: '{}'", message);
        let response = agent.process_message_directly(message).await?;

        println!("Response:\n{}", response);
        info!("Finished processing single message.");
        return Ok(());
    }
    
    // Send task to remote agent
    if let Some(task_message) = &config.mode.remote_task {
        // Check if we have a target URL
        if config.client.target_url.is_none() {
            error!("Cannot send remote task: No target URL configured.");
            return Err(anyhow!("No target URL configured. Add target_url to the [client] section in config file."));
        }
        
        info!("Starting agent to send remote task: '{}'", task_message);
        // Create the agent
        let mut agent = BidirectionalAgent::new(config.clone())?;
        
        // Send task to remote agent
        println!("Sending task to remote agent: '{}'", task_message);
        let task = agent.send_task_to_remote(task_message).await?;
        
        println!("Task sent successfully!");
        println!("Task ID: {}", task.id);
        println!("Initial state: {:?}", task.status.state);
        
        info!("Finished sending remote task.");
        return Ok(());
    }
    
    // Get remote agent card
    if config.mode.get_agent_card {
        // Check if we have a target URL
        if config.client.target_url.is_none() {
             error!("Cannot get remote card: No target URL configured.");
            return Err(anyhow!("No target URL configured. Add target_url to the [client] section in config file."));
        }
        
        info!("Starting agent to get remote agent card.");
        // Create the agent
        let mut agent = BidirectionalAgent::new(config.clone())?;
        
        // Get remote agent card
        println!("Retrieving agent card from remote agent...");
        let card = agent.get_remote_agent_card().await?;
        
        println!("Remote Agent Card:");
        println!("  Name: {}", card.name);
        println!("  Version: {}", card.version);
        println!("  Description: {}", card.description.as_deref().unwrap_or("None"));
        println!("  URL: {}", card.url);
        println!("  Capabilities:");
        println!("    - Streaming: {}", card.capabilities.streaming);
        println!("    - Push Notifications: {}", card.capabilities.push_notifications);
        println!("    - State Transition History: {}", card.capabilities.state_transition_history);
        println!("  Skills: {}", card.skills.len());
        
        info!("Finished getting remote agent card.");
        return Ok(());
    }
    
    // Default mode: Run the server
    info!("Starting agent in server mode.");
    let agent = BidirectionalAgent::new(config)?;
    agent.run().await
}

/// Helper function to load config from path, used in main arg parsing
fn load_config_from_path(config: &mut BidirectionalAgentConfig, config_path: &str) -> Result<()> {
    info!("Attempting to load configuration from path: {}", config_path);
    match BidirectionalAgentConfig::from_file(config_path) {
        Ok(loaded_config) => {
            *config = loaded_config; // Overwrite existing config
            config.config_file_path = Some(config_path.to_string());
            info!("Successfully loaded and applied configuration from {}", config_path);
        },
        Err(e) => {
            // If a config file was specified but failed to load, exit with an error.
            error!("Failed to load configuration from '{}': {}", config_path, e);
            error!("Please check the configuration file syntax and ensure all required fields are present.");
            // Exit with an error if the specified config file fails to load.
            return Err(anyhow!("Configuration file loading failed for path: {}", config_path));
        }
    }
    Ok(()) // Return Ok if loading succeeded or no error occurred
}
// Removed extra closing brace here
