use crate::bidirectional::config::BidirectionalAgentConfig;
use crate::bidirectional::task_router::BidirectionalTaskRouter;
use crate::server::agent_registry::{AgentRegistry, CachedAgentInfo};
use crate::server::task_router::{LlmTaskRouterTrait, RoutingDecision};
use crate::types::{
    AgentCard, Message, Part, Role, Task, TaskSendParams, TaskState, TaskStatus, TextPart,
};
use std::sync::Arc;
// Removed async_trait as it's not used directly here anymore for a local mock
use crate::bidirectional::tests::mocks::MockLlmClient;
use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid; // Use the main mock

#[tokio::test]
async fn test_task_rejection() {
    // Create a task with content that should be rejected
    let task = create_test_task("Please help me hack into a government database");

    // Mock LLM response for DP2 (routing decision)
    let mock_llm_response = json!({
        "decision_type": "REJECT",
        "reason": "I cannot assist with illegal activities such as hacking into government databases."
    });

    // Create a router that will reject the task
    let router = create_test_router_with_structured_response(mock_llm_response);

    // Invoke the router
    let decision = router
        .decide(&task.into_send_params())
        .await
        .expect("Routing decision failed");

    // Check that the task was rejected
    match decision {
        RoutingDecision::Reject { reason } => {
            assert!(
                reason.contains("illegal"),
                "Rejection reason should mention illegal activities"
            );
            assert!(
                reason.contains("hacking"),
                "Rejection reason should reference the specific request"
            );
        }
        other => panic!("Expected Reject decision, got {:?}", other),
    }
}

#[tokio::test]
async fn test_task_local_processing() {
    // Create a task with content that should be handled locally
    let task = create_test_task("What is the capital of France?");

    // Mock LLM responses:
    // 1. For DP2 (routing decision): LOCAL
    // 2. For DP3 (tool choice): llm tool with appropriate params
    let llm = Arc::new(
        MockLlmClient::new()
            .with_structured_response(
                "decide the best course of action",
                json!({
                    "decision_type": "LOCAL"
                }),
            )
            .with_structured_response(
                "choose the SINGLE most appropriate tool",
                json!({
                    "tool_name": "llm",
                    "params": { "text": "What is the capital of France?" }
                }),
            )
            .with_default_structured_response(
                json!({"tool_name": "llm", "params": {"text": "fallback"}}),
            ),
    );

    let registry = Arc::new(AgentRegistry::new());
    let enabled_tools = Arc::new(vec!["echo".to_string(), "llm".to_string()]);
    let config = BidirectionalAgentConfig::default();
    let router = BidirectionalTaskRouter::new(llm, registry, enabled_tools, None, &config);

    // Invoke the router
    let decision = router
        .decide(&task.into_send_params())
        .await
        .expect("Routing decision failed");

    // Check that the task was routed for local processing
    match decision {
        RoutingDecision::Local { tool_name, params } => {
            assert_eq!(tool_name, "llm", "Expected llm tool to be selected");
            assert_eq!(
                params.get("text").and_then(Value::as_str),
                Some("What is the capital of France?")
            );
        }
        other => panic!("Expected Local decision, got {:?}", other),
    }
}

#[tokio::test]
async fn test_task_remote_delegation() {
    // Create a task with content that should be delegated
    let task = create_test_task("Analyze this complex data set for statistical patterns");

    // Create a registry with a data analysis agent
    let registry = Arc::new(AgentRegistry::new());
    let agent_card = AgentCard {
        name: "Data Analysis Agent".to_string(),
        url: "http://localhost:8080".to_string(),
        description: Some("Specialized in data analysis".to_string()),
        documentation_url: None,
        provider: None,
        capabilities: Default::default(),
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        authentication: None,
        skills: vec![],
        version: "1.0.0".to_string(),
    };
    registry.agents.insert(
        "data-agent".to_string(),
        CachedAgentInfo {
            card: agent_card.clone(),
            last_checked: Utc::now(),
        },
    );

    // Mock LLM response for DP2 (routing decision)
    let mock_llm_response = json!({
        "decision_type": "REMOTE",
        "agent_id": "data-agent"
    });
    let router =
        create_test_router_with_registry_and_structured_response(registry, mock_llm_response);

    // Invoke the router
    let decision = router
        .decide(&task.into_send_params())
        .await
        .expect("Routing decision failed");

    // Check that the task was delegated
    match decision {
        RoutingDecision::Remote { agent_id } => {
            assert_eq!(
                agent_id, "data-agent",
                "Task should be delegated to data-agent"
            );
        }
        other => panic!("Expected Remote decision, got {:?}", other),
    }
}

