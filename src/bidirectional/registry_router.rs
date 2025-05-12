//! Registry-focused task router for bidirectional agents
//!
//! This router specifically handles agent registration based on URLs in messages
//! and returns the list of known agents when a non-URL message is sent.

use crate::bidirectional::agent_registry::{AgentDirectory, AgentDirectoryEntry};
use crate::server::error::ServerError;
use crate::server::task_router::{LlmTaskRouterTrait, RoutingDecision, SubtaskDefinition};
use crate::types::{Message, Part, TaskSendParams, TaskState};

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error, info, instrument, trace, warn};

/// Router that manages agent registrations and responses
#[derive(Clone)]
pub struct RegistryRouter {
    /// Registry for storing agent information
    registry: Arc<AgentDirectory>,
}

impl RegistryRouter {
    /// Create a new registry router
    pub fn new(registry: Arc<AgentDirectory>) -> Self {
        Self { registry }
    }

    /// Extract text from a message
    fn extract_text_from_message(message: &Message) -> String {
        message
            .parts
            .iter()
            .filter_map(|part| match part {
                Part::TextPart(tp) => Some(tp.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Process a message containing a potential agent URL
    #[instrument(skip(self, message_text))]
    async fn process_url_message(&self, message_text: &str) -> Result<Option<String>, ServerError> {
        debug!(message = %message_text, "Processing potential URL in message");
        
        // Extract potential agent URLs from the message
        let urls = AgentDirectory::extract_agent_urls(message_text);
        if urls.is_empty() {
            debug!("No URLs found in message");
            return Ok(None);
        }

        // Add all found URLs to the registry and fetch agent cards
        let mut registered_urls = Vec::new();
        let mut retrieved_cards = Vec::new();
        
        for url in urls {
            // Add URL to registry initially
            match self.registry.add_agent(&url, None) {
                Ok(()) => {
                    debug!(url = %url, "Added agent URL to registry");
                    registered_urls.push(url.clone());
                    
                    // Try to fetch agent card for the URL
                    match self.fetch_agent_card(&url).await {
                        Ok(Some(card)) => {
                            debug!(url = %url, agent_name = %card.name, "Retrieved agent card");
                            retrieved_cards.push((url.clone(), card.name.clone()));
                        }
                        Ok(None) => {
                            debug!(url = %url, "No agent card retrieved");
                        }
                        Err(e) => {
                            warn!(url = %url, error = %e, "Failed to retrieve agent card");
                        }
                    }
                }
                Err(e) => {
                    warn!(url = %url, error = %e, "Failed to add agent URL to registry");
                }
            }
        }

        if registered_urls.is_empty() {
            return Ok(None);
        }

        // Create a response message
        let mut response = if registered_urls.len() == 1 {
            format!("✅ Registered agent URL: {}", registered_urls[0])
        } else {
            format!(
                "✅ Registered {} agent URLs: {}",
                registered_urls.len(),
                registered_urls.join(", ")
            )
        };
        
        // Add information about retrieved cards
        if !retrieved_cards.is_empty() {
            response.push_str("\n\n🔍 Retrieved agent information:");
            for (url, name) in retrieved_cards {
                response.push_str(&format!("\n- {}: {}", name, url));
            }
        }

        Ok(Some(response))
    }
    
    /// Fetch an agent card from a URL
    async fn fetch_agent_card(&self, url: &str) -> Result<Option<crate::types::AgentCard>, ServerError> {
        debug!(url = %url, "Attempting to fetch agent card");
        
        // Create a temporary client to fetch the agent card
        let client = crate::client::A2aClient::new(url);
        
        match client.get_agent_card().await {
            Ok(card) => {
                // Store the agent card in the registry
                if let Err(e) = self.registry.update_agent_card(url, card.clone()) {
                    warn!(url = %url, error = %e, "Failed to update agent card in registry");
                }
                Ok(Some(card))
            }
            Err(e) => {
                debug!(url = %url, error = %e, "Failed to fetch agent card");
                Ok(None)
            }
        }
    }

    /// Get a formatted list of all known agents
    fn get_agent_list(&self) -> String {
        debug!("Getting list of all known agents");
        let agents = self.registry.get_all_agents();
        
        if agents.is_empty() {
            return "No agents registered yet. Send an agent URL to register one.".to_string();
        }

        // Format the list
        let mut response = format!("📋 Known Agents ({})\n\n", agents.len());
        
        // First list agents with full card information
        let agents_with_cards: Vec<_> = agents.iter().filter(|a| a.agent_card.is_some()).collect();
        let agents_without_cards: Vec<_> = agents.iter().filter(|a| a.agent_card.is_none()).collect();
        
        if !agents_with_cards.is_empty() {
            response.push_str("🔍 Agents with full information:\n");
            
            for (i, agent) in agents_with_cards.iter().enumerate() {
                let card = agent.agent_card.as_ref().unwrap();
                let last_contact = agent.last_contacted.as_deref().unwrap_or("unknown");
                
                response.push_str(&format!("{}. {} ({})\n", i + 1, card.name, agent.url));
                
                // Add more details from the card
                if let Some(desc) = &card.description {
                    response.push_str(&format!("   Description: {}\n", desc));
                }
                
                // Add capabilities
                let mut caps = Vec::new();
                if card.capabilities.streaming {
                    caps.push("streaming");
                }
                if card.capabilities.push_notifications {
                    caps.push("push_notifications");
                }
                if card.capabilities.state_transition_history {
                    caps.push("state_history");
                }
                
                if !caps.is_empty() {
                    response.push_str(&format!("   Capabilities: {}\n", caps.join(", ")));
                }
                
                // Add skills count if any
                if !card.skills.is_empty() {
                    response.push_str(&format!("   Skills: {}\n", card.skills.len()));
                }
                
                response.push_str(&format!("   Last contacted: {}\n", last_contact));
                response.push_str("\n");
            }
        }
        
        // Then list agents with only URL info
        if !agents_without_cards.is_empty() {
            if !agents_with_cards.is_empty() {
                response.push_str("🔗 Agents with basic information:\n");
            }
            
            for (i, agent) in agents_without_cards.iter().enumerate() {
                let agent_name = agent.name.as_deref().unwrap_or("(unnamed)");
                let last_contact = agent.last_contacted.as_deref().unwrap_or("unknown");
                
                response.push_str(&format!("{}. {} - {}\n", 
                    i + 1 + agents_with_cards.len(), 
                    agent_name, 
                    agent.url
                ));
                response.push_str(&format!("   Last contacted: {}\n", last_contact));
            }
        }

        response
    }
}

#[async_trait]
impl LlmTaskRouterTrait for RegistryRouter {
    #[instrument(skip_all, fields(task_id = %params.id))]
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError> {
        debug!("Registry router deciding how to handle task");
        
        // Extract message text
        let message_text = Self::extract_text_from_message(&params.message);
        
        // Check if the message contains any potential agent URLs
        if AgentDirectory::contains_agent_url(&message_text) {
            debug!("Message contains potential agent URL(s)");
            
            // Process the URL(s) in the message
            if let Some(response) = self.process_url_message(&message_text).await? {
                debug!("Processed URL message, returning response");
                // Return a completed task with the response
                return Ok(RoutingDecision::Local {
                    tool_name: "echo".to_string(),
                    params: json!({
                        "text": response
                    }),
                });
            }
        }
        
        // If no URLs or URL processing failed, return the list of known agents
        debug!("No URLs found or processing failed, returning agent list");
        let agent_list = self.get_agent_list();
        
        Ok(RoutingDecision::Local {
            tool_name: "echo".to_string(),
            params: json!({
                "text": agent_list
            }),
        })
    }

    #[instrument(skip_all, fields(task_id = %task_id))]
    async fn process_follow_up(
        &self,
        task_id: &str,
        message: &Message,
    ) -> Result<RoutingDecision, ServerError> {
        debug!(task_id = %task_id, "Processing follow-up message in registry router");
        
        // For follow-ups, process the same way as the initial message
        self.decide(&TaskSendParams {
            id: task_id.to_string(),
            message: message.clone(),
            session_id: None,
            metadata: None,
            history_length: None,
            push_notification: None,
        })
        .await
    }

    #[instrument(skip_all, fields(task_id = %params.id))]
    async fn route_task(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError> {
        debug!(task_id = %params.id, "Routing task in registry router");
        // Delegate to decide
        self.decide(params).await
    }

    #[instrument(skip_all, fields(task_id = %params.id))]
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, ServerError> {
        debug!(task_id = %params.id, "Checking if task should be decomposed");
        // Registry router doesn't decompose tasks
        Ok(false)
    }

    #[instrument(skip_all, fields(task_id = %_params.id))]
    async fn decompose_task(
        &self,
        _params: &TaskSendParams,
    ) -> Result<Vec<SubtaskDefinition>, ServerError> {
        // Registry router doesn't decompose tasks
        Ok(Vec::new())
    }
}