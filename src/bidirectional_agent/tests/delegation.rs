//! Tests for Slice 3: Task Delegation & Synthesis.

#![cfg(feature = "bidir-delegate")]

use crate::bidirectional_agent::{
    config::BidirectionalAgentConfig,
    agent_registry::{AgentRegistry, CachedAgentInfo},
    client_manager::ClientManager,
    tool_executor::ToolExecutor,
    task_router::{TaskRouter, RoutingDecision},
    task_flow::TaskFlow,
    BidirectionalAgent,
};
use crate::server::repositories::task_repository::{InMemoryTaskRepository, TaskRepository, TaskRepositoryExt};
use crate::types::{Task, TaskState, TaskStatus, Message, Role, Part, TextPart, TaskSendParams, AgentCard, AgentCapabilities, AgentSkill};
use std::sync::Arc;
use mockito::Server;
use serde_json::json;

// Helper to create a basic task for testing
async fn create_test_task(repo: &Arc<InMemoryTaskRepository>, id: &str, text: &str) -> Task {
    let task = Task {
        id: id.to_string(),
        session_id: Some(format!("session-{}", id)),
        status: TaskStatus { state: TaskState::Submitted, timestamp: Some(chrono::Utc::now()), message: None },
        artifacts: None,
        history: Some(vec![Message { // Add initial message to history
            role: Role::User,
            parts: vec![Part::TextPart(TextPart { type_: "text".to_string(), text: text.to_string(), metadata: None })],
            metadata: None,
        }]),
        metadata: None,
    };
    repo.save_task(&task).await.unwrap();
    repo.save_state_history(&task.id, &task).await.unwrap(); // Save initial state
    task
}

// Helper to create a mock agent card
fn create_mock_agent_card(name: &str, url: &str) -> AgentCard {
     AgentCard {
        name: name.to_string(), description: Some(format!("Mock agent {}", name)), url: url.to_string(),
        provider: None, version: "1.0".to_string(), documentation_url: None,
        capabilities: AgentCapabilities { streaming: true, push_notifications: false, state_transition_history: false },
        authentication: None, default_input_modes: vec!["text/plain".to_string()], default_output_modes: vec!["text/plain".to_string()],
        skills: vec![AgentSkill { id: "remote-skill".to_string(), name: "Remote Skill".to_string(), description: None, tags: None, examples: None, input_modes: None, output_modes: None }],
    }
}

#[tokio::test]
async fn test_task_delegation_flow() {
    // --- Arrange ---
    let mut remote_server = Server::new_async().await;
    let remote_agent_id = "remote-agent-1";
    let remote_agent_url = remote_server.url();

    // Mock the remote agent's Agent Card endpoint
    let mock_card = create_mock_agent_card(remote_agent_id, &remote_agent_url);
    let _m_card = remote_server.mock("GET", "/.well-known/agent.json")
        .with_status(200)
        .with_body(serde_json::to_string(&mock_card).unwrap())
        .create_async().await;

    // Mock the remote agent's tasks/send endpoint to return a completed task
    let remote_task_id = "remote-task-123";
    let mock_remote_response = json!({
        "jsonrpc": "2.0",
        "id": 1, // Assuming client manager uses sequential IDs
        "result": {
            "id": remote_task_id,
            "sessionId": "remote-session",
            "status": { "state": "completed", "timestamp": chrono::Utc::now().to_rfc3339() },
            "artifacts": [{ "parts": [{"type": "text", "text": "Result from remote agent"}], "index": 0 }]
        }
    });
     let _m_send = remote_server.mock("POST", "/")
        .match_body(mockito::Matcher::PartialJson(json!({"method": "tasks/send"})))
        .with_status(200)
        .with_body(mock_remote_response.to_string())
        .create_async().await;

     // Mock the remote agent's tasks/get endpoint for polling
     let mock_remote_get_response = json!({
         "jsonrpc": "2.0",
         "id": 2, // Assuming polling uses next ID
         "result": {
             "id": remote_task_id,
             "sessionId": "remote-session",
             "status": { "state": "completed", "timestamp": chrono::Utc::now().to_rfc3339() },
             "artifacts": [{ "parts": [{"type": "text", "text": "Result from remote agent"}], "index": 0 }]
         }
     });
      let _m_get = remote_server.mock("POST", "/")
         .match_body(mockito::Matcher::PartialJson(json!({"method": "tasks/get", "params": {"id": remote_task_id}})))
         .with_status(200)
         .with_body(mock_remote_get_response.to_string())
         .create_async().await;


    // Setup local agent components
    let repo = Arc::new(InMemoryTaskRepository::new());
    let registry = Arc::new(AgentRegistry::new());
    let config = Arc::new(BidirectionalAgentConfig::default()); // Use default config
    let client_manager = Arc::new(ClientManager::new(registry.clone(), config.clone()).unwrap());
    let tool_executor = Arc::new(ToolExecutor::new()); // Not used directly, but needed
    let task_router = Arc::new(TaskRouter::new(registry.clone(), tool_executor.clone()));

    // Discover the remote agent
    registry.discover(&remote_agent_url).await.unwrap();

    // Create a local task
    let local_task_id = "local-task-delegate";
    let task = create_test_task(&repo, local_task_id, "Delegate this task").await;

    // --- Act ---
    // Manually create a Remote routing decision
    let decision = RoutingDecision::Remote { agent_id: remote_agent_id.to_string() };

    // Create and run the task flow
    let flow = TaskFlow::new(
        local_task_id.to_string(),
        "local-agent".to_string(), // Self ID
        repo.clone(),
        client_manager.clone(),
        tool_executor.clone(),
        registry.clone(),
    );
    let flow_result = flow.process_decision(decision).await;

    // --- Assert ---
    assert!(flow_result.is_ok(), "Task flow processing failed: {:?}", flow_result.err());

    // Verify the local task state is updated to Completed (after polling)
    let final_task = repo.get_task(local_task_id).await.unwrap().unwrap();
    assert_eq!(final_task.status.state, TaskState::Completed, "Local task should be Completed");

    // Verify artifacts were copied from remote
    assert!(final_task.artifacts.is_some());
    let artifacts = final_task.artifacts.unwrap();
    assert_eq!(artifacts.len(), 1);
    assert!(artifacts[0].parts.iter().any(|p| matches!(p, Part::TextPart(tp) if tp.text == "Result from remote agent")));

    // Verify task origin was set (requires TaskRepositoryExt)
    // let origin = repo.get_task_origin(local_task_id).await.unwrap();
    // assert!(matches!(origin, Some(TaskOrigin::Delegated { agent_id, remote_task_id: r_id })
    //     if agent_id == remote_agent_id && r_id == remote_task_id));
     println!("Note: Task origin verification requires TaskRepositoryExt implementation.");
}

// Add more tests:
// - test_delegation_failure_updates_local_task
// - test_polling_timeout_updates_local_task
// - test_decomposition_flow (when implemented)
// - test_result_synthesis (when implemented)
