use crate::bidirectional::config::{
    BidirectionalAgentConfig, ClientConfig, LlmConfig, ModeConfig, ServerConfig,
};
use crate::bidirectional::tests::mocks::MockLlmClient;
use crate::bidirectional::BidirectionalAgent;
use crate::server::repositories::task_repository::{InMemoryTaskRepository, TaskRepository};
use crate::server::services::task_service::TaskService;
use crate::types::{Message, Part, Role, Task, TaskSendParams, TaskState, TaskStatus, TextPart};
use anyhow::Result;
use chrono::Utc;
use std::env;
use std::sync::Arc;
use uuid::Uuid;

// Helper function to create a mock task service for testing
fn create_mock_task_service() -> Arc<TaskService> {
    let task_repository = Arc::new(InMemoryTaskRepository::new());
    Arc::new(TaskService::standalone(task_repository))
}

// This struct will help us test the task service integration
struct TestBidirectionalAgentWithTaskService {
    task_service: Arc<TaskService>,
    current_session_id: Option<String>,
}

impl TestBidirectionalAgentWithTaskService {
    fn new(task_service: Arc<TaskService>) -> Self {
        Self {
            task_service,
            current_session_id: None,
        }
    }

    fn set_session_id(&mut self, session_id: Option<String>) {
        self.current_session_id = session_id;
    }

    // Implementation of process_message_directly that uses TaskService
    async fn process_message_directly(&self, message_text: &str) -> Result<String> {
        // Create a unique task ID
        let task_id = Uuid::new_v4().to_string();

        // Create the message
        let initial_message = Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                text: message_text.to_string(),
                metadata: None,
                type_: "text".to_string(),
            })],
            metadata: None,
        };

        // Create TaskSendParams
        let params = TaskSendParams {
            id: task_id.clone(),
            message: initial_message,
            session_id: self.current_session_id.clone(),
            metadata: None,
            history_length: None,
            push_notification: None,
        };

        // Use task_service to process the task
        let task = self.task_service.process_task(params).await?;

        // Extract response from task
        let response = self.extract_text_from_task(&task);

        Ok(response)
    }

    // Helper to extract text from task
    fn extract_text_from_task(&self, task: &Task) -> String {
        // First check the status message
        if let Some(ref message) = task.status.message {
            let text = message
                .parts
                .iter()
                .filter_map(|p| match p {
                    Part::TextPart(tp) => Some(tp.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

            if !text.is_empty() {
                return text;
            }
        }

        // Then check history if available
        if let Some(history) = &task.history {
            let agent_messages = history
                .iter()
                .filter(|m| m.role == Role::Agent)
                .flat_map(|m| m.parts.iter())
                .filter_map(|p| match p {
                    Part::TextPart(tp) => Some(tp.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

            if !agent_messages.is_empty() {
                return agent_messages;
            }
        }

        // Fallback
        "No response text available.".to_string()
    }
}

#[tokio::test]
async fn test_process_message_with_task_service() {
    // Create a mock task service
    let task_service = create_mock_task_service();

    // Create our test agent
    let agent = TestBidirectionalAgentWithTaskService::new(task_service);

    // Process a message
    let message = "Hello, world!";
    let response = agent.process_message_directly(message).await.unwrap();

    // The mock task service in standalone mode should return a standard response
    assert_eq!(response, "Task completed successfully.");
}

#[tokio::test]
async fn test_process_message_with_session() {
    // Create a mock task service
    let task_service = create_mock_task_service();

    // Create our test agent
    let mut agent = TestBidirectionalAgentWithTaskService::new(task_service);

    // Set a session ID
    let session_id = format!("session-{}", Uuid::new_v4());
    agent.set_session_id(Some(session_id.clone()));

    // Process a message
    let message = "Hello with session!";
    let response = agent.process_message_directly(message).await.unwrap();

    // The task should be processed with the session ID
    assert_eq!(response, "Task completed successfully.");

    // We would need to access task_repository to verify the session ID was used
    // This would require refactoring our test to expose task_repository
}

#[tokio::test]
async fn test_extract_text_from_task() {
    // Create a mock task service
    let task_service = create_mock_task_service();

    // Create our test agent
    let agent = TestBidirectionalAgentWithTaskService::new(task_service);

    // Create a test task with a status message
    let task_id = Uuid::new_v4().to_string();
    let task = Task {
        id: task_id.clone(),
        status: TaskStatus {
            state: TaskState::Completed,
            timestamp: Some(Utc::now()),
            message: Some(Message {
                role: Role::Agent,
                parts: vec![Part::TextPart(TextPart {
                    text: "Status message response".to_string(),
                    metadata: None,
                    type_: "text".to_string(),
                })],
                metadata: None,
            }),
        },
        history: None,
        artifacts: None,
        metadata: None,
        session_id: None,
    };

    // Extract text
    let text = agent.extract_text_from_task(&task);
    assert_eq!(text, "Status message response");

    // Create a test task with history but no status message
    let task_id = Uuid::new_v4().to_string();
    let task = Task {
        id: task_id.clone(),
        status: TaskStatus {
            state: TaskState::Completed,
            timestamp: Some(Utc::now()),
            message: None,
        },
        history: Some(vec![
            Message {
                role: Role::User,
                parts: vec![Part::TextPart(TextPart {
                    text: "User message".to_string(),
                    metadata: None,
                    type_: "text".to_string(),
                })],
                metadata: None,
            },
            Message {
                role: Role::Agent,
                parts: vec![Part::TextPart(TextPart {
                    text: "Agent response in history".to_string(),
                    metadata: None,
                    type_: "text".to_string(),
                })],
                metadata: None,
            },
        ]),
        artifacts: None,
        metadata: None,
        session_id: None,
    };

    // Extract text
    let text = agent.extract_text_from_task(&task);
    assert_eq!(text, "Agent response in history");

    // Create a test task with no text
    let task_id = Uuid::new_v4().to_string();
    let task = Task {
        id: task_id.clone(),
        status: TaskStatus {
            state: TaskState::Completed,
            timestamp: Some(Utc::now()),
            message: None,
        },
        history: None,
        artifacts: None,
        metadata: None,
        session_id: None,
    };

    // Extract text
    let text = agent.extract_text_from_task(&task);
    assert_eq!(text, "No response text available.");
}
