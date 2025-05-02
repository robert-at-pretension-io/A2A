//! Simplified Bidirectional A2A Agent Implementation
//!
//! This file implements a minimal but complete bidirectional A2A agent
//! that can both serve requests and delegate them to other agents.

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
    task_router::{TaskRouterTrait, RoutingDecision}, // Import TaskRouterTrait and RoutingDecision
    tool_executor::{ToolExecutorTrait}, // Import ToolExecutorTrait
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
use std::{sync::Arc, error::Error as StdError}; // Import StdError for error mapping
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;
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
struct AgentDirectory {
    agents: Arc<DashMap<String, AgentDirectoryEntry>>,
}

impl AgentDirectory {
    fn new() -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
        }
    }

    // Note: This uses the AgentCard from crate::types
    // We need the agent_id separately as AgentCard doesn't have it
    fn add_or_update_agent(&self, agent_id: String, card: AgentCard) {
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
trait LlmClient: Send + Sync {
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
            "model": "claude-3-opus-20240229",
            "max_tokens": 4000,
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
struct BidirectionalTaskRouter {
    llm: Arc<dyn LlmClient>,
    directory: Arc<AgentDirectory>, // Use the local directory for routing logic
}

impl BidirectionalTaskRouter {
     fn new(llm: Arc<dyn LlmClient>, directory: Arc<AgentDirectory>) -> Self {
        Self {
            llm,
            directory,
        }
    }

    // Helper to perform the actual routing logic
    async fn decide_execution_mode(&self, task: &Task) -> Result<ExecutionMode> {
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
                    a.card.name.as_deref().unwrap_or(""), // name is Option<String>
                    a.card.description.as_deref().unwrap_or(""),
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
impl TaskRouterTrait for BidirectionalTaskRouter {
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
}


// REMOVED Local ToolExecutor definition

// Define a struct that implements the server's ToolExecutorTrait
#[derive(Clone)]
struct BidirectionalToolExecutor;

impl BidirectionalToolExecutor {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ToolExecutorTrait for BidirectionalToolExecutor {
     async fn execute(&self, _task: &Task, _tool_names: &[String]) -> Result<Vec<Message>, ServerError> {
        // For simplicity, just return a placeholder response
        // In a real implementation, this would execute actual tools based on tool_names
        Ok(vec![Message {
            role: Role::Agent, // Use Agent role
            parts: vec![Part::TextPart(TextPart {
                text: "Tool execution completed (placeholder)".to_string(),
                metadata: None, // Add missing field
                type_: "text".to_string(), // Add missing field
            })],
            metadata: None, // Add missing field
        }])
        // Note: If the internal logic could fail, map the error to ServerError
        // .map_err(|e| ServerError::Internal(format!("Tool execution error: {}", e)))
    }
}


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
}

impl BidirectionalAgent {
    pub fn new(config: BidirectionalAgentConfig) -> Result<Self> {
        // Create the agent directory (local helper)
        let agent_directory = Arc::new(AgentDirectory::new());

        // Create the LLM client (local helper)
        let llm: Arc<dyn LlmClient> = if let Some(api_key) = &config.claude_api_key {
            Arc::new(ClaudeLlmClient::new(api_key.clone(), SYSTEM_PROMPT.to_string()))
        } else {
            // Allow running without LLM if only acting as server/client without local processing
            warn!("No Claude API key provided. Local LLM processing will not be available.");
            // Provide a dummy LLM client or handle this case appropriately
             return Err(anyhow!("No LLM configuration provided")); // Or handle differently
        };

        // Create the task repository (needed by services)
        let task_repository: Arc<dyn TaskRepository> = Arc::new(InMemoryTaskRepository::new());

        // Create the canonical agent registry (from server module)
        let agent_registry = Arc::new(AgentRegistry::new()); // Assuming AgentRegistry::new() exists

        // Create the canonical client manager (from server module)
        // It needs the registry
        let client_manager = Arc::new(ClientManager::new(agent_registry.clone()));

        // Create our custom task router implementation
        let bidirectional_task_router: Arc<dyn TaskRouterTrait> = Arc::new(BidirectionalTaskRouter::new(llm.clone(), agent_directory.clone()));

        // Create our custom tool executor implementation
        let bidirectional_tool_executor: Arc<dyn ToolExecutorTrait> = Arc::new(BidirectionalToolExecutor::new());

        // Create the task service using the canonical components and our trait implementations
        // Ensure TaskService::bidirectional exists and takes these arguments
        let task_service = Arc::new(TaskService::bidirectional(
            task_repository.clone(),
            Some(bidirectional_task_router), // Pass our TaskRouterTrait implementation
            Some(bidirectional_tool_executor), // Pass our ToolExecutorTrait implementation
            Some(client_manager.clone()),    // Pass canonical ClientManager
            // TaskService::bidirectional might not need agent_registry directly if ClientManager handles it
            // config.agent_id.clone(), // TaskService might not need agent_id directly
        ));

        // Create the streaming service
        let streaming_service = Arc::new(StreamingService::new(task_repository.clone()));

        // Create the notification service (pass repository)
        let notification_service = Arc::new(NotificationService::new(task_repository.clone()));

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

            port: config.port,
            bind_address: config.bind_address,
            agent_id: config.agent_id,
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

                let task_params = TaskSendParams {
                    id: task_id.clone(), // Reuse task ID or generate new? Depends on protocol.
                    message: initial_message, // Send the initial user message
                    metadata: metadata_map,
                    history_length: Some(10), // Example history length
                    push_notification: None,
                    session_id: None, // Add missing field
                };

                // Send the task using send_task which takes TaskSendParams
                let delegated_task = client.send_task(task_params).await?;
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
                    let query_params = TaskQueryParams {
                        id: delegated_task.id.clone(),
                        history_length: None,
                        metadata: None,
                    };
                    completed_task = client.get_task(query_params).await?;
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

        // Start the server
        let server_handle = run_server(
            self.port,
            &self.bind_address,
            self.task_service.clone(),
            self.streaming_service.clone(),
            self.notification_service.clone(),
            shutdown_token.clone(),
        ).await.map_err(anyhow::Error::from)?; // Map the Box<dyn Error>

        println!("Server running on {}:{}", self.bind_address, self.port);

        // Wait for the server to complete
        // The JoinHandle contains Result<(), JoinError>
        // The run_server function itself returns Result<JoinHandle<Result<(), ServerError>>, Box<dyn StdError + Send + Sync>>
        // We need to handle both potential errors.
        match server_handle.await {
             Ok(server_result) => {
                 match server_result {
                     Ok(()) => {
                        info!("Server shut down gracefully.");
                        Ok(())
                     }
                     Err(server_err) => {
                        error!("Server execution failed: {}", server_err);
                        Err(anyhow!("Server execution error: {}", server_err))
                     }
                 }
             }
            Err(join_err) => {
                error!("Failed to join server task: {}", join_err);
                Err(anyhow!("Server join error: {}", join_err))
            }
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

/// Configuration for the bidirectional agent
#[derive(Clone, Debug)]
pub struct BidirectionalAgentConfig {
    pub port: u16,
    pub bind_address: String,
    pub agent_id: String,
    pub claude_api_key: Option<String>,
}

impl Default for BidirectionalAgentConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            bind_address: DEFAULT_BIND_ADDRESS.to_string(),
            agent_id: format!("bidirectional-{}", Uuid::new_v4()),
            claude_api_key: None,
        }
    }
}

// REMOVED unused TaskSendParamsInput struct

/// Main entry point
#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing subscriber
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();

    // Create a configuration
    let mut config = BidirectionalAgentConfig::default();

    // Basic argument parsing
    for i in 1..args.len() {
        match args[i].as_str() {
            "--port" if i + 1 < args.len() => {
                config.port = args[i + 1].parse().unwrap_or(DEFAULT_PORT);
            },
            "--bind" if i + 1 < args.len() => {
                config.bind_address = args[i + 1].clone();
            },
            "--agent-id" if i + 1 < args.len() => {
                config.agent_id = args[i + 1].clone();
            },
            "--claude-api-key" if i + 1 < args.len() => {
                config.claude_api_key = Some(args[i + 1].clone());
            },
            "--message" if i + 1 < args.len() => {
                // Interactive mode - just process a single message

                // Check if we have an API key
                let api_key = config.claude_api_key.clone()
                    .or_else(|| std::env::var("CLAUDE_API_KEY").ok())
                    .ok_or_else(|| anyhow!("No Claude API key provided. Use --claude-api-key or set CLAUDE_API_KEY environment variable."))?;

                config.claude_api_key = Some(api_key);

                // Create the agent
                let agent = BidirectionalAgent::new(config)?;

                // Process the message using the direct method
                let message = args[i + 1].clone();
                println!("Processing message: '{}'", message);
                let response = agent.process_message_directly(&message).await?;

                println!("Response:\n{}", response);
                return Ok(());
            },
            _ => {} // Ignore unknown flags or flags without values for simplicity
        }
    }

    // Check for environment variables if not provided as arguments
    if config.claude_api_key.is_none() {
        config.claude_api_key = std::env::var("CLAUDE_API_KEY").ok();
        if config.claude_api_key.is_none() {
             warn!("CLAUDE_API_KEY environment variable not set. LLM features will be unavailable unless --claude-api-key is used.");
             // Decide if this should be a fatal error or just a warning
             // For now, let it proceed, but new() might fail later if LLM is required.
        }
    }

    // Create and run the agent
    info!("Starting agent with config: {:?}", config);
    let agent = BidirectionalAgent::new(config)?;
    agent.run().await
}