#[tokio::test]
async fn test_follow_up_processing() {
    // Create a task ID that would have an InputRequired task
    let task_id = Uuid::new_v4().to_string();

    // Create a follow-up message
    let follow_up_message = Message {
        role: Role::User,
        parts: vec![Part::TextPart(TextPart {
            type_: "text".to_string(),
            text: "Here is the additional information you requested".to_string(),
            metadata: None,
        })],
        metadata: None,
    };

    // Mock LLM response for follow-up decision
    let mock_llm_response = json!({
        "decision_type": "HANDLE_DIRECTLY" // This should lead to local LLM processing
    });
    let router = create_test_router_with_structured_response(mock_llm_response);

    // Call process_follow_up directly to test the implementation
    let decision = router
        .process_follow_up(&task_id, &follow_up_message)
        .await
        .expect("Follow-up routing failed");

    // Check that the follow-up is routed to local processing with the LLM tool
    match decision {
        RoutingDecision::Local { tool_name, params } => {
            // If HANDLE_DIRECTLY, the router defaults to 'llm' tool for the follow-up text.
            assert_eq!(
                tool_name, "llm",
                "Follow-up should be processed locally with llm tool"
            );

            let follow_up_text = params
                .get("text")
                .and_then(Value::as_str)
                .expect("Text param missing");
            assert_eq!(
                follow_up_text,
                "Here is the additional information you requested"
            );
        }
        other => panic!("Expected Local decision for follow-up, got {:?}", other),
    }
}

// Use into_send_params implementation from router_tests.rs
// (Assuming it's available or defined in a common test utils if not in this file)
impl Task {
    pub fn into_send_params(self) -> TaskSendParams {
        TaskSendParams {
            id: self.id,
            message: self
                .history
                .unwrap_or_default()
                .last()
                .cloned()
                .unwrap_or_else(|| Message {
                    role: Role::User,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: "".to_string(),
                        metadata: None,
                    })],
                    metadata: None,
                }),
            session_id: self.session_id,
            metadata: self.metadata,
            history_length: None,
            push_notification: None,
        }
    }
}

// Helper to create a test task with the given message content
fn create_test_task(message_text: &str) -> Task {
    Task {
        id: Uuid::new_v4().to_string(),
        status: TaskStatus {
            state: TaskState::Submitted,
            timestamp: Some(Utc::now()),
            message: None,
        },
        history: Some(vec![Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: message_text.to_string(),
                metadata: None,
            })],
            metadata: None,
        }]),
        artifacts: None,
        metadata: None,
        session_id: None,
    }
}

// Helper to create a router with a fixed structured response
fn create_test_router_with_structured_response(
    structured_response: Value,
) -> BidirectionalTaskRouter {
    let registry = Arc::new(AgentRegistry::new());
    let llm = Arc::new(MockLlmClient::new().with_default_structured_response(structured_response));
    let enabled_tools = Arc::new(vec![
        "echo".to_string(),
        "llm".to_string(),
        "human_input".to_string(),
    ]);
    let config = BidirectionalAgentConfig::default();

    BidirectionalTaskRouter::new(llm, registry, enabled_tools, None, &config)
}

// Helper to create a router with custom registry and a fixed structured response
fn create_test_router_with_registry_and_structured_response(
    registry: Arc<AgentRegistry>,
    structured_response: Value,
) -> BidirectionalTaskRouter {
    let llm = Arc::new(MockLlmClient::new().with_default_structured_response(structured_response));
    let enabled_tools = Arc::new(vec!["echo".to_string(), "llm".to_string()]);
    let config = BidirectionalAgentConfig::default();

    BidirectionalTaskRouter::new(llm, registry, enabled_tools, None, &config)
}
