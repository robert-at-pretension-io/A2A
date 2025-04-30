/// Manages discovery and caching of known A2A agents.

#[cfg(feature = "bidir-core")]
use crate::client::A2aClient; // Use the existing client for discovery
#[cfg(feature = "bidir-core")]
use crate::types::AgentCard;
#[cfg(feature = "bidir-core")]
use dashmap::DashMap;
#[cfg(feature = "bidir-core")]
use std::sync::Arc;
#[cfg(feature = "bidir-core")]
use chrono::{DateTime, Utc, Duration};
#[cfg(feature = "bidir-core")]
use anyhow::{Result, Context};

#[cfg(feature = "bidir-core")]
/// Information cached about a known agent.
#[derive(Clone, Debug)]
pub struct CachedAgentInfo {
    pub card: AgentCard,
    pub last_checked: DateTime<Utc>,
    // Add reliability metrics later
}

#[cfg(feature = "bidir-core")]
/// Thread-safe registry for discovered A2A agents.
#[derive(Clone)]
pub struct AgentRegistry {
    /// Map from agent ID (or URL if ID not known yet) to cached info.
    pub agents: Arc<DashMap<String, CachedAgentInfo>>,
    /// HTTP client for discovery.
    // Consider making this Arc<reqwest::Client> if needed elsewhere or for easier cloning
    http_client: reqwest::Client,
    /// Reference to the persistent agent directory. Included if 'bidir-core' is enabled.
    #[cfg(feature = "bidir-core")]
    agent_directory: Arc<crate::bidirectional_agent::agent_directory::AgentDirectory>,
}

// Conditional compilation for AgentRegistry implementation based on features
#[cfg(feature = "bidir-core")]
impl AgentRegistry {

    #[cfg(test)]
    pub fn add_test_agent(&self, agent_id: &str, url: &str) {
        // Add the agent to the in-memory cache
        let cache_info = crate::bidirectional_agent::agent_registry::CachedAgentInfo {
            // url field removed, URL is inside card
            card: self::tests::create_mock_agent_card(agent_id, url), // Use the function from tests module
            last_checked: chrono::Utc::now(),
        };
        self.agents.insert(agent_id.to_string(), cache_info);
    }

