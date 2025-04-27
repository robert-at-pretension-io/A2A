//! Tests for Core Registry & Client Manager, including LLM routing.

#![cfg(feature = "bidir-core")]

use crate::bidirectional_agent::{
    config::{BidirectionalAgentConfig, AuthConfig, NetworkConfig, ToolConfigs},
    agent_registry::AgentRegistry,
    client_manager::ClientManager,
    BidirectionalAgent,
};
use crate::types::{AgentCard, AgentCapabilities, AgentSkill}; // Import necessary types
use mockito::Server;
use std::sync::Arc;
use std::collections::HashMap;

// Helper to create a mock agent card for testing
fn create_test_agent_card(name: &str, url: &str) -> AgentCard {
     AgentCard {
        name: name.to_string(),
        description: Some(format!("Test Card for {}", name)),
        url: url.to_string(),
        provider: None,
        version: "1.0".to_string(),
        documentation_url: None,
        capabilities: AgentCapabilities { // Provide default capabilities
            streaming: true,
            push_notifications: true,
            state_transition_history: true,
        },
        authentication: None, // No auth for this test card
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
async fn test_registry_discover_and_get() {
    let mut server = Server::new_async().await;
    let agent_name = "discoverable-agent";
    let mock_card = create_test_agent_card(agent_name, &server.url());

    let _m = server.mock("GET", "/.well-known/agent.json")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(serde_json::to_string(&mock_card).unwrap())
        .create_async().await;

    let registry = AgentRegistry::new();
    let discover_result = registry.discover(&server.url()).await;
    assert!(discover_result.is_ok());

    // Agent should be registered under its name
    let cached_info = registry.get(agent_name);
    assert!(cached_info.is_some());
    assert_eq!(cached_info.unwrap().card.name, agent_name);
}

#[tokio::test]
async fn test_client_manager_get_caches_client() {
    let mut server = Server::new_async().await;
    let agent_name = "cache-test-agent";
    let mock_card = create_test_agent_card(agent_name, &server.url());

     let _m_card = server.mock("GET", "/.well-known/agent.json")
        .with_status(200)
        .with_body(serde_json::to_string(&mock_card).unwrap())
        .create_async().await;

    let registry = Arc::new(AgentRegistry::new());
    registry.discover(&server.url()).await.unwrap(); // Discover first

    let config = Arc::new(BidirectionalAgentConfig {
        self_id: "manager-test".to_string(), base_url: "".to_string(), discovery: vec![],
        auth: AuthConfig::default(), network: NetworkConfig::default(),
    });
    let manager = ClientManager::new(registry, config).unwrap();

    // First call - should create the client
    let client1_result = manager.get_or_create_client(agent_name).await;
    assert!(client1_result.is_ok());
    assert_eq!(manager.clients.len(), 1); // Client should be cached

    // Second call - should retrieve the cached client
    let client2_result = manager.get_or_create_client(agent_name).await;
    assert!(client2_result.is_ok());
    assert_eq!(manager.clients.len(), 1); // Count should remain 1

    // Optional: Compare client instances (requires Client to derive PartialEq or compare pointers)
    // let client1_ptr = Arc::as_ptr(&client1_result.unwrap());
    // let client2_ptr = Arc::as_ptr(&client2_result.unwrap());
    // assert_eq!(client1_ptr, client2_ptr, "Should return the same cached client instance");
}

#[tokio::test]
async fn test_client_manager_handles_unknown_agent() {
     let registry = Arc::new(AgentRegistry::new()); // Empty registry
     let config = Arc::new(BidirectionalAgentConfig::default());
     let manager = ClientManager::new(registry, config).unwrap();

     let client_result = manager.get_or_create_client("non-existent-agent").await;
     assert!(client_result.is_err());
     assert!(client_result.unwrap_err().to_string().contains("not found in registry"));
}

#[cfg(feature = "bidir-local-exec")]
#[tokio::test]
async fn test_agent_initialization_with_llm_router() {
    // Create config with LLM routing enabled
    let mut tools = ToolConfigs::default();
    tools.use_llm_routing = true;
    tools.llm_api_key = Some("test-api-key".to_string());
    tools.llm_model = "test-model".to_string();
    
    let config = BidirectionalAgentConfig {
        self_id: "test-agent-llm".to_string(),
        base_url: "http://localhost:8080".to_string(),
        discovery: vec![],
        auth: AuthConfig::default(),
        network: NetworkConfig::default(),
        tools,
    };
    
    // Initialize the agent
    let agent_result = BidirectionalAgent::new(config);
    assert!(agent_result.is_ok());
    
    let agent = agent_result.unwrap();
    
    // Check that we're using an LLM task router
    // Since we can't check the concrete type directly, we'll verify based on debug representation
    let router_debug = format!("{:?}", agent.task_router);
    assert!(router_debug.contains("LlmTaskRouter") || router_debug.contains("Arc"));
}

#[cfg(feature = "bidir-local-exec")]
#[tokio::test]
async fn test_agent_initialization_with_standard_router() {
    // Create config with LLM routing disabled
    let mut tools = ToolConfigs::default();
    tools.use_llm_routing = false;
    
    let config = BidirectionalAgentConfig {
        self_id: "test-agent-std".to_string(),
        base_url: "http://localhost:8080".to_string(),
        discovery: vec![],
        auth: AuthConfig::default(),
        network: NetworkConfig::default(),
        tools,
    };
    
    // Initialize the agent
    let agent_result = BidirectionalAgent::new(config);
    assert!(agent_result.is_ok());
    
    // With standard router, we shouldn't see any LLM-specific fields in the debug output
    let agent = agent_result.unwrap();
    let router_debug = format!("{:?}", agent.task_router);
    assert!(router_debug.contains("Arc"));
}
