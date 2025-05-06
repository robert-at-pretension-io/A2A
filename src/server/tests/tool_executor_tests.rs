use crate::server::tool_executor::{ListAgentsTool, Tool, ToolExecutor};
// Import the canonical AgentRegistry instead of the local AgentDirectory
use crate::server::agent_registry::{AgentRegistry, CachedAgentInfo};
use crate::types::AgentCard;
use chrono::Utc;
use serde_json::{json, Value};
use std::any::Any;
use std::sync::Arc;

// Test implementation of LlmClient for testing
#[derive(Clone)]
struct MockLlmClient;

#[async_trait::async_trait]
impl crate::bidirectional::llm_client::LlmClient for MockLlmClient {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    async fn complete(
        &self,
        _prompt: &str,
        _system_prompt_override: Option<&str>,
    ) -> anyhow::Result<String> {
        Ok("mock response".to_string())
    }

    async fn complete_structured(
        &self,
        _prompt_text: &str,
        _system_prompt_override: Option<&str>,
        _output_schema: Value,
    ) -> anyhow::Result<Value> {
        Ok(json!({"mock_structured_response": "ok"}))
    }
}

#[tokio::test]
async fn test_tool_executor_list_agents_with_skills() {
    // Instead of testing the bidirectional agent implementation directly,
    // let's verify that the ListAgentsTool returns agent cards with skills

    // Create agent registry
    let registry = Arc::new(AgentRegistry::new());

    // Add a test agent with skills to the registry
    let card = AgentCard { // Removed mut
        name: "Test Agent".to_string(),
        url: "http://localhost:8080".to_string(),
        description: Some("Test agent".to_string()),
        documentation_url: None,
        provider: None,
        capabilities: Default::default(),
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        authentication: None,
        skills: vec![crate::types::AgentSkill {
            id: "echo".to_string(),
            name: "Echo Tool".to_string(),
            description: Some("Echoes back the input text".to_string()),
            examples: None,
            input_modes: Some(vec!["text".to_string()]),
            output_modes: Some(vec!["text".to_string()]),
            tags: Some(vec!["echo".to_string(), "text_manipulation".to_string()]),
        }],
        version: "1.0.0".to_string(),
    };

    registry.agents.insert(
        "test-agent".to_string(),
        CachedAgentInfo {
            card: card.clone(),
            last_checked: Utc::now(),
        },
    );

    // Create the list_agents tool with the registry
    let tool = ListAgentsTool::new(registry);

    // Execute the tool to get agent details
    let result = tool
        .execute(json!({}))
        .await
        .expect("Tool execution failed");

    // Extract the agents array from the result
    let agents = result
        .get("agents")
        .and_then(Value::as_array)
        .expect("Missing agents array");
    assert_eq!(agents.len(), 1, "Should return 1 agent");

    // Verify the agent has skills in the result
    let agent = &agents[0];
    let skills = agent
        .get("skills")
        .and_then(Value::as_array)
        .expect("Agent should have skills array");
    assert_eq!(skills.len(), 1, "Agent should have 1 skill");

    // Verify the skill details
    let skill = &skills[0];
    assert_eq!(
        skill.get("id").and_then(Value::as_str).unwrap(),
        "echo",
        "Skill should have correct id"
    );
    assert_eq!(
        skill.get("name").and_then(Value::as_str).unwrap(),
        "Echo Tool",
        "Skill should have correct name"
    );
    assert!(skill.get("tags").is_some(), "Skill should have tags");
}

#[tokio::test]
async fn test_list_agents_tool_empty() {
    // Create empty agent registry
    let registry = Arc::new(AgentRegistry::new());

    // Create tool, passing the registry
    let tool = ListAgentsTool::new(registry);

    // Execute with empty registry
    let result = tool
        .execute(json!({}))
        .await
        .expect("Tool execution failed");

    // Verify result structure
    let count = result.get("count").and_then(Value::as_u64).unwrap_or(999);
    assert_eq!(count, 0, "Empty directory should return count 0");

    assert!(
        result.get("message").is_some(),
        "Should have message field for empty directory"
    );
}

