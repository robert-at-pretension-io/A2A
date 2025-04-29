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
// Add imports for Slice 2 components
#[cfg(feature = "bidir-local-exec")]
pub use tool_executor::ToolExecutor;
#[cfg(feature = "bidir-local-exec")]
pub use task_router::TaskRouter; // Keep standard router
#[cfg(feature = "bidir-local-exec")]
pub use task_router_llm::{LlmTaskRouter, LlmTaskRouterTrait, create_llm_task_router}; // Keep LLM router
#[cfg(feature = "bidir-local-exec")]
pub use llm_routing::{LlmRoutingConfig, RoutingAgent, SynthesisAgent};
// Add imports for Slice 3 components
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
    // Add components for Slice 2
    #[cfg(feature = "bidir-local-exec")]
    pub tool_executor: Arc<ToolExecutor>,
    #[cfg(feature = "bidir-local-exec")]
    pub task_router: Arc<dyn LlmTaskRouterTrait>, // Use the trait for polymorphism
    // Add components for Slice 3
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
    /// Creates a new BidirectionalAgent instance.
    pub fn new(config: BidirectionalAgentConfig) -> Result<Self> {
        let config_arc = Arc::new(config);
        let agent_registry = Arc::new(AgentRegistry::new());
        // Use the concrete InMemoryTaskRepository from the server module
        let task_repository = Arc::new(crate::server::repositories::task_repository::InMemoryTaskRepository::new());

        // Initialize Slice 2 components if feature is enabled
        #[cfg(feature = "bidir-local-exec")]
        let tool_executor = Arc::new(ToolExecutor::new()); // Initialize ToolExecutor

        // Initialize task router - potentially use LLM-based routing if configured
        #[cfg(feature = "bidir-local-exec")]
        let task_router: Arc<dyn LlmTaskRouterTrait> = {
            if config_arc.tools.use_llm_routing {
                 println!("🧠 Using LLM-powered task router");
                 // Configure LLM router with API key if available
                 let llm_config = config_arc.tools.llm_api_key.as_ref().map(|api_key| {
                     llm_routing::LlmRoutingConfig {
                         api_key: api_key.clone(),
                         model: config_arc.tools.llm_model.clone(),
                         ..Default::default()
                     }
                 });

                 // Create LLM task router using the factory
                 task_router_llm::LlmTaskRouterFactory::create_with_config(
                     agent_registry.clone(),
                     tool_executor.clone(),
                     llm_config.unwrap_or_default() // Use default config if no API key
                 )
            } else {
                 // Use the standard task router if LLM routing is not enabled
                 println!("📝 Using standard task router");
                 Arc::new(TaskRouter::new(agent_registry.clone(), tool_executor.clone())) as Arc<dyn LlmTaskRouterTrait>
            }
        };

        let client_manager = Arc::new(ClientManager::new(
            agent_registry.clone(),
            config_arc.clone()
        )?);

        Ok(Self {
            config: config_arc,
            agent_registry,
            client_manager,
            task_repository,
            // Initialize Slice 2 components if feature is enabled
            #[cfg(feature = "bidir-local-exec")]
            tool_executor,
            #[cfg(feature = "bidir-local-exec")]
            task_router,
            // Initialize cancellation token and handles
            cancellation_token: CancellationToken::new(),
            server_handle: Arc::new(StdMutex::new(None)),
            background_tasks: Arc::new(StdMutex::new(Vec::new())),
        })
    }

    /// Initializes and runs the agent's core services, including the server and background tasks.
    /// Binds to the address and port specified in the configuration.
    pub async fn run(&self) -> Result<()> {
        let bind_addr = &self.config.network.bind_address;
        let port = self.config.network.port.unwrap_or(8080); // Use config port or default
        println!("🚀 Bidirectional Agent '{}' starting...", self.config.self_id);
        println!("   Attempting to bind server to {}:{}", bind_addr, port);

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
        ).await.context("Failed to start A2A server")?;
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

    /// Initiates graceful shutdown of the agent.
    pub async fn shutdown(&self) -> Result<()> {
        println!("\n🛑 Shutting down Bidirectional Agent...");

        // Cancel all background tasks and the server
        self.cancellation_token.cancel();

        // Wait for the server task to complete
        // Use std::sync::Mutex correctly
        if let Some(handle) = self.server_handle.lock().expect("Mutex poisoned").take() {
             println!("   Waiting for server task to finish...");
             if let Err(e) = handle.await {
                 eprintln!("   Server task join error: {:?}", e);
             } else {
                 println!("   Server task finished.");
             }
        }

        // Wait for all background tasks to complete
        // Use std::sync::Mutex correctly
        let handles = std::mem::take(&mut *self.background_tasks.lock().expect("Mutex poisoned"));
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
    let agent = BidirectionalAgent::new(config)
        .context("Failed to initialize Bidirectional Agent")?;

    // Run the agent (run method now handles port binding from config)
    agent.run().await
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
        };
        let agent_result = BidirectionalAgent::new(config);
        assert!(agent_result.is_ok());
        let agent = agent_result.unwrap();
        // Check core components exist
        assert_eq!(agent.config.self_id, "test-agent-slice1");
        // agent_registry and client_manager are Arc, check they are initialized
        assert!(Arc::strong_count(&agent.agent_registry) >= 1);
        assert!(Arc::strong_count(&agent.client_manager) >= 1);
    }
}
