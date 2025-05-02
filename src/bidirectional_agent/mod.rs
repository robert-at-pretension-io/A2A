/// Bidirectional A2A Agent Implementation
///
/// This module contains the core logic for an agent that can act as both an
/// A2A client and server, enabling task delegation and local execution.
/// All components are fully A2A protocol compliant.

use std::sync::Arc;
use tokio::runtime::Runtime;
use anyhow::{Result, Context};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use std::sync::Mutex as StdMutex; // Use std Mutex for handles
use url::Url;
use crate::bidirectional_agent::tools::{RemoteToolRegistry, RemoteToolExecutor};
use crate::bidirectional_agent::agent_directory::ActiveAgentEntry; // Import for tool discovery
use std::time::Duration; // Add this for Duration::from_secs()

// Core modules
pub mod config;
pub mod agent_registry;
pub mod client_manager;
pub mod error;
pub mod types;
pub mod agent_directory;
pub mod state_history;
pub mod push_notification;
pub mod content_negotiation; 
// Local Execution & Routing modules
pub mod tool_executor;
pub mod task_router;
pub mod task_router_llm;
pub mod task_router_llm_refactored;
pub mod task_router_unified;
pub mod tools;
pub mod llm_routing; 
pub mod llm_core;

// Delegation & Streaming modules
pub mod task_flow;
pub mod result_synthesis;
pub mod task_extensions;
pub mod protocol_router;
pub mod streaming;
pub mod sse_formatter;
pub mod standard_parts;

// Re-export key types for easier access
pub use config::BidirectionalAgentConfig;
pub use agent_registry::AgentRegistry;
pub use client_manager::ClientManager;
pub use error::AgentError;
pub use agent_directory::AgentDirectory;
pub use state_history::{StateHistoryTracker, StateHistoryConfig};
pub use push_notification::{NotificationService, NotificationServiceConfig};
pub use content_negotiation::{ContentNegotiator, ContentType};
pub mod task_metadata;
pub use task_metadata::{MetadataManager, TaskOrigin};

// Re-export local execution types
pub use tool_executor::ToolExecutor;
pub use task_router::{TaskRouter, LlmTaskRouterTrait, RoutingDecision};
pub use task_router_llm::{LlmTaskRouter, create_llm_task_router};
pub use task_router_llm_refactored::{RefactoredLlmTaskRouter, create_refactored_llm_task_router, LlmRoutingConfig as RefactoredLlmRoutingConfig};
pub use task_router_unified::{UnifiedTaskRouter, create_unified_task_router};
pub use llm_routing::LlmRoutingConfig;
pub use llm_core::TemplateManager;

// Re-export delegation and streaming types
pub use llm_routing::SynthesisAgent;
pub use task_flow::TaskFlow;
pub use result_synthesis::ResultSynthesizer;
pub use task_extensions::TaskRepositoryExt;
pub use protocol_router::{ProtocolRouter, ProtocolMethod, ExecutionStrategy, ExecutionStrategySelector};
pub use streaming::{StreamingTaskHandler, StreamEvent, SseStream};
pub use sse_formatter::SseFormatter;
pub use standard_parts::{StandardToolCall, format_tool_call_message, extract_tool_calls_from_parts};

/// Main struct representing the Bidirectional Agent.
#[derive(Clone)]
pub struct BidirectionalAgent {
    // Core configuration
    pub config: Arc<BidirectionalAgentConfig>,
    
    // Core components
    pub agent_registry: Arc<AgentRegistry>,
    pub client_manager: Arc<ClientManager>,
    pub agent_directory: Arc<AgentDirectory>,
    pub task_repository: Arc<dyn crate::server::repositories::task_repository::TaskRepository>,
    pub state_history: Arc<StateHistoryTracker>,
    pub notification_service: Arc<NotificationService>,
    pub content_negotiator: Arc<ContentNegotiator>,
    
    // Local execution components
    pub tool_executor: Arc<ToolExecutor>,
    pub task_router: Arc<dyn LlmTaskRouterTrait>,
    
