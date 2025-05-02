/// Bidirectional A2A Agent Implementation
///
/// This module contains the core logic for an agent that can act as both an
/// A2A client and server, enabling task delegation and local execution.

// Only compile this module if the 'bidir-core' feature (or higher) is enabled.
#[cfg(feature = "bidir-core")]
use std::sync::Arc;
#[cfg(feature = "bidir-core")]
use tokio::runtime::Runtime;
#[cfg(feature = "bidir-core")]
use anyhow::{Result, Context};
#[cfg(feature = "bidir-core")]
use tokio::task::JoinHandle;
#[cfg(feature = "bidir-core")]
use tokio_util::sync::CancellationToken;
#[cfg(feature = "bidir-core")]
use std::sync::Mutex as StdMutex; // Use std Mutex for handles
#[cfg(feature = "bidir-core")]
use url::Url;
#[cfg(feature = "bidir-delegate")]
use crate::bidirectional_agent::tools::{RemoteToolRegistry, RemoteToolExecutor};
#[cfg(feature = "bidir-delegate")]
use crate::bidirectional_agent::agent_directory::ActiveAgentEntry; // Import for tool discovery
use std::time::Duration; // Add this for Duration::from_secs()

// Public submodules (conditionally compiled based on features)
#[cfg(feature = "bidir-core")]
pub mod config;
#[cfg(feature = "bidir-core")]
pub mod agent_registry;
#[cfg(feature = "bidir-core")]
pub mod client_manager;
#[cfg(feature = "bidir-core")]
pub mod error;
#[cfg(feature = "bidir-core")]
pub mod types;
#[cfg(feature = "bidir-core")]
pub mod agent_directory; // Add agent_directory module declaration

// Slice 2: Local Execution & Routing
#[cfg(feature = "bidir-local-exec")]
pub mod tool_executor;
#[cfg(feature = "bidir-local-exec")]
pub mod task_router;
#[cfg(feature = "bidir-local-exec")]
pub mod tools;
#[cfg(feature = "bidir-local-exec")]
pub mod llm_routing; // LLM-based routing module
#[cfg(feature = "bidir-local-exec")]
pub mod task_router_llm; // LLM task router implementation

// Slice 3: Delegation & Synthesis
#[cfg(feature = "bidir-delegate")]
pub mod task_flow;
#[cfg(feature = "bidir-delegate")]
pub mod result_synthesis;
#[cfg(feature = "bidir-delegate")]
pub mod task_extensions;


// Re-export key types for easier access
#[cfg(feature = "bidir-core")]
pub use config::BidirectionalAgentConfig;
#[cfg(feature = "bidir-core")]
pub use agent_registry::AgentRegistry;
#[cfg(feature = "bidir-core")]
pub use client_manager::ClientManager;
#[cfg(feature = "bidir-core")]
pub use error::AgentError;
pub use types::{ToolCall, ToolCallPart, ExtendedPart};
#[cfg(feature = "bidir-core")]
pub use agent_directory::AgentDirectory; // Re-export AgentDirectory
// Add imports for Slice 2 components
#[cfg(feature = "bidir-local-exec")]
pub use tool_executor::ToolExecutor;
#[cfg(feature = "bidir-local-exec")]
pub use task_router::{TaskRouter, LlmTaskRouterTrait}; // Export standard router and the trait from task_router
#[cfg(feature = "bidir-local-exec")]
pub use task_router_llm::{LlmTaskRouter, create_llm_task_router}; // Export LLM router implementation and factory
#[cfg(feature = "bidir-local-exec")]
pub use llm_routing::{LlmRoutingConfig, RoutingAgent}; // SynthesisAgent is gated by bidir-delegate
// Add imports for Slice 3 components
#[cfg(feature = "bidir-delegate")]
pub use llm_routing::SynthesisAgent; // Import SynthesisAgent only when delegate is enabled
#[cfg(feature = "bidir-delegate")]
pub use task_flow::TaskFlow;
#[cfg(feature = "bidir-delegate")]
pub use result_synthesis::ResultSynthesizer;
#[cfg(feature = "bidir-delegate")]
pub use task_extensions::TaskRepositoryExt;

