// Import from task_router
use crate::bidirectional::config::BidirectionalAgentConfig;
use crate::bidirectional::task_router::BidirectionalTaskRouter;
use crate::bidirectional::tests::mocks::MockLlmClient;
use crate::server::agent_registry::{AgentRegistry, CachedAgentInfo};
use crate::server::task_router::{LlmTaskRouterTrait, RoutingDecision};
use crate::types::{
    AgentCapabilities, AgentCard, AgentSkill, Message, Part, Role, Task, TaskSendParams, TaskState,
    TaskStatus, TextPart, Value,
};
use chrono::Utc;
use serde_json::json; // Import json macro
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn test_router_local_decision() {
    // Mock LLM responses:
    // 1. For DP2 (routing decision): LOCAL
    // 2. For DP3 (tool choice): echo tool with "hello"
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
                    "tool_name": "echo",
                    "params": { "text": "hello" }
                }),
            )
            // Add a default in case prompt matching is too brittle or for unexpected calls
            .with_default_structured_response(
                json!({"decision_type": "LOCAL", "tool_name": "llm", "params": {}}),
            ),
    );

    let registry = Arc::new(AgentRegistry::new());
    let enabled_tools = Arc::new(vec!["echo".to_string(), "llm".to_string()]);
    let config = BidirectionalAgentConfig::default();

    let router = BidirectionalTaskRouter::new(llm, registry, enabled_tools, None, &config);

    let task = create_test_task("What is the capital of France?");
    let decision = router.decide(&task.into_send_params()).await.unwrap();

    match decision {
        RoutingDecision::Local { tool_name, params } => {
            assert_eq!(tool_name, "echo");
            assert_eq!(params.get("text").and_then(Value::as_str), Some("hello"));
        }
        other => panic!("Expected Local decision, got {:?}", other),
    }
}

#[tokio::test]
async fn test_router_remote_decision() {
    // Mock LLM response for DP2 (routing decision)
    let llm = Arc::new(
        MockLlmClient::new().with_default_structured_response(json!({
            "decision_type": "REMOTE",
            "agent_id": "test-agent"
        })),
    );

    let registry = Arc::new(AgentRegistry::new());

    let agent_card = create_test_agent_card("http://example.com/agent");
    registry.agents.insert(
        "test-agent".to_string(),
        CachedAgentInfo {
            card: agent_card.clone(),
            last_checked: Utc::now(),
        },
    );
    let enabled_tools = Arc::new(vec!["echo".to_string()]);
    let config = BidirectionalAgentConfig::default();

    let router = BidirectionalTaskRouter::new(llm, registry, enabled_tools, None, &config);

    let task = create_test_task("Please forward this to test-agent");
    let decision = router.decide(&task.into_send_params()).await.unwrap();

    assert!(matches!(decision, RoutingDecision::Remote { agent_id } if agent_id == "test-agent"));
}

#[tokio::test]
async fn test_router_fallback_to_local_for_unknown_agent() {
    // Mock LLM response for DP2: delegates to an unknown agent
    let llm = Arc::new(
        MockLlmClient::new().with_default_structured_response(json!({
            "decision_type": "REMOTE",
            "agent_id": "unknown-agent"
        })),
    );

    let registry = Arc::new(AgentRegistry::new()); // Empty registry
    let enabled_tools = Arc::new(vec!["echo".to_string(), "llm".to_string()]);
    let config = BidirectionalAgentConfig::default();

    let router = BidirectionalTaskRouter::new(llm, registry, enabled_tools, None, &config);

    let task = create_test_task("Please forward this to unknown-agent");
    let decision = router.decide(&task.into_send_params()).await.unwrap();

    // Fallback should be Local with 'llm' tool
    match decision {
        RoutingDecision::Local {
            tool_name,
            params: _,
        } => {
            assert_eq!(tool_name, "llm");
        }
        other => panic!("Expected Local decision, got {:?}", other),
    }
}

#[tokio::test]
async fn test_router_fallback_to_local_for_unclear_decision() {
    // Mock LLM response for DP2: unclear decision_type
    let llm = Arc::new(
        MockLlmClient::new().with_default_structured_response(json!({
            "decision_type": "MAYBE_REMOTE_OR_LOCAL", // Invalid enum value
            "agent_id": "some-agent"
        })),
    );

    let registry = Arc::new(AgentRegistry::new());
    let enabled_tools = Arc::new(vec!["echo".to_string(), "llm".to_string()]);
    let config = BidirectionalAgentConfig::default();

    let router = BidirectionalTaskRouter::new(llm, registry, enabled_tools, None, &config);

    let task = create_test_task("What should I do with this?");
    let decision = router.decide(&task.into_send_params()).await.unwrap();

    // Fallback should be Local with 'llm' tool
    match decision {
        RoutingDecision::Local {
            tool_name,
            params: _,
        } => {
            assert_eq!(tool_name, "llm");
        }
        other => panic!("Expected Local decision, got {:?}", other),
    }
}