    // Delegation & streaming components
    pub protocol_router: Arc<ProtocolRouter>,
    pub task_flow: Arc<TaskFlow>,
    pub remote_tool_registry: Arc<RemoteToolRegistry>,
    pub remote_tool_executor: Arc<RemoteToolExecutor>,
    pub result_synthesizer: Arc<ResultSynthesizer>,
    
    // Background task management
    cancellation_token: CancellationToken,
    server_handle: Arc<StdMutex<Option<JoinHandle<()>>>>,
    background_tasks: Arc<StdMutex<Vec<JoinHandle<()>>>>,
}

impl BidirectionalAgent {
    /// Creates a new BidirectionalAgent instance. (Async because initialization is async).
    pub async fn new(config: BidirectionalAgentConfig) -> Result<Self> {
        let config_arc = Arc::new(config);

        // Initialize AgentDirectory first (requires config.directory)
        let agent_directory = Arc::new(AgentDirectory::new(&config_arc.directory).await
            .context("Failed to initialize Agent Directory")?);

        // Initialize AgentRegistry
        let agent_registry = Arc::new(AgentRegistry::new(agent_directory.clone()));

        // Initialize task repository
        let task_repository = Arc::new(crate::server::repositories::task_repository::InMemoryTaskRepository::new());
        
        // Initialize state history tracker
        let state_history_config = StateHistoryConfig {
            storage_dir: format!("./state_history_{}", config_arc.self_id),
            max_history_size: 100,
            persist_to_disk: true,
        };
        
        let state_history = Arc::new(StateHistoryTracker::new(state_history_config)
            .context("Failed to initialize State History Tracker")?);
            
        // Initialize notification service
        let notification_config = NotificationServiceConfig {
            max_retries: 3,
            retry_base_delay_ms: 500,
            jwt_signing_key: Some("a2a_test_suite_key".to_string()), // In production, use a secure key
            retry_strategy: push_notification::RetryStrategy::Exponential,
        };
        
        let notification_service = Arc::new(NotificationService::new(notification_config)
            .context("Failed to initialize Notification Service")?);

        // Create default agent card for content negotiation
        let default_card = crate::types::AgentCard {
            name: config_arc.self_id.clone(),
            version: "1.0".to_string(),
            url: config_arc.base_url.clone(),
            capabilities: crate::types::AgentCapabilities {
                streaming: true,
                push_notifications: true,
                state_transition_history: true,
            },
            description: Some("Bidirectional A2A Agent".to_string()),
            documentation_url: None,
            authentication: None,
            default_input_modes: vec![
                "text/plain".to_string(),
                "text/markdown".to_string(),
                "application/json".to_string(),
            ],
            default_output_modes: vec![
                "text/plain".to_string(),
                "text/markdown".to_string(),
                "application/json".to_string(),
            ],
            skills: vec![],
            provider: None,
        };
        
        let content_negotiator = Arc::new(ContentNegotiator::new(default_card));

        // Initialize tool-related components
        let tool_executor = Arc::new(ToolExecutor::new(
            agent_directory.clone(),
            None, // Will be set later when RemoteToolExecutor is initialized
        ));

        // Initialize task router - use unified router
        let task_router = create_unified_task_router(
            agent_registry.clone(),
            tool_executor.clone(),
            config_arc.llm.use_for_routing,
        ).context("Failed to create Task Router")?;

        // Initialize delegation components
        let remote_tool_registry = Arc::new(RemoteToolRegistry::new(agent_directory.clone()));
        
        // Initialize client manager 
        let client_manager = Arc::new(ClientManager::new(
            agent_registry.clone(),
            config_arc.clone(),
        ).context("Failed to initialize Client Manager")?);

        // Initialize RemoteToolExecutor now that client_manager is available
        let remote_tool_executor = Arc::new(RemoteToolExecutor::new(
            client_manager.clone(),
            remote_tool_registry.clone()
        ));

        // Initialize protocol router for delegation
        // For now, implement ProtocolRouter with direct dependencies
        // We'll refactor this to use proper traits later
        let protocol_router = Arc::new(ProtocolRouter::new(
            task_repository.clone(),
            task_router.clone(),
            tool_executor.clone(),
        ));

        // Initialize TaskFlow for task lifecycle management
        let task_flow = Arc::new(TaskFlow::new(
            config_arc.self_id.clone(), // Use self_id as the default task ID
            config_arc.self_id.clone(), // Use self_id as the agent ID
            task_repository.clone(),
            client_manager.clone(),
            tool_executor.clone(),
            agent_registry.clone(),
        ));

        // Initialize result synthesizer
        let synthesis_agent = if config_arc.llm.use_for_routing {
            match std::env::var("ANTHROPIC_API_KEY") {
                Ok(api_key) => {
                    match llm_routing::create_synthesis_agent(Some(api_key)) {
                        Ok(agent) => Some(agent),
                        Err(e) => {
                            println!("⚠️ Failed to create synthesis agent: {}", e);
                            None
                        }
                    }
                },
                Err(_) => None,
            }
        } else {
            None
        };
        
        // Create a placeholder parent task ID - this will be set properly when tasks are processed
        let parent_task_id = "placeholder".to_string();
        
        let result_synthesizer = Arc::new(ResultSynthesizer::new(
            parent_task_id,
            task_repository.clone()
        ));

        // Build and return the agent
        Ok(Self {
            config: config_arc,
            agent_registry,
            agent_directory,
            client_manager,
            task_repository,
            state_history: state_history as Arc<StateHistoryTracker>,
            notification_service,
            content_negotiator,
            tool_executor,
            task_router,
            protocol_router,
            task_flow,
            remote_tool_registry,
            remote_tool_executor,
            result_synthesizer,
            cancellation_token: CancellationToken::new(),
            server_handle: Arc::new(StdMutex::new(None)),
            background_tasks: Arc::new(StdMutex::new(Vec::new())),
        })
    }

