use crate::bidirectional::bidirectional_agent::{BidirectionalTaskRouter, AgentDirectory, LlmClient};
use crate::server::task_router::RoutingDecision;
use crate::types::{Task, TaskState, Message, Part, TextPart, Role, TaskStatus, AgentCard};
use std::sync::Arc;
use async_trait::async_trait;
use chrono::Utc;
use futures::lock::Mutex;
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
        RoutingDecision::Local { tool_names } => {
            assert!(!tool_names.is_empty(), "Tool names should not be empty");
            // Default to echo when tool choice is handled by the router
            assert_eq!(tool_names[0], "echo", "Expected echo tool to be selected");
        },
        other => panic!("Expected Local decision, got {:?}", other),
    }
}

#[tokio::test]
async fn test_task_remote_delegation() {
    // Create a task with content that should be delegated
    let task = create_test_task("Analyze this complex data set for statistical patterns");

    // Create a directory with a data analysis agent
    let directory = Arc::new(AgentDirectory::new());
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
    directory.add_or_update_agent("data-agent".to_string(), agent_card);

    // Create a router that will delegate to the data agent
    let router = create_test_router_with_directory_and_response(
        directory,
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
    let directory = Arc::new(AgentDirectory::new());
    let llm = Arc::new(MockLlmClient::new(response.to_string()));
    let enabled_tools = Arc::new(vec!["echo".to_string(), "llm".to_string()]);
    
    BidirectionalTaskRouter::new(
        llm,
        directory,
        enabled_tools,
    )
}

// Helper to create a router with custom directory and response
fn create_test_router_with_directory_and_response(
    directory: Arc<AgentDirectory>,
    response: String,
) -> BidirectionalTaskRouter {
    let llm = Arc::new(MockLlmClient::new(response));
    let enabled_tools = Arc::new(vec!["echo".to_string(), "llm".to_string()]);
    
    BidirectionalTaskRouter::new(
        llm,
        directory,
        enabled_tools,
    )
}