    /// Creates a new agent registry. Requires AgentDirectory if 'bidir-core' is enabled.
    pub fn new(#[cfg(feature = "bidir-core")] agent_directory: Arc<crate::bidirectional_agent::agent_directory::AgentDirectory>) -> Self {
        // TODO: Consider pre-loading cache from directory here or lazily on first access.
        // For now, cache starts empty and populates on discovery/refresh.

        Self {
            agents: Arc::new(DashMap::new()),
            // TODO: Configure client (timeout, proxy) based on BidirectionalAgentConfig.network
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30)) // Example timeout
                .build()
                .expect("Failed to build HTTP client for registry"),
            // Store the directory if the feature is enabled
            #[cfg(feature = "bidir-core")]
            agent_directory,
        }
    }

    // If bidir-core is NOT enabled, provide a constructor that doesn't need AgentDirectory
    #[cfg(not(feature = "bidir-core"))]
    pub fn new() -> Self {
         Self {
            agents: Arc::new(DashMap::new()),
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client for registry"),
            // No agent_directory field when feature is off
        }
    }

    /// Discovers an agent by its base URL and adds/updates it in the registry.
    /// Uses the agent's `name` from the card as the primary key if available,
    /// otherwise uses the discovery URL.
    pub async fn discover(&self, url: &str) -> Result<()> {
        println!("  🔍 Attempting discovery for: {}", url);
        // Use a temporary A2aClient just for fetching the card
        let temp_client = A2aClient::new(url);

        let card_result = temp_client.get_agent_card().await;

        let card = match card_result {
            Ok(card) => card,
            Err(e) => {
                // Extract status code if available from the ClientError
                let status_code_info = match &e {
                    // Use the new ReqwestError variant and extract the stored status code
                    crate::client::errors::ClientError::ReqwestError { msg: _, status_code } => {
                        status_code.map(|s| format!(" (status code {})", s))
                    }
                    crate::client::errors::ClientError::A2aError(ae) => {
                        // A2aError might wrap HTTP errors indirectly, but let's focus on ReqwestError for now
                        // You could potentially add more logic here if A2aError needs specific handling
                        None
                    }
                    _ => None,
                };
                // Include status code in the context message if found
                return Err(e).context(format!(
                    "Failed to get agent card from {}{}",
                    url,
                    status_code_info.unwrap_or_default() // Append status code info or empty string
                ));
            }
        };

        // Use agent name as ID if possible
        let agent_id = card.name.clone();

        println!("  ℹ️ Discovered agent '{}' at {}", agent_id, url);

        let cache_info = CachedAgentInfo {
            card: card.clone(),
            last_checked: Utc::now(),
        };

        self.agents.insert(agent_id.clone(), cache_info);

        // Also add/update in the persistent directory if the feature is enabled
        #[cfg(feature = "bidir-core")]
        {
            self.agent_directory.add_agent(&agent_id, url, Some(card)).await
                .context("Failed to add agent to persistent directory")?;
        }

        Ok(()) // Discover returns Result<()> now
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
                card: new_card.clone(),
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
        // Convert chrono::Duration to std::time::Duration, handle error explicitly
        let std_interval = match interval.to_std() {
             Ok(d) => d,
             Err(e) => {
                 log::error!(target: "agent_registry", "Invalid refresh interval duration: {:?}. Error: {}. Using default 5 minutes.", interval, e);
                 // Use a default interval if conversion fails
                 std::time::Duration::from_secs(300)
             }
         };

        loop {
            tokio::time::sleep(std_interval).await;
            log::info!(target: "agent_registry", "Running periodic agent info refresh...");

            // Determine the list of agents to refresh
            #[cfg(feature = "bidir-core")]
            let agents_to_refresh_result = self.agent_directory.get_active_agents().await;
            #[cfg(not(feature = "bidir-core"))]
            let agents_to_refresh_result: Result<Vec<(String, String)>> = Ok(self.agents.iter().map(|e| (e.key().clone(), e.value().card.url.clone())).collect());


            match agents_to_refresh_result {
                Ok(agents_to_refresh) => {
                    log::debug!(target: "agent_registry", "Refreshing {} agent details", agents_to_refresh.len());
                    for (agent_id, _url) in agents_to_refresh {
                        // Use spawn to refresh concurrently? Maybe not, could overload network/rate limits.
                        // Refresh sequentially for now.
                        match self.refresh_agent_info(&agent_id).await {
                            Ok(updated) => {
                                if updated {
                                    log::info!(target: "agent_registry", "Refreshed agent card details for {}", agent_id);
                                } else {
                                    log::debug!(target: "agent_registry", "Agent card checked for {}, no changes", agent_id);
                                }
                            },
                            Err(e) => {
                                log::error!(target: "agent_registry", "Failed to refresh agent info for {}: {:?}", agent_id, e);
                            }
                        }
                }},
                Err(e) => {
                    #[cfg(feature = "bidir-core")]
                    log::error!(target: "agent_registry", "Failed to get active agents from directory for refresh loop: {:?}", e);
                    #[cfg(not(feature = "bidir-core"))]
                     log::error!(target: "agent_registry", "Failed to get agents from internal cache for refresh loop: {:?}", e);
                }
            }
            log::info!(target: "agent_registry", "Periodic agent info refresh cycle complete.");
        }
        // The loop is infinite, so this part is unreachable unless cancellation is added.
        // Consider adding CancellationToken handling here as well.
        // Ok(())
    }
}

#[cfg(all(test, feature = "bidir-core"))]
pub(crate) mod tests { // Make module public within crate for reuse in other tests
    use super::*;
    use crate::bidirectional_agent::{
        agent_directory::AgentDirectory,
        config::DirectoryConfig,
    };
    use mockito::Server;
    use crate::types::{AgentCapabilities, AgentSkill, AgentCard}; // Import AgentCard
    use tempfile::tempdir;
    use std::sync::Arc; // Import Arc