    /// Initializes and runs the agent's core services, including the server and background tasks.
    /// Binds to the address and port derived from the `base_url` in the configuration.
    pub async fn run(&self) -> Result<()> {
        // Parse base_url to get host and port for binding
        let base_url = Url::parse(&self.config.base_url)
            .with_context(|| format!("Invalid base_url in configuration: {}", self.config.base_url))?;

        let bind_addr = base_url.host_str().unwrap_or("127.0.0.1"); // Default to localhost if host missing
        let port = base_url.port_or_known_default().unwrap_or(8080); // Default to 8080 if port missing

        println!("🚀 Bidirectional Agent '{}' starting...", self.config.self_id);
        println!("   Derived server binding from base_url: {}:{}", bind_addr, port);

        // --- Agent Discovery ---
        println!("🔍 Discovering initial agents...");
        for url in &self.config.discovery {
            match self.agent_registry.discover(url).await {
                Ok(_) => println!("  ✅ Discovered agent at {}", url),
                Err(e) => println!("  ⚠️ Failed to discover agent at {}: {}", url, e),
            }
        }
        println!("✅ Agent discovery complete.");

        // --- Start Background Tasks ---
        
        // Start Agent Registry refresh loop
        let registry_clone = self.agent_registry.clone();
        let refresh_interval = chrono::Duration::minutes(5); // Every 5 minutes
        let registry_token = self.cancellation_token.child_token();
        let registry_handle = tokio::spawn(async move {
            tokio::select! {
                _ = registry_token.cancelled() => println!("Registry refresh loop canceled."),
                _ = registry_clone.run_refresh_loop(refresh_interval) => {} // Run the loop
            }
        });
        self.background_tasks.lock().expect("Mutex poisoned").push(registry_handle);
        println!("✅ Started agent registry refresh loop.");

        // Start Agent Directory verification loop
        let directory_clone = self.agent_directory.clone();
        let directory_token = self.cancellation_token.child_token();
        let directory_handle = tokio::spawn(async move {
            // Pass the token to the loop function
            if let Err(e) = directory_clone.run_verification_loop(directory_token).await {
                log::error!(
                    target: "agent_directory", // Log target for filtering
                    "Directory verification loop exited with error: {:?}",
                    e
                );
            }
        });
        self.background_tasks.lock().expect("Mutex poisoned").push(directory_handle);
        println!("✅ Started agent directory verification loop.");

        // Start delegated task polling
        let client_manager_clone = self.client_manager.clone();
        let poll_interval = chrono::Duration::seconds(30); // 30 seconds
        let poll_token = self.cancellation_token.child_token();
        let poll_handle = tokio::spawn(async move {
            tokio::select! {
                _ = poll_token.cancelled() => println!("Delegated task polling loop canceled."),
                _ = client_manager_clone.run_delegated_task_poll_loop(poll_interval) => {}
            }
        });
        self.background_tasks.lock().expect("Mutex poisoned").push(poll_handle);
        println!("✅ Started delegated task polling loop.");

        // Start remote tool discovery
        let self_clone = self.clone();
        let tool_discovery_token = self.cancellation_token.child_token();
        let tool_discovery_handle = tokio::spawn(async move {
            if let Err(e) = self_clone.run_tool_discovery_loop(tool_discovery_token).await {
                log::error!(target: "tool_discovery", "Tool discovery loop exited with error: {:?}", e);
            }
        });
        self.background_tasks.lock().expect("Mutex poisoned").push(tool_discovery_handle);
        println!("✅ Started remote tool discovery loop.");

        // --- Start the A2A Server ---
        
        // Create TaskFlow service adapter for the server
        let task_service = Arc::new(crate::server::services::task_service::TaskService::with_task_flow(
            self.task_flow.clone(),
        ));
        
        // Create streaming service
        let streaming_service = Arc::new(crate::server::services::streaming_service::StreamingService::new(
            self.task_repository.clone(),
        ));
        
        // Create notification service
        let notification_service = Arc::new(crate::server::services::notification_service::NotificationService::new(
            self.task_repository.clone(),
        ));

        // Start the server
        let server_token = self.cancellation_token.child_token();
        let server_handle = crate::server::run_server(
            port,
            bind_addr,
            task_service,
            streaming_service,
            notification_service,
            server_token,
        ).await
         .map_err(|e| anyhow::anyhow!("Failed to start A2A server: {}", e))?;
         
        *self.server_handle.lock().expect("Mutex poisoned") = Some(server_handle);
        println!("✅ A2A Server started successfully.");

        // Keep the agent running until interrupted
        println!("✅ Bidirectional Agent '{}' running. Press Ctrl+C to stop.", self.config.self_id);

        // Wait for shutdown signal
        tokio::signal::ctrl_c().await?;

        // Initiate graceful shutdown
        self.shutdown().await?;

        Ok(())
    }

