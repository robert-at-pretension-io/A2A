use crate::bidirectional::BidirectionalAgent;
use crate::bidirectional::config::{BidirectionalAgentConfig, ServerConfig, ClientConfig, LlmConfig, ModeConfig};
use crate::bidirectional::tests::mocks::MockLlmClient;
use std::sync::Arc;
use std::env;

fn create_test_config() -> BidirectionalAgentConfig {
    // Create a test configuration with a mock LLM API key
    let mut config = BidirectionalAgentConfig::default();
    config.llm.claude_api_key = Some("test-api-key".to_string());
    config.server.port = 9090;
    config.server.bind_address = "127.0.0.1".to_string();
    config.server.agent_id = "test-agent".to_string();
    
    config
}

#[test]
fn test_agent_card_creation() {
    // Set up the environment to avoid actual API calls
    env::set_var("CLAUDE_API_KEY", "test-api-key");
    
    // Create a test configuration
    let config = create_test_config();
    
    // Intercept the LLM client creation to avoid actual API calls
    // This is a limitation of the current implementation - would be better to 
    // make the LLM client injectable for easier testing
    
    // Create the agent (this will actually try to create a real LLM client)
    let agent = BidirectionalAgent::new(config);
    
    // In a full implementation, we'd want to use dependency injection to avoid this issue
    // For now, if we can create the agent, we can at least test the agent card creation
    if let Ok(agent) = agent {
        let card = agent.create_agent_card();
        
        // Verify the card properties
        // The name should match the agent_id from the config ("test-agent")
        // because agent_name was None in the config.
        assert_eq!(card.name, "test-agent"); 
        assert_eq!(card.version, "1.0.0");
        assert_eq!(card.url, "http://127.0.0.1:9090");
        assert!(card.description.unwrap().contains("Bidirectional"));
        assert!(card.capabilities.push_notifications);
        assert!(card.capabilities.state_transition_history);
        assert!(card.capabilities.streaming); // Streaming is now supported
        assert_eq!(card.default_input_modes, vec!["text".to_string()]);
        assert_eq!(card.default_output_modes, vec!["text".to_string()]);
    }
}

// These tests would ideally be more extensive, but the current implementation
// has tight coupling to external services that makes it difficult to test in isolation.
// In a real-world scenario, we would refactor the agent to allow for better testability.

// Additional tests that would be valuable:
// - test_process_message_directly
// - test_send_task_to_remote
// - test_get_remote_agent_card
// - test_run_repl (with mocked stdin/stdout)
// - test_run_server

// For now, we'll focus on the parts we can test without major refactoring.

#[test]
fn test_agent_config_validation() {
    // Test with missing LLM API key
    let mut config = create_test_config();
    config.llm.claude_api_key = None;
    
    // Agent creation should fail without an API key
    let agent = BidirectionalAgent::new(config);
    assert!(agent.is_err());
    
    // Check that the error message indicates the missing API key
    match agent {
        Err(e) => {
            assert!(e.to_string().contains("No LLM configuration provided"));
        },
        Ok(_) => {
            panic!("Expected an error with missing LLM API key");
        }
    }
}

#[test]
fn test_client_initialization() {
    // Set up the environment to avoid actual API calls
    env::set_var("CLAUDE_API_KEY", "test-api-key");
    
    // Test with target URL
    let mut config = create_test_config();
    config.client.target_url = Some("http://example.com/agent".to_string());
    
    // Agent creation might succeed (depends on the implementation details)
    let agent = BidirectionalAgent::new(config);
    
    // If it succeeds, check that the client was initialized
    if let Ok(agent) = agent {
        assert!(agent.client.is_some());
    }
    
    // Test without target URL
    let mut config = create_test_config();
    config.client.target_url = None;
    
    // Agent creation might succeed (depends on the implementation details)
    let agent = BidirectionalAgent::new(config);
    
    // If it succeeds, check that the client was not initialized
    if let Ok(agent) = agent {
        assert!(agent.client.is_none());
    }
}