#[cfg(feature = "bidir-core")]
/// Main struct representing the Bidirectional Agent.
#[derive(Clone)]
pub struct BidirectionalAgent {
    pub config: Arc<BidirectionalAgentConfig>,
    pub agent_registry: Arc<AgentRegistry>,
    pub client_manager: Arc<ClientManager>,
    #[cfg(feature = "bidir-core")]
    pub agent_directory: Arc<AgentDirectory>, // Add agent directory field
    // Add components for Slice 2
    #[cfg(feature = "bidir-local-exec")]
    pub tool_executor: Arc<ToolExecutor>,
    #[cfg(feature = "bidir-local-exec")]
    pub task_router: Arc<dyn LlmTaskRouterTrait>, // Use the trait for polymorphism
    // Add components for Slice 3 (Delegation)
    #[cfg(feature = "bidir-delegate")]
    pub remote_tool_registry: Arc<RemoteToolRegistry>,
    #[cfg(feature = "bidir-delegate")]
    pub remote_tool_executor: Arc<RemoteToolExecutor>,
    // Use the concrete type from the server module
    pub task_repository: Arc<crate::server::repositories::task_repository::InMemoryTaskRepository>,
    // Add handles for server and background tasks
    cancellation_token: CancellationToken,
    // Use std::sync::Mutex for simple state like handles that don't need async locking
    server_handle: Arc<StdMutex<Option<JoinHandle<()>>>>,
    background_tasks: Arc<StdMutex<Vec<JoinHandle<()>>>>,
}