    // Make helper public within crate
    pub(crate) fn create_mock_agent_card(name: &str, url: &str) -> AgentCard {
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

    // Helper to create a test registry linked to a real temp AgentDirectory DB.
    // Returns the TempDir guard to keep the directory alive for the test duration.
    // Make helper public within crate
    pub(crate) async fn create_test_registry_with_real_dir() -> (AgentRegistry, Arc<AgentDirectory>, tempfile::TempDir) {
        let temp_dir = tempdir().expect("Failed to create temp directory for test DB");
        let db_path = temp_dir.path().join("test_registry_dir.db");
        let dir_config = DirectoryConfig {
            db_path: db_path.to_string_lossy().to_string(), // Use path from temp_dir
            // Use short timeouts/intervals for testing if needed, otherwise defaults are fine
            ..Default::default()
        };
        let directory = Arc::new(AgentDirectory::new(&dir_config).await.expect("Failed to create test AgentDirectory"));
        let registry = AgentRegistry::new(directory.clone());
        // Return the temp_dir guard along with registry and directory
        (registry, directory, temp_dir)
    }


    #[tokio::test]
    async fn test_discover_adds_to_registry_and_directory() {
        // Keep the temp_dir guard alive for the duration of the test
        let (registry, directory, _temp_dir_guard) = create_test_registry_with_real_dir().await;
        let mut server = Server::new_async().await;
        let agent_name = "test-agent-discover";
        let mock_card = create_mock_agent_card(agent_name, &server.url());

        let _m = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(serde_json::to_string(&mock_card).unwrap())
            .with_body(serde_json::to_string(&mock_card).unwrap())
            .create_async().await;

        // Discover the agent using the registry created by the helper
        let result = registry.discover(&server.url()).await;
        assert!(result.is_ok(), "Discovery failed: {:?}", result.err());

        // Assert registry cache
        let cached_info = registry.get(agent_name);
        assert!(cached_info.is_some(), "Agent not found in registry cache");
        assert_eq!(cached_info.unwrap().card.name, agent_name);

        // Assert directory persistence using the directory returned by the helper
        let dir_info = directory.get_agent_info(agent_name).await
            .expect("Agent not found in directory after discovery");
        assert_eq!(dir_info["url"], server.url());
        assert_eq!(dir_info["status"], "active"); // Should be added as active
    }

     #[tokio::test]
    async fn test_discover_http_failure() {
        // Keep the temp_dir guard alive
        let (registry, _directory, _temp_dir_guard) = create_test_registry_with_real_dir().await;
        let mut server = Server::new_async().await;
        // Mock server to return error
        let _m = server.mock("GET", "/.well-known/agent.json")
            .with_status(500) // Simulate server error
            .create_async().await;

        let result = registry.discover(&server.url()).await;
        assert!(result.is_err(), "Discovery should fail on HTTP error");
        // Check the error message includes context about fetching the card and the status code
        let err_string = result.unwrap_err().to_string();
        assert!(err_string.contains("Failed to get agent card from"), "Error message context mismatch: {}", err_string);
        assert!(err_string.contains("(status code 500)"), "Error message should mention status code 500: {}", err_string);
    }

    #[tokio::test]
    async fn test_refresh_agent_info_updates_cache() {
        // Keep the temp_dir guard alive
        let (registry, _directory, _temp_dir_guard) = create_test_registry_with_real_dir().await;
        let mut server = Server::new_async().await;
        let agent_name = "refresh-agent-cache";
        let initial_url = server.url();

        // Initial discovery mock (v1.0)
        let mut card_v1 = create_mock_agent_card(agent_name, &initial_url);
        card_v1.version = "1.0".to_string();
        let m_discover = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&card_v1).unwrap())
            .create_async().await;

        registry.discover(&initial_url).await.unwrap();
        m_discover.assert_async().await; // Ensure discovery mock was hit

        // Refresh mock (v2.0)
        let mut card_v2 = create_mock_agent_card(agent_name, &initial_url); // URL is the same
        card_v2.version = "2.0".to_string();
        let m_refresh = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&card_v2).unwrap())
            .create_async().await; // Create *new* mock for the same path

        // Act: Refresh the agent info
        let refresh_result = registry.refresh_agent_info(agent_name).await;
        assert!(refresh_result.is_ok(), "Refresh failed: {:?}", refresh_result.err());
        assert!(refresh_result.unwrap(), "Refresh should report changes (version updated)");
        m_refresh.assert_async().await; // Ensure refresh mock was hit

        // Assert: Check cache has the updated version
        let cached_info = registry.get(agent_name).expect("Agent not found in cache after refresh");
        assert_eq!(cached_info.card.version, "2.0");

        // Note: This test only verifies the registry's cache update.
        // The AgentDirectory update happens during the *initial* discover via add_agent.
        // Refreshing info in the registry doesn't automatically update the directory's stored card JSON.
        // A separate mechanism or policy would be needed if the directory's card needs frequent updates.
    }

    #[tokio::test]
    async fn test_refresh_agent_info_no_change() {
        // Keep the temp_dir guard alive
        let (registry, _directory, _temp_dir_guard) = create_test_registry_with_real_dir().await;
        let mut server = Server::new_async().await;
        let agent_name = "no-change-agent";
        let card_v1 = create_mock_agent_card(agent_name, &server.url());

        // Mock endpoint - expect it to be called twice (discover + refresh)
        let m = server.mock("GET", "/.well-known/agent.json")
            .with_status(200)
            .with_body(serde_json::to_string(&card_v1).unwrap())
            .expect(2) // IMPORTANT: Expect 2 calls
            .create_async().await;

        // Discover initially
        registry.discover(&server.url()).await.unwrap();

        // Refresh
        let refresh_result = registry.refresh_agent_info(agent_name).await;
        assert!(refresh_result.is_ok(), "Refresh failed: {:?}", refresh_result.err());
        assert!(!refresh_result.unwrap(), "Refresh should report no changes");

        // Verify mock was called twice
        m.assert_async().await;
    }
}
