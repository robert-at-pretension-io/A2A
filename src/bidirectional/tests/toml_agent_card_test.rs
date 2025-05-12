use crate::bidirectional::config::BidirectionalAgentConfig;
use crate::bidirectional::bidirectional_agent::BidirectionalAgent;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::io::Write;
use std::net::SocketAddr;
use std::str::FromStr;
use tempfile::NamedTempFile;

#[test]
fn test_agent_card_from_toml_config() {
    // Create a temporary TOML config file with custom values
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    
    // Write custom configuration to the file
    let toml_content = r#"
    [server]
    port = 9876
    bind_address = "127.0.0.5"
    agent_id = "toml-configured-agent"
    agent_name = "TOML Configured Agent"

    [client]
    # No client configuration needed

    [llm]
    # Dummy values for testing
    claude_api_key = "test-claude-key"

    [mode]
    # No mode configuration needed
    
    [tools]
    # No tools configuration needed
    "#;
    
    temp_file.write_all(toml_content.as_bytes()).expect("Failed to write to temp file");
    
    // Make sure environment has the necessary value for agent creation
    env::set_var("CLAUDE_API_KEY", "test-claude-key");
    
    // Load the configuration from the TOML file
    let config_path = temp_file.path().to_str().unwrap();
    let mut config = BidirectionalAgentConfig::default();
    config = match BidirectionalAgentConfig::from_file(config_path) {
        Ok(c) => c,
        Err(e) => panic!("Failed to load configuration from TOML: {}", e),
    };
    
    // Create agent from the loaded configuration
    let agent = match BidirectionalAgent::new(config) {
        Ok(a) => a,
        Err(e) => panic!("Failed to create agent from TOML config: {}", e),
    };
    
    // Create the agent card
    let card = agent.create_agent_card();
    
    // Verify that the values from the TOML file were correctly used
    assert_eq!(card.name, "TOML Configured Agent");
    assert_eq!(card.url, "http://127.0.0.5:9876");
    assert!(card.description.is_some());
    assert_eq!(card.description.unwrap(), "Bidirectional A2A Agent (ID: toml-configured-agent)");
}

#[test]
fn test_agent_card_from_toml_with_custom_fields() {
    // Create a temporary TOML config file with comprehensive custom values
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    
    // Write custom configuration to the file with more specific details
    let toml_content = r#"
    [server]
    port = 8765
    bind_address = "localhost"
    agent_id = "fully-configured-agent"
    agent_name = "Fully Configured Agent"
    
    [client]
    target_url = "https://target.example.com"

    [llm]
    claude_api_key = "test-claude-key"
    system_prompt = "Custom system prompt for the agent"
    
    [registry]
    registry_path = "./data/test_registry.json"
    registry_only_mode = false

    [mode]
    repl = true
    repl_log_file = "./logs/repl_test.log"
    "#;
    
    temp_file.write_all(toml_content.as_bytes()).expect("Failed to write to temp file");
    
    // Make sure environment has the necessary value for agent creation
    env::set_var("CLAUDE_API_KEY", "test-claude-key");
    
    // Load the configuration from the TOML file
    let config_path = temp_file.path().to_str().unwrap();
    let config = match BidirectionalAgentConfig::from_file(config_path) {
        Ok(c) => c,
        Err(e) => panic!("Failed to load configuration from TOML: {}", e),
    };
    
    // Verify the configuration values were correctly loaded
    assert_eq!(config.server.port, 8765);
    assert_eq!(config.server.bind_address, "localhost");
    assert_eq!(config.server.agent_id, "fully-configured-agent");
    assert_eq!(config.server.agent_name, Some("Fully Configured Agent".to_string()));
    assert_eq!(config.llm.system_prompt, "Custom system prompt for the agent");
    assert_eq!(config.client.target_url, Some("https://target.example.com".to_string()));
    
    // Create agent from the loaded configuration
    let agent = match BidirectionalAgent::new(config) {
        Ok(a) => a,
        Err(e) => panic!("Failed to create agent from TOML config: {}", e),
    };
    
    // Create the agent card
    let card = agent.create_agent_card();
    
    // Verify that the values from the TOML file were correctly used in the agent card
    assert_eq!(card.name, "Fully Configured Agent");
    assert_eq!(card.url, "http://localhost:8765");
    assert!(card.description.is_some());
    assert_eq!(card.description.unwrap(), "Bidirectional A2A Agent (ID: fully-configured-agent)");
    
    // Verify capabilities
    assert!(card.capabilities.streaming);
    assert!(card.capabilities.push_notifications);
    assert!(card.capabilities.state_transition_history);
}

