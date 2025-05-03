use crate::server::tool_executor::{ToolExecutor, ListAgentsTool, Tool, ToolError};
use crate::bidirectional::bidirectional_agent::AgentDirectory;
use crate::types::AgentCard;
use std::sync::Arc;
use serde_json::{json, Value};

// Test implementation of LlmClient for testing
#[derive(Clone)]
struct MockLlmClient;

#[async_trait::async_trait]
impl crate::bidirectional::bidirectional_agent::LlmClient for MockLlmClient {
    async fn complete(&self, _prompt: &str) -> anyhow::Result<String> {
        Ok("mock response".to_string())
    }
}

#[tokio::test]
async fn test_list_agents_tool_empty() {
    // Create empty agent directory
    let directory = Arc::new(AgentDirectory::new());
    
    // Create tool
    let tool = ListAgentsTool::new(directory);
    
    // Execute with empty directory
    let result = tool.execute(json!({})).await.expect("Tool execution failed");
    
    // Verify result structure
    let count = result.get("count").and_then(Value::as_u64).unwrap_or(999);
    assert_eq!(count, 0, "Empty directory should return count 0");
    
    assert!(result.get("message").is_some(), "Should have message field for empty directory");
}

#[tokio::test]
async fn test_list_agents_tool_with_agents() {
    // Create agent directory with entries
    let directory = Arc::new(AgentDirectory::new());
    
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
    
    // Add agents to directory
    directory.add_or_update_agent("agent-1".to_string(), card1);
    directory.add_or_update_agent("agent-2".to_string(), card2);
    
    // Create tool
    let tool = ListAgentsTool::new(directory);
    
    // Test detailed format (default)
    let detailed_result = tool.execute(json!({})).await.expect("Tool execution failed");
    let detailed_count = detailed_result.get("count").and_then(Value::as_u64).unwrap_or(0);
    assert_eq!(detailed_count, 2, "Should report 2 agents");
    
    let agents = detailed_result.get("agents").and_then(Value::as_array).expect("Missing agents array");
    assert_eq!(agents.len(), 2, "Should return 2 agents in array");
    
    // Check that each agent has the id field added
    for agent in agents {
        assert!(agent.get("id").is_some(), "Agent should have id field");
        assert!(agent.get("name").is_some(), "Agent should have name field");
        assert!(agent.get("url").is_some(), "Agent should have url field");
    }
    
    // Test simple format
    let simple_result = tool.execute(json!({ "format": "simple" })).await.expect("Tool execution failed");
    let simple_agents = simple_result.get("agents").and_then(Value::as_array).expect("Missing agents array");
    
    assert_eq!(simple_agents.len(), 2, "Simple format should return 2 agents");
    
    // Check that simple format only includes id and name
    let first_agent = &simple_agents[0];
    assert_eq!(first_agent.as_object().unwrap().len(), 2, "Simple format should only have id and name");
    assert!(first_agent.get("id").is_some(), "Simple format should have id");
    assert!(first_agent.get("name").is_some(), "Simple format should have name");
    assert!(first_agent.get("url").is_none(), "Simple format should not have url");
}

#[tokio::test]
async fn test_tool_executor_with_list_agents() {
    // Create test components
    let directory = Arc::new(AgentDirectory::new());
    let llm = Arc::new(MockLlmClient);
    
    // Add a test agent
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
    
    directory.add_or_update_agent("test-agent".to_string(), card);
    
    // Create tool executor with list_agents enabled
    let executor = ToolExecutor::with_enabled_tools(
        &["echo".to_string(), "list_agents".to_string()],
        Some(llm), // Wrap LLM client in Some()
        Some(directory),
        None, // Add None for the missing agent_registry argument
        None, // Add None for the missing known_servers argument
    );
    
    // Check that list_agents tool is registered
    let tools = &executor.tools;
    assert!(tools.contains_key("list_agents"), "list_agents tool should be registered");
    
    // Execute the tool
    let result = executor.execute_tool("list_agents", json!({})).await
        .expect("Tool execution should succeed");
    
    // Verify result
    let count = result.get("count").and_then(Value::as_u64).unwrap_or(0);
    assert_eq!(count, 1, "Should report 1 agent");
}
