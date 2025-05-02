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
        })
    }

    /// Process a message (Example: could be used for direct interaction or testing)
    /// Note: This bypasses the standard A2A server flow.
    pub async fn process_message_directly(&self, message_text: &str) -> Result<String> {
        // Create a representative Task structure (matching types.rs)
        let task_id = Uuid::new_v4().to_string();
        let initial_message = Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                text: message_text.to_string(),
                metadata: None, // Add missing field
                type_: "text".to_string(), // Add missing field
            })],
            metadata: None, // Add missing field
        };
        let task = Task {
            id: task_id.clone(),
            // Use status field
            status: TaskStatus {
                state: TaskState::Submitted, // Start as submitted
                timestamp: Some(Utc::now()), // Timestamp is Option<DateTime>
                message: None, // Add missing field
            },
            // Use history field
            history: Some(vec![initial_message.clone()]),
            artifacts: None, // Field is Option<Vec<Artifact>>
            metadata: None,
            session_id: None,
            // created_at, updated_at don't exist directly on Task
        };

        // Use the BidirectionalTaskRouter's helper to decide execution mode
        // We need an instance of it. Let's assume we create one temporarily or have access.
        // For simplicity, let's recreate the router logic here or assume access.
        // This part is tricky as the router is normally part of the TaskService flow.
        // Let's simulate the routing decision directly for this example method.

        // Simulate routing (replace with actual router call if possible)
        let router = BidirectionalTaskRouter::new(self.llm.clone(), self.agent_directory.clone());
        let decision = router.decide_execution_mode(&task).await?;


        match decision {
            ExecutionMode::Local => {
                // Option 1: Use LLM directly
                info!("Processing message locally using LLM.");
                let prompt = format!("Process this request and provide a helpful response:\n\n{}", message_text);
                self.llm.complete(&prompt).await

                // Option 2: Simulate tool execution (if applicable)
                // info!("Processing message locally using Tool Executor.");
                // let executor = BidirectionalToolExecutor::new();
                // let result_messages = executor.execute(&task, &["default_local_tool".to_string()]).await?;
                // // Extract text from result messages
                // Ok(result_messages.iter().flat_map(|m| m.parts.iter()).filter_map(|p| match p {
                //     Part::TextPart(tp) => Some(tp.text.clone()),
                //     _ => None,
                // }).collect::<Vec<_>>().join("\n"))
            },
            ExecutionMode::Remote { agent_id } => {
                 info!("Delegating message to agent: {}", agent_id);
                // Use the canonical ClientManager to delegate
                let mut client = self.client_manager.get_or_create_client(&agent_id).await?; // Use get_or_create_client

                // Construct TaskSendParams
                let metadata_map: Option<Map<String, Value>> = Some(
                    serde_json::from_value(json!({
                        "origin": {
                            "agent_id": self.agent_id.clone(), // Clone agent_id
                            "timestamp": Utc::now().to_rfc3339(),
                        }
                    }))? // Convert Value to Map
                );

                // Convert the task message to a string for the client API
                let message_text = match &initial_message.parts.first() {
                    Some(Part::TextPart(tp)) => tp.text.clone(),
                    _ => "No text content available".to_string(),
                };
                
                // Convert metadata to JSON string for client API
                let metadata_json_string = if let Some(metadata) = metadata_map {
                    match serde_json::to_string(&metadata) {
                        Ok(json) => Some(json),
                        Err(_) => None,
                    }
                } else {
                    None
                };

                // Send the task using send_task_with_metadata
                let delegated_task = match &metadata_json_string {
                    Some(json_str) => client.send_task_with_metadata(&message_text, Some(json_str)).await?,
                    None => client.send_task_with_metadata(&message_text, None).await?,
                };
                info!("Delegated task ID: {}", delegated_task.id);

                // Wait for the task to complete (simplified polling)
                let mut completed_task = delegated_task.clone(); // Clone initial response
                for i in 0..30 { // Poll for 30 seconds max
                    let current_state = completed_task.status.state;
                    info!("Polling delegated task... Attempt {}, State: {:?}", i+1, current_state);
                    if current_state == TaskState::Completed ||
                       current_state == TaskState::Failed ||
                       current_state == TaskState::Canceled {
                        break;
                    }

                    sleep(std::time::Duration::from_secs(1)).await;
                    // Use get_task which requires &mut self and TaskQueryParams
                    completed_task = client.get_task(&delegated_task.id).await?;
                }

                // Extract the response from the final task state's history
                let response = completed_task.history.as_ref().map(|history| {
                    history.iter()
                        .filter(|m| m.role == Role::Agent) // Use Agent role
                        .flat_map(|m| m.parts.iter())
                        .filter_map(|p| match p {
                            Part::TextPart(tp) => Some(tp.text.clone()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }).unwrap_or_else(|| "No response found in history.".to_string()); // Handle no history

                Ok(response)
            }
        }
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
                        
                        // Create a new client with the provided URL
                        self.client = Some(A2aClient::new(target));
                        println!("🔗 Connected to remote agent: {}", target);
                        
                        // Try to get the agent card to verify connection
                        match self.get_remote_agent_card().await {
                            Ok(card) => {
                                println!("✅ Successfully connected to agent: {}", card.name);
                                
                                // Add to known servers if not already present
                                if !known_servers.iter().any(|(_, url)| url == target) {
                                    known_servers.push((card.name.clone(), target.to_string()));
                                }
                            },
                            Err(e) => {
                                println!("⚠️ Connected, but couldn't retrieve agent card: {}", e);
                                
                                // Still add to known servers
                                if !known_servers.iter().any(|(_, url)| url == target) {
                                    known_servers.push(("Unknown Agent".to_string(), target.to_string()));
                                }
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
                                        // Wait for the server to complete or be cancelled
                                        match handle.await {
                                            Ok(()) => info!("Server shut down gracefully."),
                                            Err(e) => error!("Server error: {}", e),
                                        }
                                    },
                                    Err(e) => {
                                        error!("Failed to start server: {}", e);
                                    }
                                }
                            });
                            
                            server_running = true;
                            println!("✅ Server started on http://{}:{}", self.bind_address, port);
                            println!("The server will run until you exit the REPL or send :stop");
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
    
    // Process command line argument if provided
    if args.len() > 1 {
        let arg = &args[1];
        
        // Check if the argument is in the format "server:port"
        if arg.contains(':') {
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
            // If the argument doesn't contain ':', treat it as a config file path
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
                    warn!("Failed to load configuration from {}: {}. Using default configuration.", config_path, e);
                    // Continue with default configuration
                }
            }
        }
    } else {
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