#[tokio::test]
async fn test_list_agents_tool_with_agents() {
    // Create agent registry with entries
    let registry = Arc::new(AgentRegistry::new());

    // Create sample agent cards
    let card1 = AgentCard {
        name: "Agent One".to_string(),
        url: "http://localhost:1111".to_string(),
        description: Some("First test agent".to_string()),
        documentation_url: None,
        provider: None,
        capabilities: Default::default(),
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        authentication: None,
        skills: vec![],
        version: "1.0.0".to_string(),
    };

    let card2 = AgentCard {
        name: "Agent Two".to_string(),
        url: "http://localhost:2222".to_string(),
        description: Some("Second test agent".to_string()),
        documentation_url: None,
        provider: None,
        capabilities: Default::default(),
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        authentication: None,
        skills: vec![],
        version: "1.0.0".to_string(),
    };

    // Add agents to registry (using discover, which requires a running server or mock)
    // For simplicity in this unit test, we'll assume discover works and adds them.
    // A more complex test might involve mocking the A2aClient used by discover.
    // We'll manually insert into the registry's internal map for this test.
    registry.agents.insert(
        "agent-1".to_string(),
        CachedAgentInfo {
            card: card1.clone(),
            last_checked: Utc::now(),
        },
    );
    registry.agents.insert(
        "agent-2".to_string(),
        CachedAgentInfo {
            card: card2.clone(),
            last_checked: Utc::now(),
        },
    );

    // Create tool, passing the registry
    let tool = ListAgentsTool::new(registry);

    // Test detailed format (default)
    let detailed_result = tool
        .execute(json!({}))
        .await
        .expect("Tool execution failed");
    let detailed_count = detailed_result
        .get("count")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    assert_eq!(detailed_count, 2, "Should report 2 agents");

    let agents = detailed_result
        .get("agents")
        .and_then(Value::as_array)
        .expect("Missing agents array");
    assert_eq!(agents.len(), 2, "Should return 2 agents in array");

    // Check that each agent has the id field added
    for agent in agents {
        assert!(agent.get("id").is_some(), "Agent should have id field");
        assert!(agent.get("name").is_some(), "Agent should have name field");
        assert!(agent.get("url").is_some(), "Agent should have url field");
    }

    // Test simple format
    let simple_result = tool
        .execute(json!({ "format": "simple" }))
        .await
        .expect("Tool execution failed");
    let simple_agents = simple_result
        .get("agents")
        .and_then(Value::as_array)
        .expect("Missing agents array");

    assert_eq!(
        simple_agents.len(),
        2,
        "Simple format should return 2 agents"
    );

    // Check that simple format only includes id and name
    let first_agent = &simple_agents[0];
    assert_eq!(
        first_agent.as_object().unwrap().len(),
        2,
        "Simple format should only have id and name"
    );
    assert!(
        first_agent.get("id").is_some(),
        "Simple format should have id"
    );
    assert!(
        first_agent.get("name").is_some(),
        "Simple format should have name"
    );
    assert!(
        first_agent.get("url").is_none(),
        "Simple format should not have url"
    );
}

#[tokio::test]
async fn test_tool_executor_with_list_agents() {
    // Create test components
    let registry = Arc::new(AgentRegistry::new()); // Use registry
    let llm = Arc::new(MockLlmClient);

    // Add a test agent to the registry
    let card = AgentCard {
        name: "Test Agent".to_string(),
        url: "http://localhost:3333".to_string(),
        description: Some("Test agent for executor test".to_string()),
        documentation_url: None,
        provider: None,
        capabilities: Default::default(),
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        authentication: None,
        skills: vec![],
        version: "1.0.0".to_string(),
    };

    // Add agent to registry's internal map for the test
    registry.agents.insert(
        "test-agent".to_string(),
        CachedAgentInfo {
            card: card.clone(),
            last_checked: Utc::now(),
        },
    );

    // Create tool executor with list_agents enabled, passing the registry
    let executor = ToolExecutor::with_enabled_tools(
        &["echo".to_string(), "list_agents".to_string()], // tool_list
        false, // is_exclusion_list (false means tool_list is an inclusion list)
        Some(llm), // llm
        Some(registry), // agent_registry
        None, // known_servers
        "test-agent", // agent_id
        "Test Agent", // agent_name
        "1.0.0",      // agent_version
        "localhost",  // bind_address
        8080,         // port
    );

    // Check that list_agents tool is registered
    let tools = &executor.tools;
    assert!(
        tools.contains_key("list_agents"),
        "list_agents tool should be registered"
    );

    // Execute the tool
    let result = executor
        .execute_tool("list_agents", json!({}))
        .await
        .expect("Tool execution should succeed");

    // Verify result
    let count = result.get("count").and_then(Value::as_u64).unwrap_or(0);
    assert_eq!(count, 1, "Should report 1 agent");
}
