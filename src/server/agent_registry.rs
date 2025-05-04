/// Manages discovery and caching of known A2A agents.

use crate::client::A2aClient;
use crate::types::AgentCard;
use crate::server::error::ServerError;

use dashmap::DashMap;
use std::sync::Arc;
use chrono::{DateTime, Utc};

/// Information cached about a known agent.
#[derive(Clone, Debug)]
pub struct CachedAgentInfo {
    pub card: AgentCard,
    pub last_checked: DateTime<Utc>,
}

/// Thread-safe registry for discovered A2A agents.
#[derive(Clone)]
pub struct AgentRegistry {
    /// Map from agent ID (or URL if ID not known yet) to cached info.
    pub agents: Arc<DashMap<String, CachedAgentInfo>>,
}

impl AgentRegistry {
    /// Creates a new agent registry
    pub fn new() -> Self {
        Self {
            agents: Arc::new(DashMap::new()),
        }
    }

    /// Discovers an agent by its base URL and adds/updates it in the registry.
    /// Returns the agent's name/ID on success for tools to use.
    #[instrument(skip(self), fields(%url))]
    pub async fn discover(&self, url: &str) -> Result<String, ServerError> {
        debug!("Attempting to discover agent via URL.");
        // Use a temporary A2aClient just for fetching the card
        let temp_client = A2aClient::new(url);

        match temp_client.get_agent_card().await {
            Ok(card) => {
                // Use agent name as ID
                let agent_id = card.name.clone();

                let cache_info = CachedAgentInfo {
                    card: card.clone(),
                    last_checked: Utc::now(),
                };

                self.agents.insert(agent_id.clone(), cache_info);
                info!(%agent_id, %url, "Successfully discovered and registered/updated agent."); // Keep info for success
                Ok(agent_id) // Return the agent's name/ID
            },
            Err(e) => {
                Err(ServerError::A2aClientError(format!("Failed to get agent card from {}: {}", url, e)))
            }
        }
    }

    /// Registers a known agent directly
    pub fn register_agent(&self, agent_id: &str, url: &str, name: &str) {
        // Create a basic card 
        let card = AgentCard {
            name: name.to_string(),
            description: Some(format!("Agent '{}'", name)),
            url: url.to_string(),
            provider: None,
            version: "1.0".to_string(), 
            documentation_url: None,
            capabilities: crate::types::AgentCapabilities {
                streaming: true,
                push_notifications: true,
                state_transition_history: true,
            },
            authentication: None,
            default_input_modes: vec!["text".to_string()],
            default_output_modes: vec!["text".to_string()],
            skills: Vec::new(),
        };

        // Cache the agent info
        let cache_info = CachedAgentInfo {
            card,
            last_checked: Utc::now(),
        };
        
        self.agents.insert(agent_id.to_string(), cache_info);
    }

    /// Retrieves cached information for a specific agent by its ID (usually its name).
    pub fn get(&self, agent_id: &str) -> Option<CachedAgentInfo> {
        self.agents.get(agent_id).map(|entry| entry.value().clone())
    }

    /// Gets the URL for an agent by ID
    pub async fn get_agent_url(&self, agent_id: &str) -> Result<String, ServerError> {
        match self.get(agent_id) {
            Some(info) => Ok(info.card.url),
            None => Err(ServerError::AgentNotFound(format!("Agent '{}' not found in registry", agent_id)))
        }
    }

    /// Returns a snapshot of all currently known agents.
    pub fn all(&self) -> Vec<(String, CachedAgentInfo)> {
        self.agents.iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Returns a list of all agent IDs and their corresponding AgentCards.
    pub fn list_all_agents(&self) -> Vec<(String, AgentCard)> {
        self.agents
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().card.clone()))
            .collect()
    }
}
