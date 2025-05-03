// Import RoutingDecision from the correct path
use crate::server::task_router::RoutingDecision;
use crate::bidirectional::bidirectional_agent::{BidirectionalTaskRouter, AgentDirectory}; // Remove ExecutionMode import
use crate::bidirectional::tests::mocks::MockLlmClient;
use crate::types::{Task, TaskStatus, TaskState, Message, Part, TextPart, Role, AgentCard, AgentCapabilities};
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;

#[tokio::test]
async fn test_router_local_decision() {
    // Create a mock LLM client that always returns "LOCAL"
    let llm = Arc::new(MockLlmClient::new().with_default_response("LOCAL"));
    
    // Create an agent directory
    let directory = Arc::new(AgentDirectory::new());
    // Provide a default list of enabled tools for the test
    let enabled_tools = Arc::new(vec!["echo".to_string(), "llm".to_string()]);

    // Create the router
    let router = BidirectionalTaskRouter::new(llm, directory, enabled_tools);

    // Create a test task
    let task = create_test_task("What is the capital of France?");
    
    // Test the routing decision
    let decision = router.decide_execution_mode(&task).await.unwrap();

    // Verify that the decision is to execute locally (RoutingDecision::Local)
    // We also check that a tool was selected (e.g., "echo" as fallback)
    assert!(matches!(decision, RoutingDecision::Local { tool_names } if !tool_names.is_empty()));
}

#[tokio::test]
async fn test_router_remote_decision() {
    // Create a mock LLM client that returns a remote decision
    let llm = Arc::new(MockLlmClient::new().with_default_response("REMOTE: test-agent"));
    
    // Create an agent directory
    let directory = Arc::new(AgentDirectory::new());
    
    // Add a test agent to the directory
    let agent_card = create_test_agent_card("http://example.com/agent");
    directory.add_or_update_agent("test-agent".to_string(), agent_card);
    // Provide a default list of enabled tools for the test
    let enabled_tools = Arc::new(vec!["echo".to_string()]);

    // Create the router
    let router = BidirectionalTaskRouter::new(llm, directory, enabled_tools);

    // Create a test task
    let task = create_test_task("Please forward this to test-agent");
    
    // Test the routing decision
    let decision = router.decide_execution_mode(&task).await.unwrap();

    // Verify that the decision is to execute remotely with the correct agent ID (RoutingDecision::Remote)
    assert!(matches!(decision, RoutingDecision::Remote { agent_id } if agent_id == "test-agent"));
}

#[tokio::test]
async fn test_router_fallback_to_local_for_unknown_agent() {
    // Create a mock LLM client that returns a remote decision for an unknown agent
    let llm = Arc::new(MockLlmClient::new().with_default_response("REMOTE: unknown-agent"));
    
    // Create an agent directory (with no agents)
    let directory = Arc::new(AgentDirectory::new());
    // Provide a default list of enabled tools for the test
    let enabled_tools = Arc::new(vec!["echo".to_string()]);

    // Create the router
    let router = BidirectionalTaskRouter::new(llm, directory, enabled_tools);

    // Create a test task
    let task = create_test_task("Please forward this to unknown-agent");
    
    // Test the routing decision
    let decision = router.decide_execution_mode(&task).await.unwrap();

    // Verify that the decision falls back to local execution (RoutingDecision::Local)
    // It should default to the 'echo' tool in this case.
    assert!(matches!(decision, RoutingDecision::Local { tool_names } if tool_names == vec!["echo".to_string()]));
}

#[tokio::test]
async fn test_router_fallback_to_local_for_unclear_decision() {
    // Create a mock LLM client that returns an unclear decision
    let llm = Arc::new(MockLlmClient::new().with_default_response("I'm not sure"));
    
    // Create an agent directory
    let directory = Arc::new(AgentDirectory::new());
    // Provide a default list of enabled tools for the test
    let enabled_tools = Arc::new(vec!["echo".to_string()]);

    // Create the router
    let router = BidirectionalTaskRouter::new(llm, directory, enabled_tools);

    // Create a test task
    let task = create_test_task("What should I do with this?");
    
    // Test the routing decision
    let decision = router.decide_execution_mode(&task).await.unwrap();

    // Verify that the decision falls back to local execution (RoutingDecision::Local)
    // It should default to the 'echo' tool in this case.
    assert!(matches!(decision, RoutingDecision::Local { tool_names } if tool_names == vec!["echo".to_string()]));
}

#[tokio::test]
async fn test_router_prompt_formatting() {
    // Create a mock LLM client to capture the prompt
    let llm = Arc::new(MockLlmClient::new());
    
    // Create an agent directory with some test agents
    let directory = Arc::new(AgentDirectory::new());
    
    // Add test agents to the directory
    let agent_card1 = create_test_agent_card("http://example.com/agent1");
    let agent_card2 = create_test_agent_card("http://example.com/agent2");
    directory.add_or_update_agent("test-agent-1".to_string(), agent_card1);
    directory.add_or_update_agent("test-agent-2".to_string(), agent_card2);
    // Provide a default list of enabled tools for the test
    let enabled_tools = Arc::new(vec!["echo".to_string(), "llm".to_string()]);

    // Create the router
    let router = BidirectionalTaskRouter::new(llm.clone(), directory, enabled_tools);

    // Create a test task
    let task = create_test_task("Please route this task appropriately");
    
    // Make the routing decision
    let _ = router.decide_execution_mode(&task).await.unwrap();
    
    // Check that the prompt sent to the LLM contains key elements
    // The router now makes TWO calls: one for routing, one for tool choice if local.
    let calls = llm.calls.lock().unwrap();
    assert_eq!(calls.len(), 2, "Expected two LLM calls (routing + tool choice)");
    
    // Check the first prompt (routing)
    let routing_prompt = &calls[0];
    assert!(routing_prompt.contains("You need to decide whether to handle a task locally or delegate it to another agent"));
    assert!(routing_prompt.contains("Please route this task appropriately"));
    assert!(routing_prompt.contains("test-agent-1"));
    assert!(routing_prompt.contains("test-agent-2"));
    assert!(routing_prompt.contains("LOCAL"));
    assert!(routing_prompt.contains("REMOTE"));

    // Check the second prompt (tool choice)
    let tool_choice_prompt = &calls[1];
    assert!(tool_choice_prompt.contains("You have the following local tools available"));
    assert!(tool_choice_prompt.contains("echo, llm")); // Check enabled tools list
    assert!(tool_choice_prompt.contains("choose the single best tool"));
    assert!(prompt.contains("You need to decide whether to handle a task locally or delegate it to another agent"));
    assert!(prompt.contains("Please route this task appropriately"));
    assert!(prompt.contains("test-agent-1"));
    assert!(prompt.contains("test-agent-2"));
    assert!(prompt.contains("LOCAL"));
    assert!(prompt.contains("REMOTE"));
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
        skills: vec![],
    }
}
