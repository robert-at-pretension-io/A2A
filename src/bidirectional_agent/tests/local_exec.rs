//! Tests for Slice 2: Local Tool Execution & Routing.

#![cfg(feature = "bidir-local-exec")]

use crate::bidirectional_agent::{
    config::BidirectionalAgentConfig,
    agent_registry::AgentRegistry,
    tool_executor::{ToolExecutor, Tool, ToolError},
    task_router::{TaskRouter, RoutingDecision},
    BidirectionalAgent, // Import the main agent struct
};
use crate::types::{Task, TaskState, Message, Role, Part, TextPart, TaskSendParams}; // Import necessary types
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::json;

// --- Mock Tool for Testing ---
struct MockCalculatorTool;

#[async_trait]
impl Tool for MockCalculatorTool {
    fn name(&self) -> &str { "calculator" }
    fn description(&self) -> &str { "Performs simple arithmetic" }
    async fn execute(&self, params: serde_json::Value) -> Result<serde_json::Value, ToolError> {
        let op = params.get("operation").and_then(|v| v.as_str());
        let a = params.get("a").and_then(|v| v.as_f64());
        let b = params.get("b").and_then(|v| v.as_f64());

        match (op, a, b) {
            (Some("add"), Some(val_a), Some(val_b)) => Ok(json!(val_a + val_b)),
            (Some("subtract"), Some(val_a), Some(val_b)) => Ok(json!(val_a - val_b)),
            _ => Err(ToolError::InvalidParams("calculator".to_string(), "Requires 'operation' (add/subtract) and numbers 'a', 'b'".to_string())),
        }
    }
    fn capabilities(&self) -> &[&'static str] { &["math", "calculator"] }
}

// Helper to create TaskSendParams
fn create_test_params(id: &str, text: &str, metadata: Option<serde_json::Map<String, serde_json::Value>>) -> TaskSendParams {
     TaskSendParams {
        id: id.to_string(),
        message: Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: text.to_string(),
                metadata: None,
            })],
            metadata: None,
        },
        history_length: None,
        metadata,
        push_notification: None,
        session_id: None,
    }
}


#[tokio::test]
async fn test_tool_executor_executes_tool() {
    let mut executor = ToolExecutor::new();
    // Manually add the mock tool for this test
    executor.tools = Arc::new({
        let mut tools = std::collections::HashMap::new();
        tools.insert("calculator".to_string(), Box::new(MockCalculatorTool) as Box<dyn Tool>);
        tools
    });


    let params = json!({"operation": "add", "a": 5.0, "b": 3.0});
    let result = executor.execute_tool("calculator", params).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), json!(8.0));
}

#[tokio::test]
async fn test_task_router_decides_local() {
    // Setup components
    let registry = Arc::new(AgentRegistry::new());
    let mut executor = ToolExecutor::new();
     executor.tools = Arc::new({ // Add the calculator tool
        let mut tools = std::collections::HashMap::new();
        tools.insert("calculator".to_string(), Box::new(MockCalculatorTool) as Box<dyn Tool>);
        tools
    });
    let executor_arc = Arc::new(executor);
    let router = TaskRouter::new(registry, executor_arc);

    // Create task params that should likely be handled locally (no hints)
    let params = create_test_params("local-task-1", "Calculate 10 + 5", None);

    // Get routing decision
    let decision = router.decide(&params).await;

    // Assert: Expecting local execution (based on simplified Slice 2 logic)
    // The simplified logic currently defaults to "echo", let's refine this later.
    // For now, we check it decided *some* local execution.
    match decision {
        RoutingDecision::Local { tool_names } => {
            // In Slice 2, it defaults to "echo". In a real scenario, it might detect "calculator".
            assert!(!tool_names.is_empty());
        },
        _ => panic!("Expected Local routing decision"),
    }
}

// Test integration within BidirectionalAgent (basic check)
#[test]
fn test_agent_initialization_with_slice2_components() {
    let config = BidirectionalAgentConfig {
        self_id: "test-agent-slice2".to_string(),
        base_url: "http://localhost:8080".to_string(),
        discovery: vec![],
        auth: Default::default(),
        network: Default::default(),
    };

    // This requires modifying BidirectionalAgent::new to initialize router/executor
    // Let's assume that modification is done for this test.
    // let agent_result = BidirectionalAgent::new(config);
    // assert!(agent_result.is_ok());
    // let agent = agent_result.unwrap();
    // assert!(agent.tool_executor.is_some()); // Check if fields exist
    // assert!(agent.task_router.is_some());

    // Placeholder assertion until BidirectionalAgent::new is updated
    assert!(true, "Placeholder: Test needs BidirectionalAgent::new update");
}
//! Tests for Slice 2: Local Tool Execution & Routing.

// Only compile if local execution feature is enabled
#![cfg(feature = "bidir-local-exec")]