#[tokio::test]
async fn test_router_prompt_formatting() {
    // Create a mock LLM client to capture calls
    let llm = Arc::new(MockLlmClient::new()); // Uses default structured response

    let registry = Arc::new(AgentRegistry::new());

    let agent_card1 = create_test_agent_card("http://example.com/agent1");
    let agent_card2 = create_test_agent_card("http://example.com/agent2");
    registry.agents.insert(
        "test-agent-1".to_string(),
        CachedAgentInfo {
            card: agent_card1.clone(),
            last_checked: Utc::now(),
        },
    );
    registry.agents.insert(
        "test-agent-2".to_string(),
        CachedAgentInfo {
            card: agent_card2.clone(),
            last_checked: Utc::now(),
        },
    );
    let enabled_tools = Arc::new(vec!["echo".to_string(), "llm".to_string()]);
    let config = BidirectionalAgentConfig::default();

    let router = BidirectionalTaskRouter::new(llm.clone(), registry, enabled_tools, None, &config);

    let task = create_test_task("Please route this task appropriately");

    // The default mock response is LOCAL, then tool choice 'llm'.
    let _ = router.decide(&task.into_send_params()).await.unwrap();

    let calls = llm.calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        2,
        "Expected two LLM calls (routing + tool choice if local)"
    );

    // Check the first prompt (routing - DP2)
    let (routing_prompt_text, routing_schema) = &calls[0];
    assert!(routing_prompt_text.contains("decide the best course of action"));
    assert!(routing_prompt_text.contains("Please route this task appropriately"));
    assert!(routing_prompt_text.contains("test-agent-1")); // Check if agent info is in prompt
    assert!(routing_prompt_text.contains("test-agent-2"));
    assert!(routing_schema.is_some());
    assert_eq!(
        routing_schema
            .as_ref()
            .unwrap()
            .get("properties")
            .unwrap()
            .get("decision_type")
            .unwrap()
            .get("enum")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        3
    ); // LOCAL, REMOTE, REJECT

    // Check the second prompt (tool choice - DP3, because default mock is LOCAL)
    let (tool_choice_prompt_text, tool_choice_schema) = &calls[1];
    assert!(tool_choice_prompt_text.contains("choose the SINGLE most appropriate tool"));
    assert!(tool_choice_prompt_text.contains("AVAILABLE LOCAL TOOLS:"));
    assert!(tool_choice_schema.is_some());
    assert!(tool_choice_schema
        .as_ref()
        .unwrap()
        .get("properties")
        .unwrap()
        .get("tool_name")
        .is_some());
    assert!(tool_choice_schema
        .as_ref()
        .unwrap()
        .get("properties")
        .unwrap()
        .get("params")
        .is_some());
}

#[tokio::test]
async fn test_router_needs_clarification() {
    // Mock LLM response for NP1 (clarification check)
    let llm = Arc::new(
        MockLlmClient::new()
            .with_structured_response(
                "judge whether the request is specific and complete",
                json!({
                    "clarity": "NEEDS_CLARIFY",
                    "question": "What specific topic are you asking about?"
                }),
            )
            .with_default_structured_response(json!({"clarity": "CLEAR"})), // Default if no specific match
    );

    let registry = Arc::new(AgentRegistry::new());
    let enabled_tools = Arc::new(vec!["echo".to_string(), "llm".to_string()]);
    let mut config = BidirectionalAgentConfig::default();
    config.mode.experimental_clarification = true; // Enable clarification

    let router = BidirectionalTaskRouter::new(llm.clone(), registry, enabled_tools, None, &config);

    let task = create_test_task("Tell me about it.");
    let decision = router.decide(&task.into_send_params()).await.unwrap();

    match decision {
        RoutingDecision::NeedsClarification { question } => {
            assert_eq!(question, "What specific topic are you asking about?");
        }
        other => panic!("Expected NeedsClarification decision, got {:?}", other),
    }

    let calls = llm.calls.lock().unwrap();
    assert_eq!(
        calls.len(),
        1,
        "Expected only one LLM call for clarification check"
    );
    let (prompt_text, schema) = &calls[0];
    assert!(prompt_text.contains("judge whether the request is specific and complete"));
    assert!(prompt_text.contains("Tell me about it."));
    assert!(schema.is_some());
    assert_eq!(
        schema
            .as_ref()
            .unwrap()
            .get("properties")
            .unwrap()
            .get("clarity")
            .unwrap()
            .get("enum")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    ); // CLEAR, NEEDS_CLARIFY
}

// Helper function to create a test task
fn create_test_task(message_text: &str) -> Task {
    let task_id = Uuid::new_v4().to_string();
    let initial_message = Message {
        role: Role::User,
        parts: vec![Part::TextPart(TextPart {
            text: message_text.to_string(),
            metadata: None,
            type_: "text".to_string(),
        })],
        metadata: None,
    };

    Task {
        id: task_id.clone(),
        status: TaskStatus {
            state: TaskState::Submitted,
            timestamp: Some(Utc::now()),
            message: None,
        },
        history: Some(vec![initial_message]),
        artifacts: None,
        metadata: None,
        session_id: None,
    }
}

// Helper function to create a test agent card
fn create_test_agent_card(url: &str) -> AgentCard {
    AgentCard {
        name: "Test Agent".to_string(),
        description: Some("A test agent for routing decisions".to_string()),
        version: "1.0.0".to_string(),
        url: url.to_string(),
        capabilities: AgentCapabilities {
            push_notifications: true,
            state_transition_history: true,
            streaming: false,
        },
        authentication: None,
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        documentation_url: None,
        provider: None,
        skills: vec![AgentSkill {
            id: "echo".to_string(),
            name: "Echo Tool".to_string(),
            description: Some("Echoes back the input text".to_string()),
            examples: None,
            input_modes: Some(vec!["text".to_string()]),
            output_modes: Some(vec!["text".to_string()]),
            tags: Some(vec!["echo".to_string(), "text_manipulation".to_string()]),
        }],
    }
}