#[cfg(feature = "bidir-core")]
impl BidirectionalAgent {
    /// Creates a new BidirectionalAgent instance. (Async because AgentDirectory::new is async).
    pub async fn new(config: BidirectionalAgentConfig) -> Result<Self> {
        let config_arc = Arc::new(config);

        // Initialize AgentDirectory first (requires config.directory)
        #[cfg(feature = "bidir-core")]
        let agent_directory = Arc::new(AgentDirectory::new(&config_arc.directory).await
            .context("Failed to initialize Agent Directory")?);

        // Initialize AgentRegistry, passing the directory if the feature is enabled
        #[cfg(feature = "bidir-core")]
        let agent_registry = Arc::new(AgentRegistry::new(agent_directory.clone()));
        // If bidir-core is not enabled, AgentRegistry might need a different constructor or be stubbed.
        // Assuming for now that if AgentRegistry exists, bidir-core is enabled.
        // If AgentRegistry is used without bidir-core, this needs adjustment:
        #[cfg(not(feature = "bidir-core"))]
        let agent_registry = Arc::new(AgentRegistry::new());


        // Use the concrete InMemoryTaskRepository from the server module
        let task_repository = Arc::new(crate::server::repositories::task_repository::InMemoryTaskRepository::new());

        // Initialize Slice 2 components if feature is enabled
        // Initialize Slice 2 components if feature is enabled
        #[cfg(feature = "bidir-local-exec")]
        let tool_executor = Arc::new(ToolExecutor::new(agent_directory.clone())); // Pass directory to ToolExecutor

        // Initialize task router - potentially use LLM-based routing if configured
        #[cfg(feature = "bidir-local-exec")]
        let task_router: Arc<dyn LlmTaskRouterTrait> = {
            // Ensure agent_registry is available here if needed by routers
            let registry_for_router = agent_registry.clone();
            let executor_for_router = tool_executor.clone();

            if config_arc.tools.use_llm_routing {
                 println!("🧠 Using LLM-powered task router");
                 // Configure LLM router with API key if available
                 let llm_config = config_arc.tools.llm_api_key.as_ref().map(|api_key| {
                     llm_routing::LlmRoutingConfig {
                         model: config_arc.tools.llm_model.clone(),
                         max_tokens: 1024,
                         temperature: 0.7,
                         routing_prompt_template: "Make a routing decision for this task".to_string(),
                         decomposition_prompt_template: "Decompose this task".to_string(),
                     }
                 });

                 // Create LLM task router using the factory
                 // Note: the LLM config is not used in the current implementation
                 create_llm_task_router( // Use the factory function directly
                     registry_for_router,
                     executor_for_router
                 )
            } else {
                 // Use the standard task router if LLM routing is not enabled
                 println!("📝 Using standard rule-based task router");
                 Arc::new(TaskRouter::new(registry_for_router, executor_for_router)) as Arc<dyn LlmTaskRouterTrait>
            }
        };

        let client_manager = Arc::new(ClientManager::new(
            agent_registry.clone(),
            config_arc.clone(),
        )?);

        // Initialize Slice 3 components if feature is enabled
        #[cfg(feature = "bidir-delegate")]
        let remote_tool_registry = Arc::new(RemoteToolRegistry::new(agent_directory.clone())); // Needs AgentDirectory
        #[cfg(feature = "bidir-delegate")]
        let remote_tool_executor = Arc::new(RemoteToolExecutor::new(client_manager.clone())); // Needs ClientManager

        Ok(Self {
            config: config_arc,
            agent_registry, // Store registry
            client_manager, // Store client manager
            #[cfg(feature = "bidir-core")]
            agent_directory, // Store agent directory
            task_repository, // Store task repository
            // Initialize Slice 2 components if feature is enabled
            #[cfg(feature = "bidir-local-exec")]
            tool_executor,
            #[cfg(feature = "bidir-local-exec")]
            task_router,
            // Initialize Slice 3 components if feature is enabled
            #[cfg(feature = "bidir-delegate")]
            remote_tool_registry,
            #[cfg(feature = "bidir-delegate")]
            remote_tool_executor,
            // Initialize cancellation token and handles
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

        // --- Start Background Tasks (Registry Refresh, Delegated Task Polling) ---
        let registry_clone = self.agent_registry.clone();
        let refresh_interval = chrono::Duration::minutes(5); // Example interval
        let registry_token = self.cancellation_token.child_token(); // Create child token
        let registry_handle = tokio::spawn(async move {
            tokio::select! {
                _ = registry_token.cancelled() => println!("Registry refresh loop canceled."),
                _ = registry_clone.run_refresh_loop(refresh_interval) => {} // Run the loop
            }
        });
        // Use std::sync::Mutex correctly
        self.background_tasks.lock().expect("Mutex poisoned").push(registry_handle);
        println!("✅ Started agent registry refresh loop.");

        // Start agent directory verification loop if feature is enabled
        #[cfg(feature = "bidir-core")]
        {
            let directory_clone = self.agent_directory.clone();
            let directory_token = self.cancellation_token.child_token();
            let directory_handle = tokio::spawn(async move {
                // Pass the token to the loop function
                if let Err(e) = directory_clone.run_verification_loop(directory_token).await {
                    // Use log::error instead of println for errors
                    log::error!(
                        target: "agent_directory", // Log target for filtering
                        "Directory verification loop exited with error: {:?}",
                        e
                    );
                }
            });
            self.background_tasks.lock().expect("Mutex poisoned").push(directory_handle);
            println!("✅ Started agent directory verification loop.");
        }


        #[cfg(feature = "bidir-delegate")]
        {
            let client_manager_clone = self.client_manager.clone();
            let poll_interval = chrono::Duration::seconds(30); // Example interval
            let poll_token = self.cancellation_token.child_token(); // Create child token
            let delegation_poll_handle = tokio::spawn(async move {
                 tokio::select! {
                    _ = poll_token.cancelled() => println!("Delegated task polling loop canceled."),
                    // Pass repository to poll loop (will be done in Milestone 3)
                    _ = client_manager_clone.run_delegated_task_poll_loop(poll_interval) => {}
                 }
            });
            self.background_tasks.lock().expect("Mutex poisoned").push(delegation_poll_handle);
            println!("✅ Started delegated task polling loop.");
        }

        // Start remote tool discovery loop if feature is enabled
        #[cfg(feature = "bidir-delegate")]
        {
            let self_clone = self.clone(); // Clone Arc<Self>
            let tool_discovery_token = self.cancellation_token.child_token();
            let tool_discovery_handle = tokio::spawn(async move {
                if let Err(e) = self_clone.run_tool_discovery_loop(tool_discovery_token).await {
                     log::error!(target: "tool_discovery", "Tool discovery loop exited with error: {:?}", e);
                }
            });
            self.background_tasks.lock().expect("Mutex poisoned").push(tool_discovery_handle);
            println!("✅ Started remote tool discovery loop.");
        }


        // --- Start the A2A Server ---
        // Create the services needed by the server
        let task_service = Arc::new(crate::server::services::task_service::TaskService::bidirectional(
            self.task_repository.clone(), // Pass the repository
            #[cfg(feature = "bidir-local-exec")] self.task_router.clone(),
            #[cfg(feature = "bidir-local-exec")] self.tool_executor.clone(),
            #[cfg(feature = "bidir-delegate")] self.client_manager.clone(),
            #[cfg(feature = "bidir-delegate")] self.agent_registry.clone(),
            #[cfg(feature = "bidir-delegate")] self.config.self_id.clone(),
        ));
        let streaming_service = Arc::new(crate::server::services::streaming_service::StreamingService::new(self.task_repository.clone()));
        let notification_service = Arc::new(crate::server::services::notification_service::NotificationService::new(self.task_repository.clone()));

        // Start the server using the updated run_server function
        let server_token = self.cancellation_token.child_token(); // Create child token
        let server_handle = crate::server::run_server(
            port,
            bind_addr,
            task_service,
            streaming_service,
            notification_service,
            server_token, // Pass shutdown token
        ).await
         .map_err(|e| anyhow::anyhow!("Failed to start A2A server: {}", e))?; // Use map_err and anyhow!
        // Use std::sync::Mutex correctly
        *self.server_handle.lock().expect("Mutex poisoned") = Some(server_handle);
        println!("✅ A2A Server started.");


         // Keep the agent running until interrupted
        println!("✅ Bidirectional Agent '{}' running. Press Ctrl+C to stop.", self.config.self_id);

        // Wait for shutdown signal
        tokio::signal::ctrl_c().await?;

        // Initiate graceful shutdown
        self.shutdown().await?;

        Ok(())
    }

    /// Background task to periodically discover tools from other active agents.
    #[cfg(feature = "bidir-delegate")]
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
    #[cfg(feature = "bidir-delegate")]
    async fn discover_remote_tools(&self) -> Result<usize> {
        let active_agents: Vec<ActiveAgentEntry> = self.agent_directory.get_active_agents().await
            .context("Failed to get active agents for tool discovery")?;

        if active_agents.is_empty() {
            log::debug!(target: "tool_discovery", "No active agents found to discover tools from.");
            return Ok(0);
        }

        log::info!(target: "tool_discovery", "Discovering tools from {} active agents...", active_agents.len());
        let mut updated_count = 0;

        // Use futures::stream to process agents concurrently (optional, but good for many agents)
        use futures::stream::{self, StreamExt, TryStreamExt};
        let results = stream::iter(active_agents)
            .map(|agent_entry| async move {
                let agent_id = agent_entry.agent_id;
                let agent_url = agent_entry.url;
                log::debug!(target: "tool_discovery", "Querying tools for agent: {} at {}", agent_id, agent_url);

                // --- Placeholder: Actual Tool Discovery Logic ---
                // Example using ClientManager to get AgentCard (if AgentCard contains tool info):
                match self.client_manager.get_agent_card(&agent_id).await {
                    Ok(Some(card)) => {
                        // Assuming RemoteToolRegistry has a method to update from an AgentCard
                        match self.remote_tool_registry.update_tools_from_card(&agent_id, &card).await {
                            Ok(_) => {
                                log::debug!(target: "tool_discovery", "Successfully updated tools for agent: {}", agent_id);
                                Ok::<bool, anyhow::Error>(true) // Indicate update occurred
                            }
                            Err(e) => {
                                log::warn!(target: "tool_discovery", "Failed to update tools for agent {}: {:?}", agent_id, e);
                                Ok::<bool, anyhow::Error>(false) // No update, but not a fatal error for the whole process
                            }
                        }
                    }
                    Ok(None) => {
                        log::warn!(target: "tool_discovery", "Could not find agent card for active agent: {}", agent_id);
                        // Optionally: Remove tools for this agent if card is gone?
                        // self.remote_tool_registry.remove_tools_for_agent(agent_id).await?;
                        Ok::<bool, anyhow::Error>(false)
                    }
                    Err(e) => {
                        log::warn!(target: "tool_discovery", "Error fetching agent card for {}: {:?}", agent_id, e);
                        Ok::<bool, anyhow::Error>(false) // Error fetching card, treat as no update for this agent
                    }
                }
                // --- End Placeholder ---
            })
            .buffer_unordered(10) // Limit concurrency
            .try_collect::<Vec<bool>>() // Collect results, propagating fatal errors if any Ok(result) becomes Err
            .await;

        match results {
            Ok(update_flags) => {
                updated_count = update_flags.into_iter().filter(|&updated| updated).count();
                Ok(updated_count)
            }
            Err(e) => {
                // This would catch errors if the stream processing itself failed,
                // or if any of the async blocks returned an Err instead of Ok(bool).
                // Currently, the example logic returns Ok(false) on errors, so this
                // might not be hit unless the placeholder logic is changed to return Err.
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
}


/// Entry point function called from `main.rs` when the `bidirectional` command is used.
#[cfg(feature = "bidir-core")]
pub async fn run(config_path: &str) -> Result<()> {
    // Load configuration
    let config = config::load_config(config_path)
        .with_context(|| format!("Failed to load agent config from '{}'", config_path))?;

    // Initialize the agent
    let agent = BidirectionalAgent::new(config).await // Add .await here as new is async
        .context("Failed to initialize Bidirectional Agent")?;

    // Run the agent (run method now handles port binding from config)
    agent.run().await
        .context("Agent run failed")?; // Add context for the agent run result

    Ok(()) // Return Ok(()) on success
}


// Basic tests module (can be expanded later)
#[cfg(all(test, feature = "bidir-core"))]
mod tests {
    use super::*;
    use config::{AuthConfig, NetworkConfig}; // Import necessary config types

    #[test]
    fn test_agent_initialization_slice1() {
        let config = BidirectionalAgentConfig {
            self_id: "test-agent-slice1".to_string(),
            base_url: "http://localhost:8080".to_string(),
            discovery: vec![],
            auth: AuthConfig::default(),
            network: NetworkConfig::default(),
            // Tools config is only needed if bidir-local-exec is enabled
            #[cfg(feature = "bidir-local-exec")]
            tools: config::ToolConfigs::default(),
            #[cfg(feature = "bidir-core")]
            directory: config::DirectoryConfig::default(),
            #[cfg(feature = "bidir-delegate")]
            tool_discovery_interval_minutes: 30,
        };
        // BidirectionalAgent::new is async, use block_on for test
        let agent_result = tokio_test::block_on(BidirectionalAgent::new(config));
        assert!(agent_result.is_ok());
        let agent = agent_result.unwrap();
        // Check core components exist
        assert_eq!(agent.config.self_id, "test-agent-slice1");
        // agent_registry and client_manager are Arc, check they are initialized
        assert!(Arc::strong_count(&agent.agent_registry) >= 1);
        assert!(Arc::strong_count(&agent.client_manager) >= 1);
    }
}

// Include the new test utility and integration test modules when testing
#[cfg(all(test, feature = "bidir-core"))]
mod test_utils;
#[cfg(all(test, feature = "bidir-core"))]
mod integration_tests;
