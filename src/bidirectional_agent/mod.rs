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
pub mod tool_executor;
#[cfg(feature = "bidir-local-exec")]
pub mod task_router;
#[cfg(feature = "bidir-local-exec")]
pub mod tools;

#[cfg(feature = "bidir-delegate")]
pub mod task_flow;
#[cfg(feature = "bidir-delegate")]
pub mod result_synthesis;
#[cfg(feature = "bidir-delegate")]
pub mod task_extensions;


// Re-export key types for easier access
pub use config::BidirectionalAgentConfig;
pub use agent_registry::AgentRegistry;
pub use client_manager::ClientManager;
pub use error::AgentError;

/// Main struct representing the Bidirectional Agent.
/// This will be expanded in later slices.
#[derive(Clone)]
pub struct BidirectionalAgent {
    pub config: Arc<BidirectionalAgentConfig>,
    pub agent_registry: Arc<AgentRegistry>,
    pub client_manager: Arc<ClientManager>,
    // Add other components like task_repository, router, executor later
}

impl BidirectionalAgent {
    /// Creates a new BidirectionalAgent instance.
    pub fn new(config: BidirectionalAgentConfig) -> Result<Self> {
        let config_arc = Arc::new(config);
        let agent_registry = Arc::new(AgentRegistry::new());
        let client_manager = Arc::new(ClientManager::new(agent_registry.clone(), config_arc.clone())?);

        Ok(Self {
            config: config_arc,
            agent_registry,
            client_manager,
            // Initialize other fields later
        })
    }

    /// Initializes and runs the agent's core services.
    /// This will be expanded to include the server and background tasks.
    pub async fn run(&self, _port: u16) -> Result<()> {
        println!("🚀 Bidirectional Agent '{}' starting...", self.config.self_id);

        // Initialize agent registry from bootstrap URLs
        println!("🔍 Discovering initial agents...");
        for url in &self.config.discovery {
            match self.agent_registry.discover(url).await {
                Ok(_) => println!("  ✅ Discovered agent at {}", url),
                Err(e) => println!("  ⚠️ Failed to discover agent at {}: {}", url, e),
            }
        }
        println!("Agent discovery complete.");

        // --- Placeholder for starting the server (Slice 2/3) ---
        println!("⏳ Server component not yet implemented (Slice 2/3).");

        // --- Placeholder for starting background tasks (Slice 3) ---
        println!("⏳ Background task polling not yet implemented (Slice 3).");

        // Keep the agent running (e.g., wait for a shutdown signal)
        // For now, just print a message and exit for Slice 1.
        println!("✅ Bidirectional Agent core initialized (Slice 1).");
        println!("🛑 Agent will exit now. Full server/background tasks in later slices.");

        // In a real scenario, this would loop or wait indefinitely.
        // tokio::signal::ctrl_c().await?;
        // println!("🛑 Shutting down Bidirectional Agent...");

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