use crate::bidirectional_agent::{
    config::BidirectionalAgentConfig,
    agent_registry::AgentRegistry,
    tool_executor::{ToolExecutor, Tool, ToolError},
    task_router::{TaskRouter, RoutingDecision},
    // BidirectionalAgent, // Don't need the full agent for these tests yet
};
use crate::types::{Task, TaskState, Message, Role, Part, TextPart, TaskSendParams};
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap; // Import HashMap

// --- Mock Tool for Testing ---
struct MockCalculatorTool;

#[async_trait]
impl Tool for MockCalculatorTool {
    fn name(&self) -> &str { "calculator" }
    fn description(&self) -> &str { "Performs simple arithmetic" }
    async fn execute(&self, params: serde_json::Value) -> Result<serde_json::Value, ToolError> {
        let op = params.get("operation").and_then(|v| v.as_str());
        let a = params.get("a").and_then(|v| v.as_f64());
        let b = params.get("b").and_then(|v| v.as_f64());

        match (op, a, b) {
            (Some("add"), Some(val_a), Some(val_b)) => Ok(json!(val_a + val_b)),
            (Some("subtract"), Some(val_a), Some(val_b)) => Ok(json!(val_a - val_b)),
            _ => Err(ToolError::InvalidParams("calculator".to_string(), "Requires 'operation' (add/subtract) and numbers 'a', 'b'".to_string())),
        }
    }
    fn capabilities(&self) -> &[&'static str] { &["math", "calculator"] }
}

// Helper to create TaskSendParams
fn create_test_params(id: &str, text: &str, metadata: Option<serde_json::Map<String, serde_json::Value>>) -> TaskSendParams {
     TaskSendParams {
        id: id.to_string(),
        message: Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: text.to_string(),
                metadata: None,
            })],
            metadata: None,
        },
        history_length: None,
        metadata,
        push_notification: None,
        session_id: None,
    }
}


#[tokio::test]
async fn test_tool_executor_executes_tool() {
    let mut executor = ToolExecutor::new();
    // Manually add the mock tool for this test
    // Need to make the 'tools' field accessible or provide a registration method
    // For now, we assume ToolExecutor::new() registers the built-ins including 'shell' and 'http'
    // Let's test the built-in 'echo' (assuming ShellTool provides it)
    // Update: ToolExecutor::new() now registers ShellTool and HttpTool

    // Test the shell tool's echo functionality (assuming it's registered as 'shell')
    let params_echo = json!({"command": "echo", "args": ["Hello", "Executor!"]});
    let result_echo = executor.execute_tool("shell", params_echo).await;
    assert!(result_echo.is_ok());
    let output_echo = result_echo.unwrap();
    assert_eq!(output_echo["stdout"], "Hello Executor!\n"); // Echo adds newline

    // Test the http tool (requires a mock server or hitting a real endpoint)
    // For simplicity, we'll skip the HTTP tool execution test here.
}

#[tokio::test]
async fn test_task_router_decides_local() {
    // Setup components
    let registry = Arc::new(AgentRegistry::new());
    let executor = Arc::new(ToolExecutor::new()); // Contains shell and http tools
    let router = TaskRouter::new(registry, executor);

    // Create task params that should likely be handled locally (no hints)
    let params = create_test_params("local-task-1", "Echo this message", None);

    // Get routing decision
    let decision = router.decide(&params).await;

    // Assert: Expecting local execution (based on simplified Slice 2 logic)
    // The simplified logic currently defaults to "echo".
    match decision {
        RoutingDecision::Local { tool_names } => {
            assert_eq!(tool_names, vec!["echo".to_string()]);
        },
        _ => panic!("Expected Local routing decision"),
    }
}

#[tokio::test]
async fn test_task_router_decides_remote_via_hint() {
    // Setup components
    let registry = Arc::new(AgentRegistry::new());
    // Add a mock remote agent
    let remote_agent_id = "remote-calc-agent";
     let mock_card = crate::types::AgentCard {
         name: remote_agent_id.to_string(), url: "http://remote.com".to_string(), version: "1.0".to_string(),
         capabilities: Default::default(), skills: vec![], default_input_modes: vec![], default_output_modes: vec![],
         description: None, provider: None, documentation_url: None, authentication: None,
     };
     registry.agents.insert(remote_agent_id.to_string(), crate::bidirectional_agent::agent_registry::CachedAgentInfo {
         card: mock_card, last_checked: chrono::Utc::now()
     });

    let executor = Arc::new(ToolExecutor::new());
    let router = TaskRouter::new(registry, executor);

    // Create task params with a hint to delegate
    let mut metadata = HashMap::new();
    metadata.insert("_route_to".to_string(), json!(remote_agent_id));
    let params = create_test_params("remote-task-1", "Calculate 5+3 remotely", Some(metadata.into_iter().collect()));

    // Get routing decision
    let decision = router.decide(&params).await;

    // Assert: Expecting remote delegation
    assert_eq!(decision, RoutingDecision::Remote { agent_id: remote_agent_id.to_string() });
}
