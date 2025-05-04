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
    // errors::ClientError, // Unused
    streaming::{/* StreamingResponse, StreamingResponseStream */}, // Unused
};

use crate::server::{
    repositories::task_repository::{TaskRepository, InMemoryTaskRepository},
    services::{
        task_service::TaskService,
        streaming_service::StreamingService,
        notification_service::NotificationService,
    },
    // Use the canonical TaskRouter and ToolExecutor from the server module
    task_router::{LlmTaskRouterTrait, /* RoutingDecision */}, // Import LlmTaskRouterTrait // Removed unused RoutingDecision
    tool_executor::ToolExecutor, // Import ToolExecutor
    // Use the canonical AgentRegistry and ClientManager
    agent_registry::AgentRegistry,
    client_manager::ClientManager,
    run_server,
    // error::ServerError, // Unused
};

use crate::types::{
    Task, TaskState, Message, Part, TextPart, Role, AgentCard, AgentCapabilities,
    TaskSendParams, TaskQueryParams, /* TaskIdParams, PushNotificationConfig, */ // Unused
    /* DataPart, FilePart, TaskStatus, */ // Unused
};


use anyhow::{anyhow, /* Context, */ Result}; // Removed unused Context
// use async_trait::async_trait; // Unused
use chrono::{/* DateTime, */ Utc}; // Removed unused DateTime
use dashmap::DashMap;
use futures_util::{StreamExt, /* TryStreamExt */}; // Removed unused TryStreamExt
// use reqwest; // Unused
use serde::{Deserialize, Serialize};
use serde_json::{/* json, Value, Map */}; // Unused imports
use std::{
    sync::Arc,
    // error::Error as StdError, // Unused
    // fs, // Unused
    path::Path,
    io::{self, /* Write, BufRead */}, // Removed unused Write, BufRead
    path::PathBuf, // Import PathBuf
    // collections::HashMap, // Unused
}; // Import StdError for error mapping and IO
use tokio::{
    // fs::OpenOptions, // Unused
    // io::AsyncWriteExt, // Unused
    // time::sleep, // Unused
};
use tokio_util::sync::CancellationToken;
// use toml; // Unused
use tracing::{debug, error, info, trace, warn, instrument, /* Level, */ Instrument}; // Removed unused Level
use tracing_subscriber::{fmt::{self, format::FmtSpan}, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer, /* filter */}; // Removed unused filter
use uuid::Uuid;

// Use items from the new config module (relative to parent mod.rs)
use super::agent_helpers;
use super::config::*;
use super::llm_client::{LlmClient, ClaudeLlmClient};
use super::task_router::ExecutionMode;
// Import the router struct
use crate::bidirectional::BidirectionalTaskRouter;

// REMOVED AgentLogContext struct and impl block

// Constants
pub const AGENT_NAME: &str = "Bidirectional A2A Agent";
pub const AGENT_VERSION: &str = "1.0.0";
// Defaults moved to config.rs

// ExecutionMode enum moved to task_router.rs

// REMOVED AgentDirectoryEntry struct definition
// REMOVED AgentDirectory struct definition and impl block


// REMOVED Local AgentRegistry definition
// REMOVED Local ClientManager definition

// REMOVED LlmClient trait definition
// REMOVED ClaudeLlmClient struct definition
// REMOVED ClaudeLlmClient impl LlmClient block

// REMOVED BidirectionalTaskRouter struct definition

// REMOVED BidirectionalTaskRouter impl block
 
// REMOVED impl LlmTaskRouterTrait for BidirectionalTaskRouter block

// REMOVED Local ToolExecutor definition

// Define a struct that implements the server's ToolExecutorTrait
// We'll use the provided ToolExecutor from the server module
// instead of implementing our own ToolExecutorTrait


/// Main bidirectional agent implementation
pub struct BidirectionalAgent {
    // Core components (TaskService, StreamingService, NotificationService are needed for run_server)
    pub task_service: Arc<TaskService>,
    pub streaming_service: Arc<StreamingService>,
    pub notification_service: Arc<NotificationService>,

    // Use canonical server components
    pub agent_registry: Arc<AgentRegistry>, // From crate::server::agent_registry
    pub client_manager: Arc<ClientManager>, // From crate::server::client_manager

    // Local components for specific logic
    // REMOVED agent_directory field
    // REMOVED agent_directory_path field
    pub llm: Arc<dyn LlmClient>, // Local LLM client

    // Server configuration
    pub port: u16,
    pub bind_address: String,
    pub agent_id: String, // Keep agent_id for internal identification
    pub agent_name: String, // Name used for the agent card
    pub client_config: ClientConfig, // Store client config for URL access

