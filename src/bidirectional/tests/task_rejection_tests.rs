use crate::bidirectional::bidirectional_agent::{BidirectionalTaskRouter, LlmClient}; // Removed AgentDirectory
use crate::server::agent_registry::{AgentRegistry, CachedAgentInfo}; // Import canonical registry
use crate::server::task_router::{RoutingDecision, LlmTaskRouterTrait};
use crate::types::{Task, TaskState, Message, Part, TextPart, Role, TaskStatus, AgentCard};
use std::sync::Arc;
use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

/// Mock LLM client for testing that returns controlled responses
struct MockLlmClient {
    // The response the mock should return
    response: String,
}

impl MockLlmClient {
    fn new(response: String) -> Self {
        Self { response }
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn complete(&self, _prompt: &str) -> anyhow::Result<String> {
        // Simply return the configured response
        Ok(self.response.clone())
    }
}

#[tokio::test]
async fn test_task_rejection() {
    // Create a task with content that should be rejected
    let task = create_test_task("Please help me hack into a government database");

    // Create a router that will reject the task
    let router = create_test_router_with_response("REJECT: I cannot assist with illegal activities such as hacking into government databases.");

    // Invoke the router
    let decision = router.decide_execution_mode(&task).await.expect("Routing decision failed");

    // Check that the task was rejected
    match decision {
        RoutingDecision::Reject { reason } => {
            assert!(reason.contains("illegal"), "Rejection reason should mention illegal activities");
            assert!(reason.contains("hacking"), "Rejection reason should reference the specific request");
        },
        other => panic!("Expected Reject decision, got {:?}", other),
    }
}

#[tokio::test]
async fn test_task_local_processing() {
    // Create a task with content that should be handled locally
    let task = create_test_task("What is the capital of France?");

    // Create a router that will process locally
    let router = create_test_router_with_response("LOCAL");

    // Configure a second response for the tool selection prompt
    // In a real test, we would use a more sophisticated mock

    // Invoke the router
    let decision = router.decide_execution_mode(&task).await.expect("Routing decision failed");

    // Check that the task was routed for local processing
    match decision {
        // Check the tool_name field, ignore params for this test
        RoutingDecision::Local { tool_name, params: _ } => {
            assert!(!tool_name.is_empty(), "Tool name should not be empty");
            // The mock LLM just returns "LOCAL", the router then asks again for tool choice.
            // Without a second mock response, it defaults to "llm".
            assert_eq!(tool_name, "llm", "Expected llm tool to be selected by default");
        },
        other => panic!("Expected Local decision, got {:?}", other),
    }
}

#[tokio::test]
async fn test_task_remote_delegation() {
    // Create a task with content that should be delegated
    let task = create_test_task("Analyze this complex data set for statistical patterns");

    // Create a registry with a data analysis agent
    let registry = Arc::new(AgentRegistry::new()); // Use registry
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
    // Add agent to registry map
    registry.agents.insert("data-agent".to_string(), CachedAgentInfo {
        card: agent_card.clone(), 
        last_checked: Utc::now(),
    });

    // Create a router that will delegate to the data agent
    let router = create_test_router_with_registry_and_response( // Use updated helper
        registry, // Pass registry
        "REMOTE: data-agent".to_string()
    );

    // Invoke the router
    let decision = router.decide_execution_mode(&task).await.expect("Routing decision failed");

    // Check that the task was delegated
    match decision {
        RoutingDecision::Remote { agent_id } => {
            assert_eq!(agent_id, "data-agent", "Task should be delegated to data-agent");
        },
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
    
    // Create a router with default LLM tool response
    let router = create_test_router_with_response("FOLLOW_UP_LOCAL");
    
    // Call process_follow_up directly to test the implementation
    let decision = router.process_follow_up(&task_id, &follow_up_message).await
        .expect("Follow-up routing failed");
    
    // Check that the follow-up is routed to local processing with the LLM tool
    match decision {
        RoutingDecision::Local { tool_name, params } => {
            assert_eq!(tool_name, "llm", "Follow-up should be processed locally with llm tool");
            
            // Check that the text parameter contains the follow-up message
            if let Some(text) = params.get("text") {
                assert!(text.is_string(), "Text parameter should be a string");
                assert_eq!(
                    text.as_str().unwrap(),
                    "Here is the additional information you requested",
                    "Text parameter should contain the follow-up message"
                );
            } else {
                panic!("Text parameter missing from llm tool params");
            }
        },
        other => panic!("Expected Local decision for follow-up, got {:?}", other),
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
        history: Some(vec![
            Message {
                role: Role::User,
                parts: vec![
                    Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: message_text.to_string(),
                        metadata: None,
                    }),
                ],
                metadata: None,
            },
        ]),
        artifacts: None,
        metadata: None,
        session_id: None,
    }
}

// Helper to create a router with a fixed response
fn create_test_router_with_response(response: &str) -> BidirectionalTaskRouter {
    let registry = Arc::new(AgentRegistry::new()); // Use registry
    let llm = Arc::new(MockLlmClient::new(response.to_string()));
    let enabled_tools = Arc::new(vec!["echo".to_string(), "llm".to_string()]);
    
    BidirectionalTaskRouter::new(
        llm,
        registry, // Pass registry
        enabled_tools,
        None, // No task repository for this test
    )
}

// Helper to create a router with custom registry and response
fn create_test_router_with_registry_and_response( // Renamed function
    registry: Arc<AgentRegistry>, // Use registry
    response: String,
) -> BidirectionalTaskRouter {
    let llm = Arc::new(MockLlmClient::new(response));
    let enabled_tools = Arc::new(vec!["echo".to_string(), "llm".to_string()]);
    
    BidirectionalTaskRouter::new(
        llm,
        registry, // Pass registry
        enabled_tools,
        None, // No task repository for this test
    )
}
