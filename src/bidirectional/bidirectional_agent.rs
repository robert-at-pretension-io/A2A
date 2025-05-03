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
    io::{self, Write, BufRead}
}; // Import StdError for error mapping and IO
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
use toml;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

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
}

impl BidirectionalTaskRouter {
     pub fn new(llm: Arc<dyn LlmClient>, directory: Arc<AgentDirectory>) -> Self {
        Self {
            llm,
            directory,
        }
    }

    // Helper to perform the actual routing logic
    pub async fn decide_execution_mode(&self, task: &Task) -> Result<ExecutionMode> {
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
            // For now, default to Local if history is empty/None.
            return Ok(ExecutionMode::Local);
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
        let decision = self.llm.complete(&routing_prompt).await?;
        let decision = decision.trim();

        // Parse the decision
        if decision == "LOCAL" {
            Ok(ExecutionMode::Local)
        } else if decision.starts_with("REMOTE: ") {
            let agent_id = decision.strip_prefix("REMOTE: ").unwrap().trim().to_string();
            
            // Verify the agent exists in the local directory
            if self.directory.get_agent(&agent_id).is_none() {
                 warn!("LLM decided to delegate to unknown agent '{}', falling back to local execution.", agent_id);
                return Ok(ExecutionMode::Local); // Fall back to local if agent not found
            }
            
            Ok(ExecutionMode::Remote { agent_id })
        } else {
            warn!("LLM routing decision was unclear ('{}'), falling back to local execution.", decision);
            // Default to local if the decision isn't clear
            Ok(ExecutionMode::Local)
        }
    }
}

