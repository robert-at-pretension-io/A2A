// Update the use path to point to the new config module
use crate::bidirectional::config::{
    BidirectionalAgentConfig, ServerConfig, ClientConfig, LlmConfig, ModeConfig
};
use std::fs;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_default_config() {
    // Test default configuration
    let config = BidirectionalAgentConfig::default();
    
    // Check default server settings
    assert_eq!(config.server.port, 8080);
    assert_eq!(config.server.bind_address, "0.0.0.0");
    assert!(config.server.agent_id.starts_with("bidirectional-"));
    
    // Check default client settings
    assert_eq!(config.client.target_url, None);
    
    // Check default LLM settings
    assert_eq!(config.llm.claude_api_key, None);
    assert!(config.llm.system_prompt.contains("You are an AI agent assistant"));
    
    // Check default mode settings
    assert_eq!(config.mode.repl, false);
    assert_eq!(config.mode.message, None);
    assert_eq!(config.mode.get_agent_card, false);
    assert_eq!(config.mode.remote_task, None);
}

#[test]
fn test_load_config_from_valid_toml() {
    // Create a temporary directory
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("test_config.toml");
    
    // Create a test TOML file
    let config_content = r#"
        [server]
        port = 9090
        bind_address = "127.0.0.1"
        agent_id = "test-agent"
        
        [client]
        target_url = "http://example.com/agent"
        
        [llm]
        claude_api_key = "test-api-key"
        system_prompt = "Test system prompt"
        
        [mode]
        repl = true
        message = "Test message"
        get_agent_card = true
        remote_task = "Remote task"
    "#;
    
    fs::write(&config_path, config_content).expect("Failed to write test config");
    
    // Load the config
    let config = BidirectionalAgentConfig::from_file(&config_path).expect("Failed to load config");
    
    // Check server settings
    assert_eq!(config.server.port, 9090);
    assert_eq!(config.server.bind_address, "127.0.0.1");
    assert_eq!(config.server.agent_id, "test-agent");
    
    // Check client settings
    assert_eq!(config.client.target_url, Some("http://example.com/agent".to_string()));
    
    // Check LLM settings
    assert_eq!(config.llm.claude_api_key, Some("test-api-key".to_string()));
    assert_eq!(config.llm.system_prompt, "Test system prompt");
    
    // Check mode settings
    assert_eq!(config.mode.repl, true);
    assert_eq!(config.mode.message, Some("Test message".to_string()));
    assert_eq!(config.mode.get_agent_card, true);
    assert_eq!(config.mode.remote_task, Some("Remote task".to_string()));
}

#[test]
fn test_load_config_with_partial_values() {
    // Create a temporary directory
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("partial_config.toml");
    
    // Create a test TOML file with only some values
    let config_content = r#"
        [server]
        port = 9090
        
        [mode]
        repl = true
        get_agent_card = false
    "#;
    
    fs::write(&config_path, config_content).expect("Failed to write test config");
    
    // Load the config
    let config = BidirectionalAgentConfig::from_file(&config_path).expect("Failed to load config");
    
    // Check that specified values were loaded
    assert_eq!(config.server.port, 9090);
    assert_eq!(config.mode.repl, true);
    assert_eq!(config.mode.get_agent_card, false);
    
    // Check that default values were used for unspecified fields
    assert_eq!(config.server.bind_address, "0.0.0.0");
    assert!(config.server.agent_id.starts_with("bidirectional-"));
    assert_eq!(config.client.target_url, None);
}

#[test]
fn test_load_invalid_config() {
    // Create a temporary directory
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let config_path = temp_dir.path().join("invalid_config.toml");
    
    // Create an invalid TOML file
    let config_content = r#"
        [server
        port = 9090
        invalid syntax
    "#;
    
    fs::write(&config_path, config_content).expect("Failed to write test config");
    
    // Load the config - should fail
    let result = BidirectionalAgentConfig::from_file(&config_path);
    assert!(result.is_err());
}

#[test]
fn test_load_nonexistent_config() {
    // Try to load a nonexistent config file
    let result = BidirectionalAgentConfig::from_file(Path::new("/nonexistent/path/config.toml"));
    assert!(result.is_err());
}