    /// Background task to periodically discover tools from other active agents.
    async fn run_tool_discovery_loop(&self, token: CancellationToken) -> Result<()> {
        let interval_minutes = self.config.tool_discovery_interval_minutes;
        if interval_minutes == 0 {
            log::warn!(target: "tool_discovery", "Tool discovery interval is 0, discovery loop will not run.");
            return Ok(());
        }
        let interval = chrono::Duration::minutes(interval_minutes as i64);
        log::info!(target: "tool_discovery", "Starting tool discovery loop with interval: {:?}", interval);

        // Use interval_at to start the first tick immediately or after a short delay
        let mut interval_timer = tokio::time::interval_at(
            tokio::time::Instant::now() + Duration::from_secs(5), // Start after 5s delay
            interval.to_std()?
        );

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    log::info!(target: "tool_discovery", "Tool discovery loop cancelled.");
                    return Ok(());
                }
                _ = interval_timer.tick() => {
                    log::debug!(target: "tool_discovery", "Running tool discovery scan...");
                    // Spawn discovery in a separate task to avoid blocking the loop timer
                    let self_clone = self.clone(); // Clone Arc<Self>
                    tokio::spawn(async move {
                        match self_clone.discover_remote_tools().await {
                            Ok(count) => log::info!(target: "tool_discovery", "Tool discovery scan complete. Updated/found tools for {} agents.", count),
                            Err(e) => log::error!(target: "tool_discovery", "Error during tool discovery scan: {:?}", e),
                        }
                    });
                }
            }
        }
    }

    /// Performs a single scan to discover tools from active agents.
    async fn discover_remote_tools(&self) -> Result<usize> {
        let active_agents: Vec<ActiveAgentEntry> = self.agent_directory.get_active_agents().await
            .context("Failed to get active agents for tool discovery")?;

        if active_agents.is_empty() {
            log::debug!(target: "tool_discovery", "No active agents found to discover tools from.");
            return Ok(0);
        }

        log::info!(target: "tool_discovery", "Discovering tools from {} active agents...", active_agents.len());
        let mut updated_count = 0;

        // Use futures::stream to process agents concurrently 
        use futures::stream::{self, StreamExt, TryStreamExt};
        let results = stream::iter(active_agents)
            .map(|agent_entry| async move {
                let agent_id = agent_entry.agent_id;
                let agent_url = agent_entry.url;
                log::debug!(target: "tool_discovery", "Querying tools for agent: {} at {}", agent_id, agent_url);

                // Get agent card from ClientManager
                match self.client_manager.get_agent_card(&agent_id).await {
                    Ok(Some(card)) => {
                        // Update tools from card in RemoteToolRegistry
                        match self.remote_tool_registry.update_tools_from_card(&agent_id, &card).await {
                            Ok(_) => {
                                log::debug!(target: "tool_discovery", "Successfully updated tools for agent: {}", agent_id);
                                Ok::<bool, anyhow::Error>(true) // Indicate update occurred
                            }
                            Err(e) => {
                                log::warn!(target: "tool_discovery", "Failed to update tools for agent {}: {:?}", agent_id, e);
                                Ok::<bool, anyhow::Error>(false) // No update, but not a fatal error
                            }
                        }
                    }
                    Ok(None) => {
                        log::warn!(target: "tool_discovery", "Could not find agent card for active agent: {}", agent_id);
                        Ok::<bool, anyhow::Error>(false)
                    }
                    Err(e) => {
                        log::warn!(target: "tool_discovery", "Error fetching agent card for {}: {:?}", agent_id, e);
                        Ok::<bool, anyhow::Error>(false)
                    }
                }
            })
            .buffer_unordered(10) // Limit concurrency
            .try_collect::<Vec<bool>>()
            .await;

        match results {
            Ok(update_flags) => {
                updated_count = update_flags.into_iter().filter(|&updated| updated).count();
                Ok(updated_count)
            }
            Err(e) => {
                Err(anyhow::anyhow!("Tool discovery stream processing failed: {}", e))
            }
        }
    }

    /// Initiates graceful shutdown of the agent.
    pub async fn shutdown(&self) -> Result<()> {
        println!("\n🛑 Shutting down Bidirectional Agent...");

        // Cancel all background tasks and the server
        self.cancellation_token.cancel();

        // Wait for the server task to complete
        // Take the handle out of the mutex to avoid holding the MutexGuard across await
        let server_handle = {
            let mut lock = self.server_handle.lock().expect("Mutex poisoned");
            lock.take()
        };
        
        if let Some(handle) = server_handle {
             println!("   Waiting for server task to finish...");
             if let Err(e) = handle.await {
                 eprintln!("   Server task join error: {:?}", e);
             } else {
                 println!("   Server task finished.");
             }
        }

        // Wait for all background tasks to complete
        // Take handles out of the mutex to avoid holding the MutexGuard across await
        let handles = {
            let mut lock = self.background_tasks.lock().expect("Mutex poisoned");
            std::mem::take(&mut *lock)
        };
        println!("   Waiting for {} background tasks...", handles.len());
        for (i, handle) in handles.into_iter().enumerate() {
            if let Err(e) = handle.await {
                eprintln!("   Background task {} join error: {:?}", i, e);
            }
        }
         println!("   All background tasks finished.");

        println!("🏁 Bidirectional Agent shutdown complete.");
        Ok(())
    }
    
    /// Process a task using the A2A protocol flow
    pub async fn process_task(&self, params: crate::types::TaskSendParams) -> Result<crate::types::Task, AgentError> {
        self.task_flow.process_task(params).await
    }
    
    /// Find tools matching the specified capability or description
    pub async fn find_tools(&self, capability: &str) -> Vec<String> {
        let mut tools = Vec::new();
        
        // Check local tools
        for tool_name in self.tool_executor.tools.keys() {
            let tool_matches = match tool_name.as_str() {
                "echo" => capability.contains("echo") || capability.contains("text") || capability.contains("repeat"),
                "http" => capability.contains("http") || capability.contains("web") || capability.contains("fetch"),
                "shell" => capability.contains("shell") || capability.contains("command") || capability.contains("execute"),
                "directory" => capability.contains("directory") || capability.contains("agent") || capability.contains("list"),
                _ => false,
            };
            
            if tool_matches {
                tools.push(format!("local:{}", tool_name));
            }
        }
        
        // Remote tool lookup is temporarily disabled
        // Future implementation would check remote tool registry
        
        tools
    }
}


