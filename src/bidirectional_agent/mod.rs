//! Bidirectional A2A Agent Implementation
//!
//! This module contains the core logic for an agent that can act as both an
//! A2A client and server, enabling task delegation and local execution.

// Only compile this module if the 'bidir-core' feature (or higher) is enabled.
#![cfg(feature = "bidir-core")]

use std::sync::Arc;
use tokio::runtime::Runtime;
use anyhow::Result;

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
    // Add other components like server handle, background task handles later
}

impl BidirectionalAgent {
    /// Creates a new BidirectionalAgent instance.
    pub fn new(config: BidirectionalAgentConfig) -> Result<Self> {
        let config_arc = Arc::new(config);
        let agent_registry = Arc::new(AgentRegistry::new());
        let client_manager = Arc::new(ClientManager::new(agent_registry.clone(), config_arc.clone())?);

        // Initialize Slice 2 components if feature is enabled
        #[cfg(feature = "bidir-local-exec")]
        let tool_executor = Arc::new(ToolExecutor::new());
        #[cfg(feature = "bidir-local-exec")]
        let task_router = Arc::new(TaskRouter::new(agent_registry.clone(), tool_executor.clone()));

        // Initialize Task Repository (concrete type for now)
        let task_repository = Arc::new(crate::server::repositories::task_repository::InMemoryTaskRepository::new());


        Ok(Self {
            config: config_arc,
            agent_registry,
            client_manager,
            task_repository, // Add repository
            // Initialize Slice 2 fields
            #[cfg(feature = "bidir-local-exec")]
            tool_executor,
            #[cfg(feature = "bidir-local-exec")]
            task_router,
            // Initialize other fields later
        })
    }

    /// Initializes and runs the agent's core services, including the server and background tasks.
    pub async fn run(&self, port: u16) -> Result<()> {
        println!("🚀 Bidirectional Agent '{}' starting...", self.config.self_id);

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
        let _registry_refresh_handle = tokio::spawn(async move {
            registry_clone.run_refresh_loop(refresh_interval).await;
        });
        println!("✅ Started agent registry refresh loop.");

        #[cfg(feature = "bidir-delegate")]
        {
            let client_manager_clone = self.client_manager.clone();
            let poll_interval = chrono::Duration::seconds(30); // Example interval
             let _delegation_poll_handle = tokio::spawn(async move {
                client_manager_clone.run_delegated_task_poll_loop(poll_interval).await;
            });
            println!("✅ Started delegated task polling loop.");
        }


        // --- Start the A2A Server ---
        println!("🔌 Starting A2A server component on port {}...", port);
        // We need to pass the necessary components (TaskService, etc.) to the server runner.
        // This requires modifying src/server/mod.rs to accept these components.
        // For now, we'll use a placeholder call.
        // TODO: Update src/server/mod.rs run_server function signature
        let server_handle = tokio::spawn(async move {
             // Placeholder: Need to pass TaskService configured with router/executor
             if let Err(e) = crate::server::run_server(port).await {
                 eprintln!("❌ Server failed: {}", e);
             }
        });
        println!("✅ A2A Server started.");


        // Keep the agent running until interrupted
        println!("✅ Bidirectional Agent '{}' running. Press Ctrl+C to stop.", self.config.self_id);
        tokio::signal::ctrl_c().await?;
        println!("\n🛑 Shutting down Bidirectional Agent...");

        // TODO: Add graceful shutdown logic for server and background tasks
        server_handle.abort(); // Simple abort for now

        println!("🏁 Bidirectional Agent stopped.");
        Ok(())
    }
}

/// Entry point function called from `main.rs` when the `bidirectional` command is used.
pub async fn run(config_path: &str) -> Result<()> {
    // Load configuration
    let config = config::load_config(config_path)?;

    // Initialize and run the agent
    let agent = BidirectionalAgent::new(config)?;
    agent.run(0).await // Port is not used yet in Slice 1
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
