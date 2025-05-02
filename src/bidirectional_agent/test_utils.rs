//! Utility functions for bidirectional agent integration tests.

// Only compile when testing and bidir-core feature is enabled
#![cfg(all(test, feature = "bidir-core"))]

use crate::{
    bidirectional_agent::{
        agent_directory::AgentDirectory,
        agent_registry::{AgentRegistry, CachedAgentInfo}, // Import CachedAgentInfo if needed directly
        config::{DirectoryConfig, BidirectionalAgentConfig}, // Removed AgentRegistryConfig which doesn't exist
    },
    types::{AgentCard, AgentCapabilities, AgentSkill},
};
use std::sync::Arc;
use tempfile::{tempdir, TempDir}; // Keep TempDir import

/// Creates a mock AgentCard for testing purposes.
pub fn create_mock_agent_card(name: &str, url: &str) -> AgentCard {
    AgentCard {
        name: name.to_string(),
        description: Some(format!("Mock agent {}", name)),
        url: url.to_string(),
        provider: None,
        version: "1.0".to_string(),
        documentation_url: None,
        capabilities: AgentCapabilities {
            streaming: true,
            push_notifications: true,
            state_transition_history: true,
        },
        authentication: None,
        default_input_modes: vec!["text/plain".to_string()],
        default_output_modes: vec!["text/plain".to_string()],
        skills: vec![AgentSkill {
            id: "mock-skill".to_string(),
            name: "Mock Skill".to_string(),
            description: None,
            tags: None,
            examples: None,
            input_modes: None,
            output_modes: None,
        }],
    }
}

/// Sets up a test environment with AgentDirectory and AgentRegistry using a temporary DB.
///
/// Returns the registry, directory, and the TempDir guard to keep the DB alive.
pub async fn setup_test_environment(
    dir_config_override: Option<DirectoryConfig>,
    // Add registry config override if needed
) -> (AgentRegistry, Arc<AgentDirectory>, TempDir) {
    let temp_dir = tempdir().expect("Failed to create temp directory for test DB");
    let db_path = temp_dir.path().join("test_agent_dir.db");

    let dir_config = dir_config_override.unwrap_or_else(|| DirectoryConfig {
        db_path: db_path.to_string_lossy().to_string(),
        // Use short timeouts/intervals for testing by default
        verification_interval_minutes: 1, // Check frequently in tests
        request_timeout_seconds: 5,
        max_failures_before_inactive: 2, // Make it easy to trigger inactive state
        backoff_seconds: 1, // Start backoff quickly
        health_endpoint_path: "/health".to_string(), // Example health endpoint
        ..Default::default() // Use defaults for other fields if any added
    });

    let directory = Arc::new(
        AgentDirectory::new(&dir_config)
            .await
            .expect("Failed to create test AgentDirectory"),
    );

    // Assuming AgentRegistry::new takes Arc<AgentDirectory>
    // If AgentRegistry needs its own config, handle that here too
    let registry = AgentRegistry::new(directory.clone());

    (registry, directory, temp_dir)
}

/// Helper to directly query the agent status from the test database.
pub async fn get_agent_status_from_db(
    directory: &AgentDirectory,
    agent_id: &str,
) -> Option<String> {
    // Access the internal db_pool (assuming it's accessible or add a helper method to AgentDirectory)
    // For now, let's assume we might need to add a helper if db_pool isn't pub
    // Alternative: Use directory.get_agent_info and parse the status field.
    match directory.get_agent_info(agent_id).await {
        Ok(info) => info.get("status").and_then(|s| s.as_str()).map(String::from),
        Err(_) => None,
    }
}

/// Helper to directly query the consecutive failures from the test database.
pub async fn get_agent_failures_from_db(
    directory: &AgentDirectory,
    agent_id: &str,
) -> Option<i64> {
     match directory.get_agent_info(agent_id).await {
        Ok(info) => info.get("consecutive_failures").and_then(|f| f.as_i64()),
        Err(_) => None,
    }
}