/// Entry point function called from `main.rs` when the `bidirectional` command is used.
pub async fn run(config_path: &str) -> Result<()> {
    // Load configuration
    let config = config::load_config(config_path)
        .with_context(|| format!("Failed to load agent config from '{}'", config_path))?;

    // Initialize the agent
    let agent = BidirectionalAgent::new(config).await
        .context("Failed to initialize Bidirectional Agent")?;

    // Run the agent (run method now handles port binding from config)
    agent.run().await
        .context("Agent run failed")?;

    Ok(())
}


// Basic tests module 
#[cfg(test)]
mod tests {
    use super::*;
    use config::{AuthConfig, NetworkConfig};

    #[test]
    fn test_agent_initialization_basic() {
        let config = BidirectionalAgentConfig {
            self_id: "test-agent-basic".to_string(),
            base_url: "http://localhost:8080".to_string(),
            discovery: vec![],
            auth: AuthConfig::default(),
            network: NetworkConfig::default(),
            tools: config::ToolConfigs::default(),
            directory: config::DirectoryConfig::default(),
            tool_discovery_interval_minutes: 30,
        };
        
        // BidirectionalAgent::new is async, use block_on for test
        let agent_result = tokio_test::block_on(BidirectionalAgent::new(config));
        assert!(agent_result.is_ok());
        
        let agent = agent_result.unwrap();
        
        // Check core components exist
        assert_eq!(agent.config.self_id, "test-agent-basic");
        assert!(Arc::strong_count(&agent.agent_registry) >= 1);
        assert!(Arc::strong_count(&agent.client_manager) >= 1);
        assert!(Arc::strong_count(&agent.agent_directory) >= 1);
        assert!(Arc::strong_count(&agent.state_history) >= 1);
        assert!(Arc::strong_count(&agent.notification_service) >= 1);
        assert!(Arc::strong_count(&agent.content_negotiator) >= 1);
        
        // Check tool components
        assert!(Arc::strong_count(&agent.tool_executor) >= 1);
        assert!(Arc::strong_count(&agent.task_router) >= 1);
        
        // Check delegation components
        assert!(Arc::strong_count(&agent.protocol_router) >= 1);
        assert!(Arc::strong_count(&agent.task_flow) >= 1);
        assert!(Arc::strong_count(&agent.remote_tool_registry) >= 1);
        assert!(Arc::strong_count(&agent.remote_tool_executor) >= 1);
        assert!(Arc::strong_count(&agent.result_synthesizer) >= 1);
    }
}

// Include the test utility and integration test modules when testing
#[cfg(test)]
mod test_utils;
#[cfg(test)]
mod integration_tests;