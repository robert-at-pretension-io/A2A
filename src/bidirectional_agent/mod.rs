//! Bidirectional A2A Agent Implementation
//!
//! This module contains the core logic for an agent that can act as both an
//! A2A client and server, enabling task delegation and local execution.

// Only compile this module if the 'bidir-core' feature (or higher) is enabled.
#![cfg(feature = "bidir-core")]

use std::{sync::{Arc, Mutex}, net::SocketAddr}; // Add SocketAddr and Mutex
use tokio::runtime::Runtime;
use anyhow::{Result, Context}; // Add Context
use tokio::task::JoinHandle; // Add JoinHandle
use tokio_util::sync::CancellationToken; // Add CancellationToken

// Public submodules (conditionally compiled based on features)
pub mod config;
pub mod agent_registry;
pub mod client_manager;
pub mod error;
pub mod types;

#[cfg(feature = "bidir-local-exec")]
pub mod tool_executor; // Keep this
#[cfg(feature = "bidir-local-exec")]
pub mod task_router; // Keep this
#[cfg(feature = "bidir-local-exec")]
pub mod tools; // Keep this

#[cfg(feature = "bidir-delegate")]
pub mod task_flow; // Keep this
#[cfg(feature = "bidir-delegate")]
pub mod result_synthesis; // Keep this
#[cfg(feature = "bidir-delegate")]
pub mod task_extensions; // Keep this


// Re-export key types for easier access
pub use config::BidirectionalAgentConfig;
pub use agent_registry::AgentRegistry;
pub use client_manager::ClientManager;
pub use error::AgentError;
// Add imports for Slice 2 components
#[cfg(feature = "bidir-local-exec")]
pub use tool_executor::ToolExecutor;
#[cfg(feature = "bidir-local-exec")]
pub use task_router::TaskRouter;
// Add imports for Slice 3 components
#[cfg(feature = "bidir-delegate")]
pub use task_flow::TaskFlow;
#[cfg(feature = "bidir-delegate")]
pub use result_synthesis::ResultSynthesizer;
#[cfg(feature = "bidir-delegate")]
pub use task_extensions::TaskRepositoryExt;

// We've removed the GlobalRepository singleton since we're now passing
// the task repository directly to the client manager's poll loop


/// Main struct representing the Bidirectional Agent.
/// This will be expanded in later slices.
#[derive(Clone)]
pub struct BidirectionalAgent {
    pub config: Arc<BidirectionalAgentConfig>,
    pub agent_registry: Arc<AgentRegistry>,
    pub client_manager: Arc<ClientManager>,
    // Add components for Slice 2
    #[cfg(feature = "bidir-local-exec")]
    pub tool_executor: Arc<ToolExecutor>,
    #[cfg(feature = "bidir-local-exec")]
    pub task_router: Arc<TaskRouter>,
    // Add components for Slice 3
    // Note: TaskRepository needs to be Arc<dyn TaskRepositoryExt> potentially
    pub task_repository: Arc<crate::server::repositories::task_repository::InMemoryTaskRepository>, // Use concrete type for now
    // Add handles for server and background tasks
    #[cfg(feature="bidir-core")] // Only needed if core is enabled
    cancellation_token: CancellationToken,
    server_handle: Arc<Mutex<Option<JoinHandle<()>>>>, // Use Mutex for interior mutability
    background_tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl BidirectionalAgent {
    /// Creates a new BidirectionalAgent instance.
    pub fn new(config: BidirectionalAgentConfig) -> Result<Self> {
        let config_arc = Arc::new(config);
        let agent_registry = Arc::new(AgentRegistry::new());
        let task_repository = Arc::new(crate::server::repositories::task_repository::InMemoryTaskRepository::new());
        
        // Initialize Slice 2 components if feature is enabled
        #[cfg(feature = "bidir-local-exec")]
        let tool_executor = Arc::new(ToolExecutor::new());
        #[cfg(feature = "bidir-local-exec")]
        let task_router = Arc::new(TaskRouter::new(agent_registry.clone(), tool_executor.clone()));
        
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
            #[cfg(feature="bidir-core")]
            cancellation_token: CancellationToken::new(),
            server_handle: Arc::new(Mutex::new(None)),
            background_tasks: Arc::new(Mutex::new(Vec::new())),
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
        self.background_tasks.lock().await.push(registry_handle); // Store handle
        println!("✅ Started agent registry refresh loop.");

        #[cfg(feature = "bidir-delegate")]
        {
            let client_manager_clone = self.client_manager.clone();
            let poll_interval = chrono::Duration::seconds(30); // Example interval
            let poll_token = self.cancellation_token.child_token(); // Create child token
            let delegation_poll_handle = tokio::spawn(async move {
                 tokio::select! {
                    _ = poll_token.cancelled() => println!("Delegated task polling loop canceled."),
                    // Pass repository to poll loop (will be done in Milestone 2)
                    _ = client_manager_clone.run_delegated_task_poll_loop(poll_interval) => {}
                 }
            });
            self.background_tasks.lock().await.push(delegation_poll_handle); // Store handle
            println!("✅ Started delegated task polling loop.");
        }


        // --- Start the A2A Server ---
        // Create the services needed by the server
        let task_service = Arc::new(crate::server::services::TaskService::bidirectional(
            self.task_repository.clone(), // Pass the repository
            #[cfg(feature = "bidir-local-exec")] self.task_router.clone(),
            #[cfg(feature = "bidir-local-exec")] self.tool_executor.clone(),
            #[cfg(feature = "bidir-delegate")] self.client_manager.clone(),
            #[cfg(feature = "bidir-delegate")] self.agent_registry.clone(),
            #[cfg(feature = "bidir-delegate")] self.config.self_id.clone(),
        ));
        let streaming_service = Arc::new(crate::server::services::StreamingService::new(self.task_repository.clone()));
        let notification_service = Arc::new(crate::server::services::NotificationService::new(self.task_repository.clone()));

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
        *self.server_handle.lock().await = Some(server_handle); // Store handle
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
        if let Some(handle) = self.server_handle.lock().await.take() {
             println!("   Waiting for server task to finish...");
             if let Err(e) = handle.await {
                 eprintln!("   Server task join error: {:?}", e);
             } else {
                 println!("   Server task finished.");
             }
        }

        // Wait for all background tasks to complete
        let handles = std::mem::take(&mut *self.background_tasks.lock().await);
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
#[cfg(test)]
mod tests {
    use super::*;
    use config::AuthConfig; // Import AuthConfig

    #[test]
    fn test_agent_initialization() {
        let config = BidirectionalAgentConfig {
            self_id: "test-agent".to_string(),
            base_url: "http://localhost:8080".to_string(),
            discovery: vec![],
            auth: AuthConfig::default(), // Provide default AuthConfig
            network: config::NetworkConfig::default(), // Provide default NetworkConfig
        };
        let agent_result = BidirectionalAgent::new(config);
        assert!(agent_result.is_ok());
    }
}
