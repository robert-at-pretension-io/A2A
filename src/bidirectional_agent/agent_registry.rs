//! Manages discovery and caching of known A2A agents.

#![cfg(feature = "bidir-core")]

use crate::client::A2aClient; // Use the existing client for discovery
use crate::types::AgentCard;
use dashmap::DashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc, Duration};
use anyhow::{Result, Context};

/// Information cached about a known agent.
#[derive(Clone, Debug)]
pub struct CachedAgentInfo {
    pub card: AgentCard,
    pub last_checked: DateTime<Utc>,
    // Add reliability metrics later
}

/// Thread-safe registry for discovered A2A agents.
#[derive(Clone)]
pub struct AgentRegistry {
    /// Map from agent ID (or URL if ID not known yet) to cached info.
    agents: Arc<DashMap<String, CachedAgentInfo>>,
    /// HTTP client for discovery.
    http_client: reqwest::Client,
}

impl AgentRegistry {
    /// Creates a new, empty agent registry.
    pub fn new() -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
            // Consider making client configurable later
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to build HTTP client for registry"),
        }
    }

    /// Discovers an agent by its base URL and adds/updates it in the registry.
    /// Uses the agent's `name` from the card as the primary key if available,
    /// otherwise uses the discovery URL.
    pub async fn discover(&self, url: &str) -> Result<()> {
        println!("  🔍 Attempting discovery for: {}", url);
        // Use a temporary A2aClient just for fetching the card
        // We don't need authentication for the .well-known endpoint typically
        let temp_client = A2aClient::new(url);

        let card = temp_client.get_agent_card().await
            .with_context(|| format!("Failed to get agent card from {}", url))?;

        // Use agent name as ID if possible, otherwise fall back to URL
        let agent_id = card.name.clone(); // Use name as the primary identifier

        println!("  ℹ️ Discovered agent '{}' at {}", agent_id, url);

        let cache_info = CachedAgentInfo {
            card,
            last_checked: Utc::now(),
        };

        self.agents.insert(agent_id, cache_info);
        Ok(())
    }

    /// Retrieves cached information for a specific agent by its ID (usually its name).
    pub fn get(&self, agent_id: &str) -> Option<dashmap::mapref::one::Ref<String, CachedAgentInfo>> {
        self.agents.get(agent_id)
    }

    /// Returns a snapshot of all currently known agents.
    pub fn all(&self) -> Vec<(String, CachedAgentInfo)> {
        self.agents.iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Refreshes the agent card information for a specific agent.
    /// Returns true if the card was updated, false otherwise.
    pub async fn refresh_agent_info(&self, agent_id: &str) -> Result<bool> {
        let current_info = match self.agents.get(agent_id) {
            Some(info) => info.clone(), // Clone to avoid holding lock during network call
            None => anyhow::bail!("Agent '{}' not found in registry for refresh.", agent_id),
        };

        // Use the URL from the cached card for refresh
        let url = &current_info.card.url;
        println!("  🔄 Refreshing agent info for '{}' from {}", agent_id, url);

        let temp_client = A2aClient::new(url);
        let new_card = temp_client.get_agent_card().await
            .with_context(|| format!("Failed to refresh agent card from {}", url))?;

        // Check if the card content has actually changed by comparing key fields
        if new_card.name == current_info.card.name && 
           new_card.url == current_info.card.url && 
           new_card.version == current_info.card.version {
            println!("  ✅ Agent card for '{}' is unchanged.", agent_id);
            // Update last_checked timestamp even if content is the same
            if let Some(mut entry) = self.agents.get_mut(agent_id) {
                entry.last_checked = Utc::now();
            }
            Ok(false) // No update needed
        } else {
            println!("  ✨ Agent card for '{}' updated.", agent_id);
            let new_cache_info = CachedAgentInfo {
                card: new_card,
                last_checked: Utc::now(),
            };
            self.agents.insert(agent_id.to_string(), new_cache_info);
            Ok(true) // Card was updated
        }
    }

    /// Periodically refreshes information for all known agents.
    /// This should be run in a background task.
    pub async fn run_refresh_loop(&self, interval: Duration) {
        println!("🕰️ Starting agent registry refresh loop (interval: {:?})", interval);
        // Convert chrono::Duration to std::time::Duration
        let std_duration = std::time::Duration::from_secs(interval.num_seconds() as u64);
        loop {
            tokio::time::sleep(std_duration).await;
            println!("🔄 Running periodic agent refresh...");
            let agent_ids: Vec<String> = self.agents.iter().map(|e| e.key().clone()).collect();
            for agent_id in agent_ids {
                match self.refresh_agent_info(&agent_id).await {
                    Ok(updated) => {
                        if updated {
                            println!("  ✅ Refreshed agent: {}", agent_id);
                        } else {
                             println!("  ℹ️ Agent checked, no changes: {}", agent_id);
                        }
                    },
                    Err(e) => {
                        println!("  ⚠️ Failed to refresh agent {}: {}", agent_id, e);
                        // Consider adding logic to mark agent as potentially stale or unreachable
                    }
                }
            }
             println!("🔄 Periodic agent refresh complete.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server; // Add mockito import
    use crate::types::{AgentCapabilities, AgentSkill}; // Import necessary types

    fn create_mock_agent_card(name: &str, url: &str) -> AgentCard {
         AgentCard {
            name: name.to_string(),
            description: Some(format!("Mock agent {}", name)),
            url: url.to_string(),
            provider: None,
            version: "1.0".to_string(),
            documentation_url: None,
            capabilities: AgentCapabilities { // Provide default capabilities
                streaming: true,
                push_notifications: true,
                state_transition_history: true,
            },
            authentication: None,
            default_input_modes: vec!["text/plain".to_string()],
            default_output_modes: vec!["text/plain".to_string()],
            skills: vec![AgentSkill{ // Provide a default skill
                id: "mock-skill".to_string(),
                name: "Mock Skill".to_string(),
                description: None, tags: None, examples: None, input_modes: None, output_modes: None
            }],
        }
    }

    #[tokio::test]
    async fn test_discover_and_get_agent() {
        let mut server = Server::new_async().await;
        let agent_name = "test-agent-1";
        let mock_card = create_mock_agent_card(agent_name, &server.url());

        let _m = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_card).unwrap())
            .create_async().await;

        let registry = AgentRegistry::new();
        let result = registry.discover(&server.url()).await;
        assert!(result.is_ok());

        let cached_info = registry.get(agent_name);
        assert!(cached_info.is_some());
        assert_eq!(cached_info.unwrap().card.name, agent_name);
    }

     #[tokio::test]
    async fn test_discover_failure() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/.well-known/agent.json")
            .with_status(404)
            .create_async().await;

        let registry = AgentRegistry::new();
        let result = registry.discover(&server.url()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to get agent card"));
    }

    #[tokio::test]
    async fn test_refresh_agent_info_updated() {
        let mut server = Server::new_async().await;
        let agent_name = "refresh-agent";
        let initial_url = server.url(); // URL for initial discovery
        let mock_card_v1 = create_mock_agent_card(agent_name, &initial_url);
        mock_card_v1.version = "1.0".to_string(); // Ensure version is set

        // Mock for initial discovery
        let m1 = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&mock_card_v1).unwrap())
            .create_async().await;

        let registry = AgentRegistry::new();
        registry.discover(&initial_url).await.unwrap();
        m1.assert_async().await; // Verify initial discovery happened

        // Prepare updated card for refresh
        let mut mock_card_v2 = mock_card_v1.clone();
        mock_card_v2.version = "2.0".to_string(); // Change version for update

        // Mock for refresh discovery (same endpoint, different response)
         let m2 = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&mock_card_v2).unwrap())
            .create_async().await; // Create a *new* mock for the same path

        // Act: Refresh the agent
        let refreshed = registry.refresh_agent_info(agent_name).await.unwrap();

        // Assert: Check if refresh reported an update and verify new version
        assert!(refreshed); // Should report true as card changed
        let updated_info = registry.get(agent_name).unwrap();
        assert_eq!(updated_info.card.version, "2.0");
        m2.assert_async().await; // Verify refresh mock was called
    }

     #[tokio::test]
    async fn test_refresh_agent_info_no_change() {
        let mut server = Server::new_async().await;
        let agent_name = "no-change-agent";
        let mock_card = create_mock_agent_card(agent_name, &server.url());

        // Mock for both discovery and refresh (same response)
        let m = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&mock_card).unwrap())
            .expect(2) // Expect it to be called twice (discovery + refresh)
            .create_async().await;

        let registry = AgentRegistry::new();
        registry.discover(&server.url()).await.unwrap();

        // Act: Refresh
        let refreshed = registry.refresh_agent_info(agent_name).await.unwrap();

        // Assert: Should report false (no change)
        assert!(!refreshed);
        m.assert_async().await; // Verify mock was called twice
    }
}
//! Manages discovery and caching of known A2A agents.

