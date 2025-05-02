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
    run_server,
};

use crate::types::{
    Task, TaskState, Message, Part, TextPart, Role, AgentCard,
    TaskSendParams, TaskQueryParams, TaskIdParams, PushNotificationConfig,
    DataPart, FilePart,
};

use crate::bidirectional_agent::task_router;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use futures_util::{StreamExt, TryStreamExt};
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
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

// Helper Types

/// Task execution mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// Process the task locally with the agent
    Local,
    /// Delegate to a remote agent
    Remote { agent_id: String },
}

/// Entry in the agent directory
#[derive(Debug, Clone)]
struct AgentDirectoryEntry {
    /// Agent card with capabilities
    card: AgentCard,
    /// Last time this agent was seen
    last_seen: DateTime<Utc>,
    /// Whether the agent is currently active
    active: bool,
}

/// Simplified Agent Directory
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

    fn add_or_update_agent(&self, card: AgentCard) {
        let agent_id = card.id.clone();
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

/// Simplified Agent Registry for discovering and caching agent information
#[derive(Clone)]
struct AgentRegistry {
    directory: Arc<AgentDirectory>,
    http_client: reqwest::Client,
}

impl AgentRegistry {
    fn new(directory: Arc<AgentDirectory>) -> Self {
        Self {
            directory,
            http_client: reqwest::Client::new(),
        }
    }

    async fn discover_agent(&self, base_url: &str) -> Result<AgentCard> {
        // Create a temporary client for discovery
        let client = A2aClient::new(base_url);
        
        // Get the agent card from the well-known endpoint
        let card = client.get_agent_card().await
            .context("Failed to get agent card")?;
        
        // Add or update the agent in the directory
        self.directory.add_or_update_agent(card.clone());
        
        Ok(card)
    }
}

/// Simplified Client Manager for managing A2A clients
#[derive(Clone)]
struct ClientManager {
    clients: Arc<DashMap<String, A2aClient>>,
    registry: Arc<AgentRegistry>,
}

impl ClientManager {
    fn new(registry: Arc<AgentRegistry>) -> Self {
        Self {
            clients: Arc::new(DashMap::new()),
            registry,
        }
    }

    async fn get_client(&self, agent_id: &str) -> Result<A2aClient> {
        // Check if we already have a client for this agent
        if let Some(client) = self.clients.get(agent_id) {
            return Ok(client.value().clone());
        }

        // Get the agent info from the registry
        let agent = self.registry.directory.get_agent(agent_id)
            .ok_or_else(|| anyhow!("Agent not found: {}", agent_id))?;

        // Create a new client
        let base_url = agent.card.url.clone();
        let client = A2aClient::new(&base_url);

        // Store the client for reuse
        self.clients.insert(agent_id.to_string(), client.clone());

        Ok(client)
    }

    async fn delegate_task(&self, agent_id: &str, task_params: TaskSendParams) -> Result<Task> {
        let client = self.get_client(agent_id).await?;
        let task = client.send_task_with_metadata(
            &task_params.input.text,
            task_params.metadata.map(|m| m.to_string()).as_deref(),
        ).await?;
        
        Ok(task)
    }

    async fn get_delegated_task(&self, agent_id: &str, task_id: &str) -> Result<Task> {
        let client = self.get_client(agent_id).await?;
        let task = client.get_task(task_id).await?;
        
        Ok(task)
    }
}

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

/// Task Router for determining how to process tasks
#[derive(Clone)]
struct TaskRouter {
    llm: Arc<dyn LlmClient>,
    directory: Arc<AgentDirectory>,
}

impl TaskRouter {
    fn new(llm: Arc<dyn LlmClient>, directory: Arc<AgentDirectory>) -> Self {
        Self {
            llm,
            directory,
        }
    }

    async fn route_task(&self, task: &Task) -> Result<ExecutionMode> {
        // Extract the task content for analysis
        let task_text = task.messages.iter()
            .filter(|m| m.role == Role::User)
            .flat_map(|m| m.parts.iter())
            .filter_map(|p| match p {
                Part::TextPart(tp) => Some(tp.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        if task_text.is_empty() {
            return Ok(ExecutionMode::Local);
        }

        // Get the list of available agents
        let available_agents = self.directory.list_active_agents();
        let agent_descriptions = available_agents.iter()
            .map(|a| format!("ID: {}\nName: {}\nDescription: {}\nCapabilities: {}", 
                a.card.id, 
                a.card.name, 
                a.card.description.as_deref().unwrap_or(""), 
                a.card.capabilities.join(", ")))
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
            
            // Verify the agent exists
            if self.directory.get_agent(&agent_id).is_none() {
                return Ok(ExecutionMode::Local); // Fall back to local if agent not found
            }
            
            Ok(ExecutionMode::Remote { agent_id })
        } else {
            // Default to local if the decision isn't clear
            Ok(ExecutionMode::Local)
        }
    }
}

/// Tool Executor for running local tools
#[derive(Clone)]
struct ToolExecutor;

impl ToolExecutor {
    fn new() -> Self {
        Self
    }

    async fn execute(&self, _task: &Task) -> Result<String> {
        // For simplicity, just return a placeholder response
        // In a real implementation, this would execute actual tools
        Ok("Tool execution completed (placeholder)".to_string())
    }
}

/// Main bidirectional agent implementation
pub struct BidirectionalAgent {
    // Core components
    task_repository: Arc<InMemoryTaskRepository>,
    task_service: Arc<TaskService>,
    streaming_service: Arc<StreamingService>,
    notification_service: Arc<NotificationService>,
    
    // Bidirectional components
    agent_directory: Arc<AgentDirectory>,
    agent_registry: Arc<AgentRegistry>,
    client_manager: Arc<ClientManager>,
    task_router: Arc<TaskRouter>,
    tool_executor: Arc<ToolExecutor>,
    llm: Arc<dyn LlmClient>,
    
    // Server configuration
    port: u16,
    bind_address: String,
    agent_id: String,
}

impl BidirectionalAgent {
    pub fn new(config: BidirectionalAgentConfig) -> Result<Self> {
        // Create the agent directory
        let agent_directory = Arc::new(AgentDirectory::new());
        
        // Create the LLM client
        let llm: Arc<dyn LlmClient> = if let Some(api_key) = &config.claude_api_key {
            Arc::new(ClaudeLlmClient::new(api_key.clone(), SYSTEM_PROMPT.to_string()))
        } else {
            return Err(anyhow!("No LLM configuration provided"));
        };
        
        // Create the task repository
        let task_repository = Arc::new(InMemoryTaskRepository::new());
        
        // Create the agent registry
        let agent_registry = Arc::new(AgentRegistry::new(agent_directory.clone()));
        
        // Create the client manager
        let client_manager = Arc::new(ClientManager::new(agent_registry.clone()));
        
        // Create the task router
        let task_router = Arc::new(TaskRouter::new(llm.clone(), agent_directory.clone()));
        
        // Create the tool executor
        let tool_executor = Arc::new(ToolExecutor::new());
        
        // Create the task service
        let task_service = Arc::new(TaskService::bidirectional(
            task_repository.clone(),
            // Implement TaskRouter trait with our struct
            Arc::new(move |task: &Task| {
                let task_router = task_router.clone();
                let task = task.clone();
                Box::pin(async move {
                    task_router.route_task(&task).await
                        .map_err(|e| anyhow!(e.to_string()))
                        .map(|decision| match decision {
                            ExecutionMode::Local => {
                                // Convert to the server's RoutingDecision
                                task_router::RoutingDecision::Local { 
                                    tool_names: vec!["default".to_string()] 
                                }
                            },
                            ExecutionMode::Remote { agent_id } => {
                                // Convert to the server's RoutingDecision
                                task_router::RoutingDecision::Remote { 
                                    agent_id: agent_id.clone() 
                                }
                            },
                        })
                })
            }),
            // Implement ToolExecutor trait with our struct
            Arc::new(move |task: &Task, _tool_names: &[String]| {
                let tool_executor = tool_executor.clone();
                let task = task.clone();
                Box::pin(async move {
                    tool_executor.execute(&task).await
                        .map_err(|e| anyhow!(e.to_string()))
                        .map(|result| {
                            vec![Message {
                                role: Role::Assistant,
                                parts: vec![Part::TextPart(TextPart { 
                                    text: result
                                })],
                            }]
                        })
                })
            }),
            client_manager.clone(),
            agent_registry.clone(),
            config.agent_id.clone(),
        ));
        
        // Create the streaming service
        let streaming_service = Arc::new(StreamingService::new(task_repository.clone()));
        
        // Create the notification service
        let notification_service = Arc::new(NotificationService::new());
        
        Ok(Self {
            task_repository,
            task_service,
            streaming_service,
            notification_service,
            
            agent_directory,
            agent_registry,
            client_manager,
            task_router,
            tool_executor,
            llm,
            
            port: config.port,
            bind_address: config.bind_address,
            agent_id: config.agent_id,
        })
    }
    
    /// Process a message using the LLM
    pub async fn process_message(&self, message: &str) -> Result<String> {
        // Create a temporary task to hold the message
        let task = Task {
            id: Uuid::new_v4().to_string(),
            state: TaskState::Working,
            messages: vec![Message {
                role: Role::User,
                parts: vec![Part::TextPart(TextPart { 
                    text: message.to_string() 
                })],
            }],
            state_history: Vec::new(),
            artifacts: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        
        // Route the task
        let decision = self.task_router.route_task(&task).await?;
        
        match decision {
            ExecutionMode::Local => {
                // Process locally with the LLM
                let prompt = format!("Process this request and provide a helpful response:\n\n{}", message);
                self.llm.complete(&prompt).await
            },
            ExecutionMode::Remote { agent_id } => {
                // Delegate to remote agent
                let task_params = TaskSendParams {
                    input: TaskSendParamsInput {
                        text: message.to_string(),
                    },
                    metadata: Some(json!({
                        "origin": {
                            "agent_id": self.agent_id,
                            "timestamp": Utc::now().to_rfc3339(),
                        }
                    })),
                };
                
                let delegated_task = self.client_manager.delegate_task(&agent_id, task_params).await?;
                
                // Wait for the task to complete (simplified)
                let mut completed_task = delegated_task;
                for _ in 0..30 {
                    if completed_task.state == TaskState::Completed || 
                       completed_task.state == TaskState::Failed || 
                       completed_task.state == TaskState::Canceled {
                        break;
                    }
                    
                    sleep(std::time::Duration::from_secs(1)).await;
                    completed_task = self.client_manager.get_delegated_task(&agent_id, &delegated_task.id).await?;
                }
                
                // Extract the response
                let response = completed_task.messages.iter()
                    .filter(|m| m.role == Role::Assistant)
                    .flat_map(|m| m.parts.iter())
                    .filter_map(|p| match p {
                        Part::TextPart(tp) => Some(tp.text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                
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
        ).await?;
        
        println!("Server running on {}:{}", self.bind_address, self.port);
        
        // Wait for the server to complete
        server_handle.await.map_err(|e| anyhow!("Server error: {}", e))?;
        
        Ok(())
    }
    
    /// Create an agent card for discovery
    pub fn create_agent_card(&self) -> AgentCard {
        AgentCard {
            id: self.agent_id.clone(),
            name: AGENT_NAME.to_string(),
            description: Some("A bidirectional A2A agent that can process tasks and delegate to other agents".to_string()),
            version: Some(AGENT_VERSION.to_string()),
            url: format!("http://{}:{}", self.bind_address, self.port),
            capabilities: vec!["texttask".to_string(), "bidirectional".to_string()],
            auth_required: false,
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

// TaskSendParams input struct for easier JSON deserialization
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskSendParamsInput {
    text: String,
}

/// Main entry point
#[tokio::main]
async fn main() -> Result<()> {
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
                
                // Process the message
                let message = args[i + 1].clone();
                let response = agent.process_message(&message).await?;
                
                println!("{}", response);
                return Ok(());
            },
            _ => {}
        }
    }
    
    // Check for environment variables if not provided as arguments
    if config.claude_api_key.is_none() {
        config.claude_api_key = std::env::var("CLAUDE_API_KEY").ok();
    }
    
    // Create and run the agent
    let agent = BidirectionalAgent::new(config)?;
    agent.run().await
}
