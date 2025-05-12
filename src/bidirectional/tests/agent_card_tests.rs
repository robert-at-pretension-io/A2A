use crate::bidirectional::agent_helpers;
use crate::bidirectional::bidirectional_agent::BidirectionalAgent;
use crate::bidirectional::config::BidirectionalAgentConfig;
use crate::server;
use crate::types::AgentCapabilities;
use std::env;
use std::net::IpAddr;
use std::str::FromStr;

fn create_test_agent(
    agent_id: &str,
    agent_name: Option<&str>,
    port: u16,
    bind_address: &str,
) -> BidirectionalAgent {
    // Set up a test API key for agent creation
    env::set_var("CLAUDE_API_KEY", "test-claude-key");

    // Create a test configuration
    let mut config = BidirectionalAgentConfig::default();
    config.llm.claude_api_key = Some("test-claude-key".to_string());
    config.server.agent_id = agent_id.to_string();
    config.server.agent_name = agent_name.map(|s| s.to_string());
    config.server.port = port;
    config.server.bind_address = bind_address.to_string();

    // Create test agent
    match BidirectionalAgent::new(config) {
        Ok(agent) => agent,
        Err(e) => panic!("Failed to create test agent: {}", e),
    }
}

#[test]
fn test_agent_card_default_values() {
    // Create an agent with minimal configuration
    let agent = create_test_agent("test-agent", None, 8080, "127.0.0.1");

    // Create the agent card
    let card = agent_helpers::create_agent_card(&agent);

    // Verify default values
    assert_eq!(card.name, "test-agent"); // Should use agent_id when agent_name is None
    assert_eq!(card.url, "http://127.0.0.1:8080"); // Should use bind_address and port
    assert_eq!(card.version, "1.0.0"); // Should use default version
    assert!(card.description.is_some());
    assert_eq!(
        card.description.unwrap(),
        "Bidirectional A2A Agent (ID: test-agent)"
    );

    // Verify capabilities
    assert!(card.capabilities.streaming);
    assert!(card.capabilities.push_notifications);
    assert!(card.capabilities.state_transition_history);

    // Verify default modes
    assert_eq!(card.default_input_modes, vec!["text".to_string()]);
    assert_eq!(card.default_output_modes, vec!["text".to_string()]);

    // Verify other properties
    assert!(card.skills.is_empty());
    assert!(card.authentication.is_none());
    assert!(card.documentation_url.is_none());
    assert!(card.provider.is_none());
}

#[test]
fn test_agent_card_with_custom_name() {
    // Create an agent with a custom name
    let agent = create_test_agent("test-agent", Some("Custom Agent Name"), 8081, "127.0.0.1");

    // Create the agent card
    let card = agent_helpers::create_agent_card(&agent);

    // Verify custom name was used
    assert_eq!(card.name, "Custom Agent Name");
    assert_eq!(card.url, "http://127.0.0.1:8081");
    assert!(card.description.is_some());
    assert_eq!(
        card.description.unwrap(),
        "Bidirectional A2A Agent (ID: test-agent)"
    );
}

#[test]
fn test_agent_card_with_ipv6_address() {
    // Check if we can parse an IPv6 address
    if IpAddr::from_str("::1").is_ok() {
        // Create an agent with IPv6 loopback
        let agent = create_test_agent("ipv6-agent", None, 8082, "::1");

        // Create the agent card
        let card = agent_helpers::create_agent_card(&agent);

        // Verify URL format for IPv6
        assert_eq!(card.url, "http://[::1]:8082");
    }
}

#[test]
fn test_agent_card_with_unique_port() {
    // Create an agent with a custom port
    let agent = create_test_agent("port-agent", None, 9999, "127.0.0.1");

    // Create the agent card
    let card = agent_helpers::create_agent_card(&agent);

    // Verify port was used in URL
    assert_eq!(card.url, "http://127.0.0.1:9999");
}

#[test]
fn test_server_create_agent_card_dynamic_params() {
    // Test server's create_agent_card function with dynamic parameters
    let custom_name = "Custom Server Name";
    let custom_description = "This is a custom server description";
    let custom_url = "https://custom.example.com";
    let custom_version = "2.5.0";
    
    // Create custom skills
    let skills = serde_json::json!([
        {
            "id": "translate",
            "name": "Translation",
            "description": "Translates text between languages",
            "tags": ["language", "translation"],
            "input_modes": ["text"],
            "output_modes": ["text"]
        },
        {
            "id": "calculate",
            "name": "Calculator",
            "description": "Performs mathematical calculations",
            "tags": ["math", "calculation"],
            "input_modes": ["text"],
            "output_modes": ["text"]
        }
    ]);

    // Create agent card with all custom parameters
    let card_json = server::create_agent_card(
        Some(custom_name),
        Some(custom_description),
        Some(custom_url),
        Some(custom_version),
        Some(skills.clone()),
    );

    // Verify all custom values were used
    assert_eq!(card_json["name"], custom_name);
    assert_eq!(card_json["description"], custom_description);
    assert_eq!(card_json["url"], custom_url);
    assert_eq!(card_json["version"], custom_version);
    assert_eq!(card_json["skills"], skills);

    // Verify capabilities are always set
    assert!(card_json["capabilities"]["streaming"].as_bool().unwrap());
    assert!(card_json["capabilities"]["pushNotifications"].as_bool().unwrap());
    assert!(card_json["capabilities"]["stateTransitionHistory"].as_bool().unwrap());
}