#![cfg(feature = "bidir-core")]

use crate::client::A2aClient; // Use the existing client for discovery
use crate::types::AgentCard;
use dashmap::DashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc, Duration};
use anyhow::{Result, Context};

/// Information cached about a known agent.
#[derive(Clone, Debug)]
pub struct CachedAgentInfo {
    pub card: AgentCard,
    pub last_checked: DateTime<Utc>,
    // Add reliability metrics later
}

/// Thread-safe registry for discovered A2A agents.
#[derive(Clone)]
pub struct AgentRegistry {
    /// Map from agent ID (or URL if ID not known yet) to cached info.
    pub(crate) agents: Arc<DashMap<String, CachedAgentInfo>>, // Make pub(crate) for tests
    /// HTTP client for discovery.
    http_client: reqwest::Client,
}

impl AgentRegistry {
    /// Creates a new, empty agent registry.
    pub fn new() -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
            // Consider making client configurable later
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("Failed to build HTTP client for registry"),
        }
    }

    /// Discovers an agent by its base URL and adds/updates it in the registry.
    /// Uses the agent's `name` from the card as the primary key if available,
    /// otherwise uses the discovery URL.
    pub async fn discover(&self, url: &str) -> Result<()> {
        println!("  🔍 Attempting discovery for: {}", url);
        // Use a temporary A2aClient just for fetching the card
        // We don't need authentication for the .well-known endpoint typically
        let temp_client = A2aClient::new(url);

        let card = temp_client.get_agent_card().await
            .with_context(|| format!("Failed to get agent card from {}", url))?;

        // Use agent name as ID if possible, otherwise fall back to URL
        let agent_id = card.name.clone(); // Use name as the primary identifier

        println!("  ℹ️ Discovered agent '{}' at {}", agent_id, url);

        let cache_info = CachedAgentInfo {
            card,
            last_checked: Utc::now(),
        };

        self.agents.insert(agent_id, cache_info);
        Ok(())
    }

    /// Retrieves cached information for a specific agent by its ID (usually its name).
    pub fn get(&self, agent_id: &str) -> Option<dashmap::mapref::one::Ref<String, CachedAgentInfo>> {
        self.agents.get(agent_id)
    }

    /// Returns a snapshot of all currently known agents.
    pub fn all(&self) -> Vec<(String, CachedAgentInfo)> {
        self.agents.iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Refreshes the agent card information for a specific agent.
    /// Returns true if the card was updated, false otherwise.
    pub async fn refresh_agent_info(&self, agent_id: &str) -> Result<bool> {
        let current_info = match self.agents.get(agent_id) {
            Some(info) => info.value().clone(), // Clone to avoid holding lock during network call
            None => anyhow::bail!("Agent '{}' not found in registry for refresh.", agent_id),
        };

        // Use the URL from the cached card for refresh
        let url = &current_info.card.url;
        println!("  🔄 Refreshing agent info for '{}' from {}", agent_id, url);

        let temp_client = A2aClient::new(url);
        let new_card = match temp_client.get_agent_card().await {
             Ok(card) => card,
             Err(e) => {
                 // Log the error but don't remove the agent immediately
                 println!("  ⚠️ Failed to refresh agent card for '{}' from {}: {}", agent_id, url, e);
                 // Optionally mark the agent as stale here
                 return Ok(false); // Indicate no successful update
             }
        };


        // Check if the card content has actually changed by comparing key fields
        // Use PartialEq derived for AgentCard if available, otherwise compare fields manually
        if new_card == current_info.card {
            println!("  ✅ Agent card for '{}' is unchanged.", agent_id);
            // Update last_checked timestamp even if content is the same
            if let Some(mut entry) = self.agents.get_mut(agent_id) {
                entry.last_checked = Utc::now();
            }
            Ok(false) // No update needed
        } else {
            println!("  ✨ Agent card for '{}' updated.", agent_id);
            let new_cache_info = CachedAgentInfo {
                card: new_card,
                last_checked: Utc::now(),
            };
            self.agents.insert(agent_id.to_string(), new_cache_info);
            Ok(true) // Card was updated
        }
    }

    /// Periodically refreshes information for all known agents.
    /// This should be run in a background task.
    pub async fn run_refresh_loop(&self, interval: chrono::Duration) {
        println!("🕰️ Starting agent registry refresh loop (interval: {:?})", interval);
        // Convert chrono::Duration to std::time::Duration
        let std_duration = match interval.to_std() {
            Ok(d) => d,
            Err(_) => {
                 eprintln!("Error converting refresh interval duration, defaulting to 5 minutes.");
                 std::time::Duration::from_secs(300)
            }
        };
        loop {
            tokio::time::sleep(std_duration).await;
            println!("🔄 Running periodic agent refresh...");
            let agent_ids: Vec<String> = self.agents.iter().map(|e| e.key().clone()).collect();
            for agent_id in agent_ids {
                match self.refresh_agent_info(&agent_id).await {
                    Ok(updated) => {
                        if updated {
                            println!("  ✅ Refreshed agent: {}", agent_id);
                        } else {
                             // Only log if verbose logging is enabled
                             // println!("  ℹ️ Agent checked, no changes: {}", agent_id);
                        }
                    },
                    Err(e) => {
                        println!("  ⚠️ Failed to refresh agent {}: {}", agent_id, e);
                        // Consider adding logic to mark agent as potentially stale or unreachable
                    }
                }
            }
             println!("🔄 Periodic agent refresh complete.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server; // Add mockito import
    use crate::types::{AgentCapabilities, AgentSkill}; // Import necessary types

    // Helper function to create a mock agent card
    fn create_mock_agent_card(name: &str, url: &str) -> AgentCard {
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
                description: None, tags: None, examples: None, input_modes: None, output_modes: None
            }],
        }
    }


    #[tokio::test]
    async fn test_discover_and_get_agent() {
        let mut server = Server::new_async().await;
        let agent_name = "test-agent-1";
        let mock_card = create_mock_agent_card(agent_name, &server.url());

        let _m = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_card).unwrap())
            .create_async().await;

        let registry = AgentRegistry::new();
        let result = registry.discover(&server.url()).await;
        assert!(result.is_ok());

        let cached_info = registry.get(agent_name);
        assert!(cached_info.is_some());
        assert_eq!(cached_info.unwrap().card.name, agent_name);
    }

     #[tokio::test]
    async fn test_discover_failure() {
        let mut server = Server::new_async().await;
        let _m = server.mock("GET", "/.well-known/agent.json")
            .with_status(404)
            .create_async().await;

        let registry = AgentRegistry::new();
        let result = registry.discover(&server.url()).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed to get agent card"));
    }

    #[tokio::test]
    async fn test_refresh_agent_info_updated() {
        let mut server = Server::new_async().await;
        let agent_name = "refresh-agent";
        let initial_url = server.url(); // URL for initial discovery
        let mut mock_card_v1 = create_mock_agent_card(agent_name, &initial_url);
        mock_card_v1.version = "1.0".to_string(); // Ensure version is set

        // Mock for initial discovery
        let m1 = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&mock_card_v1).unwrap())
            .create_async().await;

        let registry = AgentRegistry::new();
        registry.discover(&initial_url).await.unwrap();
        m1.assert_async().await; // Verify initial discovery happened

        // Prepare updated card for refresh
        let mut mock_card_v2 = mock_card_v1.clone();
        mock_card_v2.version = "2.0".to_string(); // Change version for update

        // Mock for refresh discovery (same endpoint, different response)
         let m2 = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&mock_card_v2).unwrap())
            .create_async().await; // Create a *new* mock for the same path

        // Act: Refresh the agent
        let refreshed = registry.refresh_agent_info(agent_name).await.unwrap();

        // Assert: Check if refresh reported an update and verify new version
        assert!(refreshed); // Should report true as card changed
        let updated_info = registry.get(agent_name).unwrap();
        assert_eq!(updated_info.card.version, "2.0");
        m2.assert_async().await; // Verify refresh mock was called
    }

     #[tokio::test]
    async fn test_refresh_agent_info_no_change() {
        let mut server = Server::new_async().await;
        let agent_name = "no-change-agent";
        let mock_card = create_mock_agent_card(agent_name, &server.url());

        // Mock for both discovery and refresh (same response)
        let m = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&mock_card).unwrap())
            .expect(2) // Expect it to be called twice (discovery + refresh)
            .create_async().await;

        let registry = AgentRegistry::new();
        registry.discover(&server.url()).await.unwrap();

        // Act: Refresh
        let refreshed = registry.refresh_agent_info(agent_name).await.unwrap();

        // Assert: Should report false (no change)
        assert!(!refreshed);
        m.assert_async().await; // Verify mock was called twice
    }
}