#[test]
fn test_bidirectional_agent_from_example_toml() {
    // Read the example bidirectional_agent.toml provided in the repository
    let example_path = PathBuf::from("bidirectional_agent.toml");
    
    // Skip the test if the example file doesn't exist
    if !example_path.exists() {
        eprintln!("Example TOML file not found: {}", example_path.display());
        return;
    }
    
    // Make sure environment has the necessary value for agent creation
    env::set_var("CLAUDE_API_KEY", "test-claude-key");
    
    // Load the configuration from the example file
    let config_path = example_path.to_str().unwrap();
    let config = match BidirectionalAgentConfig::from_file(config_path) {
        Ok(c) => c,
        Err(e) => panic!("Failed to load configuration from example TOML: {}", e),
    };
    
    // Create agent from the loaded configuration
    let agent = match BidirectionalAgent::new(config.clone()) {
        Ok(a) => a,
        Err(e) => panic!("Failed to create agent from example TOML config: {}", e),
    };
    
    // Create the agent card
    let card = agent.create_agent_card();
    
    // Verify that the agent card has the values from the example TOML
    assert_eq!(card.name, config.server.agent_name.unwrap_or(config.server.agent_id.clone()));
    
    // Verify the URL format
    let expected_url = format!("http://{}:{}", config.server.bind_address, config.server.port);
    assert_eq!(card.url, expected_url);
    
    // Verify description contains the agent_id from the config
    assert!(card.description.is_some());
    assert!(card.description.unwrap().contains(&config.server.agent_id));
}

#[test]
fn test_complex_toml_config_ipv6() {
    // Skip if IPv6 isn't possible on this machine
    if SocketAddr::from_str("[::1]:8080").is_err() {
        eprintln!("IPv6 not supported on this system, skipping test");
        return;
    }
    
    // Create a temporary TOML config file with IPv6 configuration
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    
    // Write custom configuration with IPv6 address
    let toml_content = r#"
    [server]
    port = 7654
    bind_address = "::1"
    agent_id = "ipv6-agent"
    agent_name = "IPv6 Agent"

    [client]
    # No client configuration needed

    [llm]
    # Dummy values for testing
    claude_api_key = "test-claude-key"
    "#;
    
    temp_file.write_all(toml_content.as_bytes()).expect("Failed to write to temp file");
    
    // Make sure environment has the necessary value for agent creation
    env::set_var("CLAUDE_API_KEY", "test-claude-key");
    
    // Load the configuration from the TOML file
    let config_path = temp_file.path().to_str().unwrap();
    let config = match BidirectionalAgentConfig::from_file(config_path) {
        Ok(c) => c,
        Err(e) => panic!("Failed to load configuration from TOML: {}", e),
    };
    
    // Create agent from the loaded configuration
    let agent = match BidirectionalAgent::new(config) {
        Ok(a) => a,
        Err(e) => panic!("Failed to create agent from TOML config: {}", e),
    };
    
    // Create the agent card
    let card = agent.create_agent_card();
    
    // Verify that the values were correctly used
    assert_eq!(card.name, "IPv6 Agent");
    assert_eq!(card.url, "http://[::1]:7654"); // IPv6 should be properly formatted with []
}