#[async_trait]
impl LlmTaskRouterTrait for BidirectionalTaskRouter {
    // Match the trait signature: takes TaskSendParams, returns Result<RoutingDecision, ServerError>
    async fn route_task(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError> {
        // We need a Task object to make the decision based on history/message.
        // Construct a temporary Task from TaskSendParams.
        // This might be simplified if the trait signature changes or if TaskService provides the Task.
        let task = Task {
            id: params.id.clone(),
            status: TaskStatus { // Default status for routing decision
                state: TaskState::Submitted,
                timestamp: Some(Utc::now()),
                message: None, // Add missing field
            },
            history: Some(vec![params.message.clone()]), // Use the incoming message as history start
            artifacts: None,
            metadata: params.metadata.clone(),
            session_id: params.session_id.clone(),
        };

        self.decide_execution_mode(&task).await
            .map(|decision| match decision {
                ExecutionMode::Local => {
                    // Convert to the server's RoutingDecision
                    // Provide tool names if applicable, otherwise empty or default
                    RoutingDecision::Local {
                        tool_names: vec!["default_local_tool".to_string()] // Example tool name
                    }
                },
                ExecutionMode::Remote { agent_id } => {
                    // Convert to the server's RoutingDecision
                    RoutingDecision::Remote {
                        agent_id: agent_id.clone()
                    }
                },
            })
            // Map the anyhow::Error to ServerError::Internal
            .map_err(|e| ServerError::Internal(format!("Routing error: {}", e)))
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
        // Simply delegate to route_task for now
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
    agent_id: String, // Keep agent_id for identification
    
    // A2A client for making outbound requests to other agents
    // This is a simple client that can be expanded later
    pub client: Option<A2aClient>,
    
    // Session management
    current_session_id: Option<String>,
    session_tasks: Arc<DashMap<String, Vec<String>>>, // Map session ID to task IDs
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
        let bidirectional_task_router: Arc<dyn LlmTaskRouterTrait> = Arc::new(BidirectionalTaskRouter::new(llm.clone(), agent_directory.clone()));

        // Create a tool executor using the provided implementation
        let bidirectional_tool_executor = Arc::new(ToolExecutor::new());

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

        Ok(Self {
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
            bind_address: config.server.bind_address,
            agent_id: config.server.agent_id,
            
            // Store the A2A client (if configured)
            client,
            
            // Initialize session management
            current_session_id: None,
            session_tasks: Arc::new(DashMap::new()),
        })
    }

    /// Process a message (Example: could be used for direct interaction or testing)
    pub async fn process_message_directly(&self, message_text: &str) -> Result<String> {
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
                    
                    if let Ok(task) = self.task_service.get_task(params).await {
                        if task.status.state == TaskState::InputRequired {
                            // We should continue this task instead of creating a new one
                            continue_task_id = Some(last_task_id.clone());
                        }
                    }
                }
            }
        }
    
        // Create a unique task ID if not continuing an existing task
        let task_id = if let Some(id) = continue_task_id {
            id
        } else {
            Uuid::new_v4().to_string()
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
        
        // Use task_service to process the task
        let task = self.task_service.process_task(params).await
            .map_err(|e| anyhow!("Failed to process task: {}", e))?;
        
        // Save task to history
        self.save_task_to_history(task.clone()).await?;
        
        // Extract response from task
        let mut response = self.extract_text_from_task(&task);
        
        // If the task is in InputRequired state, indicate that in the response
        if task.status.state == TaskState::InputRequired {
            response.push_str("\n\n[The agent needs more information. Your next message will continue this task.]");
        }
        
        Ok(response)
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
        println!("\n========================================");
        println!("⚡ Bidirectional A2A Agent REPL Mode ⚡");
        println!("========================================");
        println!("Type a message to process it directly with the agent.");
        println!("Special commands:");
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
        
        // Keep track of known servers
        let mut known_servers: Vec<(String, String)> = Vec::new(); // (name, url)
        
        // Display initial client URL if we have one
        if let Some(client) = &self.client {
            if let Some(url) = &self.client_url() {
                println!("🔗 Connected to remote agent: {}", url);
                // Try to get agent name
                match self.get_remote_agent_card().await {
                    Ok(card) => {
                        println!("📇 Connected to: {} ({})", card.name, url);
                        // Add to known servers if not already present
                        if !known_servers.iter().any(|(_, server_url)| server_url == url) {
                            known_servers.push((card.name.clone(), url.clone()));
                        }
                    },
                    Err(_) => {
                        println!("🔗 Connected to: {}", url);
                    }
                }
            }
        }
        
        // Flag to track if we have a listening server running
        let mut server_running = false;
        let mut server_shutdown_token: Option<CancellationToken> = None;
        
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
            reader.read_line(&mut input)?;
            
            let input = input.trim();
            
            if input.is_empty() {
                continue;
            }
            
            if input.starts_with(":") {
                // Handle special commands
                if input == ":help" {
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
                } else if input == ":quit" {
                    println!("Exiting REPL. Goodbye!");
                    
                    // Shutdown server if running
                    if let Some(token) = server_shutdown_token.take() {
                        println!("Shutting down server...");
                        token.cancel();
                    }
                    
                    break;
                } else if input == ":card" {
                    let card = self.create_agent_card();
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
                } else if input == ":servers" {
                    // List known servers
                    if known_servers.is_empty() {
                        println!("No known servers. Connect to a server first with :connect URL");
                    } else {
                        println!("\n📋 Known Servers:");
                        for (i, (name, url)) in known_servers.iter().enumerate() {
                            // Mark the currently connected server with an asterisk
                            let marker = if Some(url) == self.client_url().as_ref() { "*" } else { " " };
                            println!("  {}{}: {} - {}", marker, i+1, name, url);
                        }
                        println!("\nUse :connect N to connect to a server by number");
                        println!("");
                    }
                } else if input == ":disconnect" {
                    // Disconnect from current server
                    if self.client.is_some() {
                        let url = self.client_url().unwrap_or_else(|| "unknown".to_string());
                        self.client = None;
                        println!("🔌 Disconnected from {}", url);
                    } else {
                        println!("Not connected to any server");
                    }
                } else if input.starts_with(":connect ") {
                    let target = input.trim_start_matches(":connect ").trim();
                    
                    // Check if it's a number (referring to a server in the list)
                    if let Ok(server_idx) = target.parse::<usize>() {
                        if server_idx > 0 && server_idx <= known_servers.len() {
                            let (name, url) = &known_servers[server_idx - 1];
                            
                            // Create a new client with the provided URL
                            self.client = Some(A2aClient::new(url));
                            println!("🔗 Connected to {}: {}", name, url);
                        } else {
                            println!("❌ Error: Invalid server number. Use :servers to see available servers.");
                        }
                    } else {
                        // Treat as URL
                        if target.is_empty() {
                            println!("❌ Error: No URL provided. Use :connect URL");
                            continue;
                        }
                        
                        // Create a new client with the provided URL, but don't assume connection will succeed
                        let client = A2aClient::new(target);
                        
                        // Try to get the agent card to verify connection
                        let connect_result = match client.get_agent_card().await {
                            Ok(card) => {
                                println!("✅ Successfully connected to agent: {}", card.name);
                                
                                // Add to known servers if not already present
                                if !known_servers.iter().any(|(_, url)| url == target) {
                                    known_servers.push((card.name.clone(), target.to_string()));
                                }
                                
                                // Store the client since connection was successful
                                self.client = Some(client);
                                
                                // IMPORTANT: Add the agent to the agent directory for routing
                                // Use the agent name as the ID
                                self.agent_directory.add_or_update_agent(card.name.clone(), card.clone());
                                
                                // This sets up the bidirectional agent so it can route tasks to this agent
                                println!("🔄 Added agent to directory for task routing");
                                
                                true
                            },
                            Err(e) => {
                                println!("❌ Failed to connect to agent at {}: {}", target, e);
                                println!("Please check that the server is running and the URL is correct.");
                                false
                            }
                        };
                        
                        // Only add to known servers if we got a connection, and it's not already in the list
                        if !connect_result && !known_servers.iter().any(|(_, url)| url == target) {
                            // Ask the user if they want to add the server anyway
                            println!("Do you want to add this server to the known servers list anyway? (y/n)");
                            let mut answer = String::new();
                            io::stdin().read_line(&mut answer).unwrap_or_default();
                            
                            if answer.trim().to_lowercase() == "y" {
                                known_servers.push(("Unknown Agent".to_string(), target.to_string()));
                                println!("Added server to known servers list.");
                            }
                        }
                    }
                } else if input.starts_with(":listen ") {
                    let port_str = input.trim_start_matches(":listen ").trim();
                    
                    // Check if already running
                    if server_running {
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
                        },
                        Err(_) => {
                            println!("❌ Error: Invalid port number. Please provide a valid port.");
                        }
                    }
                } else if input == ":stop" {
                    // Stop the server if running
                    if let Some(token) = server_shutdown_token.take() {
                        println!("Shutting down server...");
                        token.cancel();
                        server_running = false;
                        println!("✅ Server stopped");
                    } else {
                        println!("⚠️ No server currently running");
                    }
                } else if input.starts_with(":remote ") {
                    let message = input.trim_start_matches(":remote ").trim();
                    if message.is_empty() {
                        println!("❌ Error: No message provided. Use :remote MESSAGE");
                        continue;
                    }
                    
                    // Check if we're connected to a remote agent
                    if self.client.is_none() {
                        println!("❌ Error: Not connected to a remote agent. Use :connect URL first.");
                        continue;
                    }
                    
                    // Send task to remote agent
                    println!("📤 Sending task to remote agent: '{}'", message);
                    match self.send_task_to_remote(message).await {
                        Ok(task) => {
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
                            println!("❌ Error sending task: {}", e);
                        }
                    }
                } else if input == ":session new" {
                    let session_id = self.create_new_session();
                    println!("✅ Created new session: {}", session_id);
                } else if input == ":session show" {
                    if let Some(session_id) = &self.current_session_id {
                        println!("🔍 Current session: {}", session_id);
                    } else {
                        println!("⚠️ No active session. Use :session new to create one.");
                    }
                } else if input == ":history" {
                    if let Some(session_id) = &self.current_session_id {
                        let tasks = self.get_current_session_tasks().await?;
                        if tasks.is_empty() {
                            println!("📭 No messages in current session.");
                        } else {
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
                        println!("⚠️ No active session. Use :session new to create one.");
                    }
                } else if input == ":tasks" {
                    if let Some(session_id) = &self.current_session_id {
                        let tasks = self.get_current_session_tasks().await?;
                        if tasks.is_empty() {
                            println!("📭 No tasks in current session.");
                        } else {
                            println!("\n📋 Tasks in Current Session:");
                            for (i, task) in tasks.iter().enumerate() {
                                println!("  {}. {} - Status: {:?}", i + 1, task.id, task.status.state);
                            }
                        }
                    } else {
                        println!("⚠️ No active session. Use :session new to create one.");
                    }
                } else if input.starts_with(":task ") {
                    let task_id = input.trim_start_matches(":task ").trim();
                    if task_id.is_empty() {
                        println!("❌ Error: No task ID provided. Use :task TASK_ID");
                    } else {
                        // Get task details
                        let params = TaskQueryParams {
                            id: task_id.to_string(),
                            history_length: None,
                            metadata: None,
                        };
                        
                        match self.task_service.get_task(params).await {
                            Ok(task) => {
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
                                println!("❌ Error: Failed to get task: {}", e);
                            }
                        }
                    }
                } else if input.starts_with(":artifacts ") {
                    let task_id = input.trim_start_matches(":artifacts ").trim();
                    if task_id.is_empty() {
                        println!("❌ Error: No task ID provided. Use :artifacts TASK_ID");
                    } else {
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
                                        println!("📦 No artifacts for task {}", task.id);
                                    } else {
                                        println!("\n📦 Artifacts for Task {}:", task.id);
                                        for (i, artifact) in artifacts.iter().enumerate() {
                                            println!("  {}. {} ({})", i + 1, 
                                                     artifact.name.clone().unwrap_or_else(|| format!("Artifact {}", artifact.index)),
                                                     artifact.description.clone().unwrap_or_else(|| "No description".to_string()));
                                        }
                                    }
                                } else {
                                    println!("📦 No artifacts for task {}", task.id);
                                }
                            },
                            Err(e) => {
                                println!("❌ Error: Failed to get task: {}", e);
                            }
                        }
                    }
                } else if input.starts_with(":cancelTask ") {
                    let task_id = input.trim_start_matches(":cancelTask ").trim();
                    if task_id.is_empty() {
                        println!("❌ Error: No task ID provided. Use :cancelTask TASK_ID");
                    } else {
                        // Cancel the task
                        let params = TaskIdParams {
                            id: task_id.to_string(),
                            metadata: None,
                        };
                        
                        match self.task_service.cancel_task(params).await {
                            Ok(task) => {
                                println!("✅ Successfully canceled task {}", task.id);
                                println!("  Current state: {:?}", task.status.state);
                            },
                            Err(e) => {
                                println!("❌ Error: Failed to cancel task: {}", e);
                            }
                        }
                    }
                } else {
                    println!("❌ Unknown command: {}", input);
                    println!("Type :help for a list of commands");
                }
            } else {
                // Process the message directly with the agent
                match self.process_message_directly(input).await {
                    Ok(response) => {
                        println!("\n🤖 Agent response:\n{}\n", response);
                    },
                    Err(e) => {
                        println!("❌ Error processing message: {}", e);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Create a new session
    pub fn create_new_session(&mut self) -> String {
        let session_id = format!("session-{}", Uuid::new_v4());
        self.current_session_id = Some(session_id.clone());
        self.session_tasks.insert(session_id.clone(), Vec::new());
        session_id
    }
    
    /// Add task to current session
    async fn save_task_to_history(&self, task: Task) -> Result<()> {
        if let Some(session_id) = &self.current_session_id {
            if let Some(mut tasks) = self.session_tasks.get_mut(session_id) {
                tasks.push(task.id.clone());
            }
        }
        Ok(())
    }
    
    /// Get tasks for current session
    pub async fn get_current_session_tasks(&self) -> Result<Vec<Task>> {
        let mut tasks = Vec::new();
        if let Some(session_id) = &self.current_session_id {
            if let Some(task_ids) = self.session_tasks.get(session_id) {
                for task_id in task_ids.iter() {
                    // Use TaskQueryParams to get task with history
                    let params = TaskQueryParams {
                        id: task_id.clone(),
                        history_length: None, // Get full history
                        metadata: None,
                    };
                    
                    if let Ok(task) = self.task_service.get_task(params).await {
                        tasks.push(task);
                    }
                }
            }
        }
        Ok(tasks)
    }
    
    /// Get the URL of the currently connected client
    fn client_url(&self) -> Option<String> {
        // Since we don't have a getter for the URL in the client,
        // we'll use a simple approach to track the URL for now
        // In a real implementation, you'd add a getter to A2aClient
        // or track the URL separately
        self.client.as_ref().map(|c| extract_base_url_from_client(c))
    }
    
    /// Run the agent server
    pub async fn run(&self) -> Result<()> {
        // Create a cancellation token for graceful shutdown
        let shutdown_token = CancellationToken::new();
        let shutdown_token_clone = shutdown_token.clone();

        // Set up signal handlers for graceful shutdown
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.expect("Failed to install CTRL+C handler");
            println!("Received shutdown signal, stopping server...");
            shutdown_token_clone.cancel();
        });

        // Create the agent card for this server
        let agent_card = self.create_agent_card();
        
        // Convert to JSON value for the server
        let agent_card_json = serde_json::to_value(agent_card).unwrap_or_else(|e| {
            warn!("Failed to serialize agent card: {}", e);
            serde_json::json!({})
        });
        
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
            Ok(handle) => handle,
            Err(e) => {
                return Err(anyhow!("Failed to start server: {}", e));
            }
        };

        println!("Server running on {}:{}", self.bind_address, self.port);

        // Wait for the server to complete
        // The JoinHandle contains Result<(), JoinError>
        match server_handle.await {
            Ok(()) => {
                info!("Server shut down gracefully.");
                Ok(())
            }
            Err(join_err) => {
                error!("Failed to join server task: {}", join_err);
                Err(anyhow!("Server join error: {}", join_err))
            }
        }
    }

    /// Send a task to a remote agent using the A2A client
    pub async fn send_task_to_remote(&mut self, message: &str) -> Result<Task> {
        // Check if we have a client configured
        if let Some(client) = &mut self.client {
            info!("Sending task to remote agent: {}", message);
            // Use the simple send_task method for now
            // This can be expanded to use more advanced client features later
            let task = client.send_task(message).await
                .map_err(|e| anyhow!("Error sending task to remote agent: {}", e))?;
            
            info!("Remote task created with ID: {}", task.id);
            return Ok(task);
        } else {
            return Err(anyhow!("No A2A client configured. Use --target-url to specify a remote agent."));
        }
    }
    
    /// Get capabilities of a remote agent (using A2A client)
    pub async fn get_remote_agent_card(&mut self) -> Result<AgentCard> {
        // Check if we have a client configured
        if let Some(client) = &mut self.client {
            info!("Retrieving agent card from remote agent");
            let agent_card = client.get_agent_card().await
                .map_err(|e| anyhow!("Error retrieving agent card: {}", e))?;
            
            info!("Retrieved agent card for: {}", agent_card.name);
            return Ok(agent_card);
        } else {
            return Err(anyhow!("No A2A client configured. Use --target-url to specify a remote agent."));
        }
    }

    /// Create an agent card (matching types.rs structure)
    pub fn create_agent_card(&self) -> AgentCard {
        // Construct AgentCapabilities based on actual capabilities
        let capabilities = AgentCapabilities {
            push_notifications: true, // Example: Assuming supported
            state_transition_history: true, // Example: Assuming supported
            streaming: false, // Example: Assuming not supported
            // Add other fields from AgentCapabilities if they exist
        };

        AgentCard {
            // id field does not exist on AgentCard in types.rs
            name: AGENT_NAME.to_string(), // name is String
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

/// Configuration for the bidirectional agent
#[derive(Clone, Debug, Deserialize)]
pub struct BidirectionalAgentConfig {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub client: ClientConfig,
    #[serde(default)]
    pub llm: LlmConfig,
    
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
    pub get_agent_card: bool,
    pub remote_task: Option<String>,
    
    // Auto-listen on server port at startup
    #[serde(default)]
    pub auto_listen: bool,
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

/// Helper function to extract the base URL from an A2aClient
fn extract_base_url_from_client(client: &A2aClient) -> String {
    // In a real implementation, you'd add a getter to the A2aClient
    // For now, we'll use this workaround to access private fields
    
    // If we have a client config with target_url, use that
    if let Some(url) = client_url_hack(client) {
        return url;
    }
    
    // Fallback
    format!("remote-agent")
}

// Temporary hack to get the URL from a client until we can add a getter
fn client_url_hack(client: &A2aClient) -> Option<String> {
    // Try to get the internal object as a debug string and extract the URL
    let debug_str = format!("{:?}", client);
    
    // Extract URL from debug output - likely to contain "base_url: ..."
    if let Some(start_idx) = debug_str.find("base_url: ") {
        let start_idx = start_idx + "base_url: ".len();
        if let Some(end_idx) = debug_str[start_idx..].find('\"') {
            let end_idx = start_idx + end_idx;
            if let Some(url_str) = debug_str.get(start_idx..end_idx) {
                return Some(url_str.to_string());
            }
        }
    }
    
    None
}

/// Main entry point
#[tokio::main]
pub async fn main() -> Result<()> {
    // Initialize tracing subscriber
    tracing_subscriber::fmt::init();

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
    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        
        if arg == "--listen" || arg == "-l" {
            // Enable auto-listen mode
            config.mode.auto_listen = true;
            std::env::set_var("AUTO_LISTEN", "true");
            
            // If a port is provided after the flag, use it
            if i + 1 < args.len() {
                if let Ok(port) = args[i + 1].parse::<u16>() {
                    config.server.port = port;
                    info!("Will auto-listen on port {}", port);
                    i += 1; // Skip the port argument in the next iteration
                }
            }
        } else if arg.starts_with("--port=") {
            // Parse port from --port=NNNN format
            if let Ok(port) = arg.trim_start_matches("--port=").parse::<u16>() {
                config.server.port = port;
                if config.mode.auto_listen {
                    info!("Will auto-listen on port {}", port);
                }
            } else {
                warn!("Invalid port format in '{}'. Using default port.", arg);
            }
        } else if arg.contains(':') {
            // Check if the argument is in the format "server:port"
            let parts: Vec<&str> = arg.split(':').collect();
            if parts.len() == 2 {
                let server = parts[0];
                if let Ok(port) = parts[1].parse::<u16>() {
                    // Set server and port in the configuration
                    info!("Setting up connection to server '{}' on port {}", server, port);
                    
                    // Update client configuration with the server URL
                    let server_url = format!("http://{}:{}", server, port);
                    config.client.target_url = Some(server_url);
                } else {
                    warn!("Invalid port number in '{}'. Using default configuration.", arg);
                }
            } else {
                warn!("Invalid argument format '{}'. Expected 'server:port'. Using default configuration.", arg);
            }
        } else {
            // If the argument doesn't match any flag, treat it as a config file path
            let config_path = arg;
            info!("Loading configuration from {}", config_path);
            
            // Try to load configuration from file
            match BidirectionalAgentConfig::from_file(config_path) {
                Ok(loaded_config) => {
                    config = loaded_config;
                    config.config_file_path = Some(config_path.to_string());
                    info!("Successfully loaded configuration from {}", config_path);
                },
                Err(e) => {
                    // If a config file was specified but failed to load, exit with an error.
                    // Don't silently fall back to defaults in this case.
                    error!("Failed to load configuration from '{}': {}", config_path, e);
                    error!("Please check the configuration file syntax and ensure all required fields are present.");
                    return Err(anyhow!("Configuration file loading failed"));
                }
            }
        }
        
        i += 1;
    }
    
    if args.len() <= 1 {
        info!("No arguments provided. Using default configuration.");
    }
    
    // Process actions based on config.mode settings
    
    // REPL mode takes precedence
    if config.mode.repl {
        // Create the agent
        let mut agent = BidirectionalAgent::new(config.clone())?;
        
        // Run the REPL
        return agent.run_repl().await;
    }
    
    // Process a single message
    if let Some(message) = &config.mode.message {
        // Create the agent
        let agent = BidirectionalAgent::new(config.clone())?;

        // Process the message
        println!("Processing message: '{}'", message);
        let response = agent.process_message_directly(message).await?;

        println!("Response:\n{}", response);
        return Ok(());
    }
    
    // Send task to remote agent
    if let Some(task_message) = &config.mode.remote_task {
        // Check if we have a target URL
        if config.client.target_url.is_none() {
            return Err(anyhow!("No target URL configured. Add target_url to the [client] section in config file."));
        }
        
        // Create the agent
        let mut agent = BidirectionalAgent::new(config.clone())?;
        
        // Send task to remote agent
        println!("Sending task to remote agent: '{}'", task_message);
        let task = agent.send_task_to_remote(task_message).await?;
        
        println!("Task sent successfully!");
        println!("Task ID: {}", task.id);
        println!("Initial state: {:?}", task.status.state);
        
        return Ok(());
    }
    
    // Get remote agent card
    if config.mode.get_agent_card {
        // Check if we have a target URL
        if config.client.target_url.is_none() {
            return Err(anyhow!("No target URL configured. Add target_url to the [client] section in config file."));
        }
        
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
        
        return Ok(());
    }
    
    // Default mode: Run the server
    info!("Starting agent with config: {:?}", config);
    let agent = BidirectionalAgent::new(config)?;
    agent.run().await
}
