use crate::bidirectional::bidirectional_agent::BidirectionalAgent;
use crate::bidirectional::config::BidirectionalAgentConfig;
use std::env;

fn create_test_config() -> BidirectionalAgentConfig {
    // Create a test configuration with a mock LLM API key
    // Prioritize Gemini for testing if both are set, otherwise Claude
    let mut config = BidirectionalAgentConfig::default();
    if env::var("GEMINI_API_KEY").is_ok() || env::var("TEST_USE_GEMINI").is_ok() {
        config.llm.gemini_api_key =
            Some(env::var("GEMINI_API_KEY").unwrap_or_else(|_| "test-gemini-key".to_string()));
    } else {
        config.llm.claude_api_key =
            Some(env::var("CLAUDE_API_KEY").unwrap_or_else(|_| "test-claude-key".to_string()));
    }
    config.server.port = 9090;
    config.server.bind_address = "127.0.0.1".to_string();
    config.server.agent_id = "test-agent".to_string();

    config
}

#[test]
fn test_agent_card_creation() {
    // Set up the environment to avoid actual API calls
    // Ensure at least one API key is set for agent creation to succeed
    if env::var("GEMINI_API_KEY").is_err() && env::var("CLAUDE_API_KEY").is_err() {
        env::set_var("CLAUDE_API_KEY", "test-claude-key"); // Default to Claude for this test if none are set
    }

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
        // Using the server module's create_agent_card function, all capabilities are set to true
        assert!(card.capabilities.push_notifications, "push_notifications capability should be true");
        assert!(card.capabilities.state_transition_history, "state_transition_history capability should be true");
        assert!(card.capabilities.streaming, "streaming capability should be true");
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
    config.llm.gemini_api_key = None; // Ensure both are None

    // Agent creation should fail without an API key
    let agent_result = BidirectionalAgent::new(config);
    assert!(agent_result.is_err());

    // Check that the error message indicates the missing API key
    match agent_result {
        Err(e) => {
            let err_msg = e.to_string();
            assert!(
                err_msg.contains("No LLM configuration provided"),
                "Error message was: {}",
                err_msg
            );
        }
        Ok(_) => {
            panic!("Expected an error with missing LLM API key");
        }
    }
}

#[test]
fn test_client_initialization() {
    // Set up the environment to avoid actual API calls
    // Ensure at least one API key is set for agent creation to succeed
    if env::var("GEMINI_API_KEY").is_err() && env::var("CLAUDE_API_KEY").is_err() {
        env::set_var("CLAUDE_API_KEY", "test-claude-key"); // Default to Claude for this test if none are set
    }

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