#[test]
fn test_server_create_agent_card_partial_params() {
    // Test with only some parameters provided
    let card_json = server::create_agent_card(
        Some("Partial Custom Name"),
        None,  // Use default description
        None,  // Use default URL
        Some("3.0.0"),
        None,  // Use default skills
    );

    // Verify the provided values were used
    assert_eq!(card_json["name"], "Partial Custom Name");
    assert_eq!(card_json["version"], "3.0.0");

    // Verify defaults for missing values
    assert_eq!(card_json["description"], "A reference implementation of the A2A protocol for testing");
    assert_eq!(card_json["url"], "http://localhost:8081");
    
    // Verify the skills array contains the default echo skill
    let skills = card_json["skills"].as_array().unwrap();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0]["id"], "echo");
}

#[test]
fn test_agent_card_serialization() {
    // Create an agent
    let agent = create_test_agent("serialization-test", None, 8085, "127.0.0.1");
    
    // Create the agent card
    let card = agent_helpers::create_agent_card(&agent);
    
    // Serialize to JSON
    let json_string = serde_json::to_string(&card).unwrap();
    let json_value: serde_json::Value = serde_json::from_str(&json_string).unwrap();
    
    // Verify JSON structure matches schema requirements
    assert!(json_value["name"].is_string());
    assert!(json_value["url"].is_string());
    assert!(json_value["version"].is_string());
    assert!(json_value["capabilities"].is_object());
    assert!(json_value["defaultInputModes"].is_array());
    assert!(json_value["defaultOutputModes"].is_array());
    assert!(json_value["capabilities"]["streaming"].as_bool().unwrap());
    assert!(json_value["capabilities"]["pushNotifications"].as_bool().unwrap());
    assert!(json_value["capabilities"]["stateTransitionHistory"].as_bool().unwrap());
}

#[test]
fn test_agent_capabilities_consistency() {
    // Create an AgentCapabilities struct directly
    let capabilities = AgentCapabilities {
        streaming: true,
        push_notifications: true,
        state_transition_history: true,
    };
    
    // Serialize to JSON
    let json_string = serde_json::to_string(&capabilities).unwrap();
    let json_value: serde_json::Value = serde_json::from_str(&json_string).unwrap();
    
    // Check serialized field names match schema
    assert!(json_value["streaming"].as_bool().unwrap());
    assert!(json_value["pushNotifications"].as_bool().unwrap());
    assert!(json_value["stateTransitionHistory"].as_bool().unwrap());
    
    // Create an agent card
    let agent = create_test_agent("capabilities-test", None, 8086, "127.0.0.1");
    let card = agent_helpers::create_agent_card(&agent);
    
    // Compare capabilities
    assert_eq!(card.capabilities.streaming, capabilities.streaming);
    assert_eq!(card.capabilities.push_notifications, capabilities.push_notifications);
    assert_eq!(card.capabilities.state_transition_history, capabilities.state_transition_history);
}

#[test]
fn test_complex_agent_card_creation() {
    // Create a complex custom card via the server function
    let name = "Complex Test Agent";
    let description = "An agent with complex custom configuration";
    let url = "https://complex.example.org:8443";
    let version = "4.2.1";
    
    let skills = serde_json::json!([
        {
            "id": "code-generation",
            "name": "Code Generator",
            "description": "Generates code in various programming languages",
            "tags": ["programming", "development", "code"],
            "input_modes": ["text"],
            "output_modes": ["text", "code"]
        },
        {
            "id": "data-analysis",
            "name": "Data Analysis",
            "description": "Analyzes structured data and provides insights",
            "tags": ["data", "analysis", "statistics"],
            "input_modes": ["text", "data"],
            "output_modes": ["text", "data", "visualization"]
        },
        {
            "id": "qa",
            "name": "Question Answering",
            "description": "Answers questions based on the input",
            "tags": ["question", "answer", "qa"],
            "input_modes": ["text"],
            "output_modes": ["text"]
        }
    ]);
    
    // Create card JSON
    let card_json = server::create_agent_card(
        Some(name),
        Some(description),
        Some(url),
        Some(version),
        Some(skills.clone()),
    );
    
    // Verify complex configuration
    assert_eq!(card_json["name"], name);
    assert_eq!(card_json["description"], description);
    assert_eq!(card_json["url"], url);
    assert_eq!(card_json["version"], version);
    
    // Verify skills array
    let skills_array = card_json["skills"].as_array().unwrap();
    assert_eq!(skills_array.len(), 3);
    
    // Check each skill
    let skill_ids: Vec<&str> = skills_array
        .iter()
        .map(|s| s["id"].as_str().unwrap())
        .collect();
    
    assert!(skill_ids.contains(&"code-generation"));
    assert!(skill_ids.contains(&"data-analysis"));
    assert!(skill_ids.contains(&"qa"));
    
    // Verify multimodal capabilities in skills
    let data_analysis_skill = skills_array.iter()
        .find(|s| s["id"] == "data-analysis")
        .unwrap();
    
    let input_modes: Vec<String> = data_analysis_skill["input_modes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_str().unwrap().to_string())
        .collect();
    
    let output_modes: Vec<String> = data_analysis_skill["output_modes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_str().unwrap().to_string())
        .collect();
    
    assert!(input_modes.contains(&"text".to_string()));
    assert!(input_modes.contains(&"data".to_string()));
    assert!(output_modes.contains(&"text".to_string()));
    assert!(output_modes.contains(&"data".to_string()));
    assert!(output_modes.contains(&"visualization".to_string()));
}