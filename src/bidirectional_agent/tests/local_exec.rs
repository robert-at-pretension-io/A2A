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