    // A2A client for making outbound requests to other agents
    // This is a simple client that can be expanded later
    pub client: Option<A2aClient>,

    // Session management
    pub current_session_id: Option<String>,
    pub session_tasks: Arc<DashMap<String, Vec<String>>>, // Map session ID to task IDs

    // REPL logging
    pub repl_log_file: Option<std::path::PathBuf>,

    // Known servers discovered/connected to (URL -> Name) - Shared for background updates
    pub known_servers: Arc<DashMap<String, String>>,
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

        // Create the task router, passing the enabled tools list, registry, and task repository
        let bidirectional_task_router: Arc<dyn LlmTaskRouterTrait> =
            Arc::new(BidirectionalTaskRouter::new(
                llm.clone(),
                agent_registry.clone(),
                enabled_tools.clone(),
                Some(task_repository.clone()),
                &config, // Pass the full config reference
            ));
        trace!("BidirectionalTaskRouter created with config flags.");

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

    // ensure_session moved to agent_helpers.rs


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
            // Call the central command handler from the repl module
            // We need to pass the *original* arguments string, not the potentially empty one
            let full_args = if command == trimmed_text.to_lowercase() { "" } else { trimmed_text.split_once(' ').map_or("", |(_, a)| a) };
            // Call the function directly, passing the mutable agent reference
            return crate::bidirectional::repl::commands::handle_repl_command(self, &command, full_args).await;
        }
        // --- End Command Interception ---


        // --- Original Logic (if not intercepted as a command) ---
        debug!("Input does not match command keyword. Proceeding with standard task processing.");

        // Ensure we have an active session before processing
        debug!("Ensuring session exists before processing message.");
        agent_helpers::ensure_session(self).await; // Call helper function

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
        agent_helpers::save_task_to_history(self, task.clone()).await?; // Call helper

        // Extract response from task
        debug!(task_id = %task.id, "Extracting text response from task.");
        let mut response = agent_helpers::extract_text_from_task(self, &task); // Call helper
        trace!(response = %response, "Extracted response text.");

        // If the task is in InputRequired state, indicate that in the response
        if task.status.state == TaskState::InputRequired {
            debug!(task_id = %task.id, "Task requires more input. Appending message to response."); // Changed to debug
            response.push_str("\n\n[The agent needs more information. Your next message will continue this task.]");
        }

        debug!(task_id = %task.id, "Local message processing complete. Returning response."); // Changed to debug
        Ok(response) // Return the final response string
    }

    // REMOVED extract_text_from_task function
    // REMOVED create_new_session function
    // REMOVED save_task_to_history function
    // REMOVED get_current_session_tasks function
    // REMOVED client_url function

    // REMOVED log_agent_action function
    // REMOVED clone_for_logging function

    /// Run the agent server
    #[instrument(skip(self), fields(agent_id = %self.agent_id, port = %self.port, bind_address = %self.bind_address))]
    pub async fn run(&self) -> Result<()> {
        // Use the run_server function from agent_helpers.rs
        agent_helpers::run_server(self).await
    }

    /// Send a task to a remote agent using the A2A client, ensuring session consistency.
    #[instrument(skip(self, message), fields(agent_id = %self.agent_id, message_len = message.len()))]
    // Make pub so REPL module can access it
    pub async fn send_task_to_remote(&mut self, message: &str) -> Result<Task> {
        agent_helpers::send_task_to_remote(self, message).await
    }

    /// Get capabilities of a remote agent (using A2A client)
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    // Make pub so REPL module can access it
    pub async fn get_remote_agent_card(&mut self) -> Result<AgentCard> {
        agent_helpers::get_remote_agent_card(self).await
    }

    /// Create an agent card for this agent instance.
    // No instrument macro needed here as it's synchronous and called from instrumented contexts.
    pub fn create_agent_card(&self) -> AgentCard {
        agent_helpers::create_agent_card(self)
    }
}


// REMOVED Config Structs (ServerConfig, ClientConfig, LlmConfig, ToolsConfig, ModeConfig, BidirectionalAgentConfig)
// REMOVED Default implementations for Config Structs
// REMOVED Default helper functions (default_port, default_bind_address, default_agent_id, default_system_prompt)
// REMOVED BidirectionalAgentConfig::from_file implementation

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
        // Call the run_repl function from the new module
        crate::bidirectional::repl::run_repl(&mut agent).await
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

// REMOVED load_config_from_path helper function (moved to config.rs)
