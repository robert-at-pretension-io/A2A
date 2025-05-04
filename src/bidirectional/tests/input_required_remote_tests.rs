use crate::bidirectional::BidirectionalTaskRouter;
use crate::server::agent_registry::{AgentRegistry, CachedAgentInfo};
use crate::bidirectional::tests::mocks::MockLlmClient;
use crate::types::{Task, TaskStatus, TaskState, Message, Part, TextPart, Role, AgentCard, AgentCapabilities};
use crate::server::task_router::{RoutingDecision, LlmTaskRouterTrait}; // Import the trait
use crate::server::repositories::task_repository::{InMemoryTaskRepository, TaskRepository}; // Import TaskRepository trait
use std::sync::Arc;
use uuid::Uuid;
use chrono::Utc;
use serde_json::json;

// Tests for handling InputRequired state in returning remote tasks

// Add a mock task repository implementation for the router to use
struct MockTaskRouterWithTasks {
    tasks: Arc<std::sync::Mutex<std::collections::HashMap<String, Task>>>,
}

impl MockTaskRouterWithTasks {
    fn new() -> Self {
        Self {
            tasks: Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }
    
    // Add a task to the internal mock storage
    async fn add_task(&self, task: Task) {
        let mut tasks = self.tasks.lock().unwrap();
        tasks.insert(task.id.clone(), task);
    }
}

// Implement the TaskRepository trait for our mock
#[async_trait::async_trait]
impl TaskRepository for MockTaskRouterWithTasks {
    async fn get_task(&self, task_id: &str) -> Result<Option<Task>, crate::server::error::ServerError> {
        let tasks = self.tasks.lock().unwrap();
        Ok(tasks.get(task_id).cloned())
    }
    
    async fn save_task(&self, task: &Task) -> Result<(), crate::server::error::ServerError> {
        let mut tasks = self.tasks.lock().unwrap();
        tasks.insert(task.id.clone(), task.clone());
        Ok(())
    }
    
    async fn delete_task(&self, task_id: &str) -> Result<(), crate::server::error::ServerError> {
        let mut tasks = self.tasks.lock().unwrap();
        tasks.remove(task_id);
        Ok(())
    }
    
    async fn get_state_history(&self, _task_id: &str) -> Result<Vec<Task>, crate::server::error::ServerError> {
        Ok(Vec::new())
    }
    
    async fn save_state_history(&self, _task_id: &str, _task: &Task) -> Result<(), crate::server::error::ServerError> {
        // No-op implementation for tests
        Ok(())
    }

    async fn get_push_notification_config(&self, _task_id: &str) -> Result<Option<crate::types::PushNotificationConfig>, crate::server::error::ServerError> {
        // Return None for tests
        Ok(None)
    }
    
    async fn save_push_notification_config(&self, _task_id: &str, _config: &crate::types::PushNotificationConfig) -> Result<(), crate::server::error::ServerError> {
        // No-op implementation for tests
        Ok(())
    }
}

// Update the existing test to use our new mock
#[tokio::test]
async fn test_input_required_llm_decision_direct_handling() {
    // Create a mock LLM client that will return "HANDLE_DIRECTLY"
    let llm = Arc::new(MockLlmClient::new().with_default_response("HANDLE_DIRECTLY"));
    
    // Create our custom task repository
    let task_repository = Arc::new(MockTaskRouterWithTasks::new());
    
    // Create an agent registry
    let registry = Arc::new(AgentRegistry::new());
    
    // Provide a default list of enabled tools for the test
    let enabled_tools = Arc::new(vec!["llm".to_string(), "echo".to_string()]);

    // Create the router - now passing the task repository
    let router = BidirectionalTaskRouter::new(
        llm.clone(), 
        registry.clone(), 
        enabled_tools,
        Some(task_repository.clone()), // Add task repository
    );
    
    // Create a task that appears to be returning from another agent in InputRequired state
    let task_id = Uuid::new_v4().to_string();
    let task = Task {
        id: task_id.clone(),
        status: TaskStatus {
            state: TaskState::InputRequired,
            timestamp: Some(Utc::now()),
            message: Some(Message {
                role: Role::Agent,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "Please provide more details about X".to_string(),
                    metadata: None,
                })],
                metadata: None,
            }),
        },
        history: Some(vec![
            Message {
                role: Role::User,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "Tell me about X".to_string(),
                    metadata: None,
                })],
                metadata: None,
            },
            Message {
                role: Role::Agent,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "I need more information to answer that".to_string(),
                    metadata: None,
                })],
                metadata: None,
            }
        ]),
        artifacts: None,
        metadata: Some(json!({
            "delegated_from": "agent-1",
            "source_agent_id": "agent-1"
        }).as_object().unwrap().clone()),
        session_id: None,
    };
    
    // Add the task to our mock task repository
    task_repository.add_task(task).await;
    
    // Create a follow-up message
    let follow_up_message = Message {
        role: Role::User,
        parts: vec![Part::TextPart(TextPart {
            type_: "text".to_string(),
            text: "X is a programming language".to_string(),
            metadata: None,
        })],
        metadata: None,
    };
    
    // Process the follow-up message
    let decision = router.process_follow_up(&task_id, &follow_up_message).await.unwrap();
    
    // Check that the decision is to handle directly with llm tool
    match decision {
        RoutingDecision::Local { tool_name, params } => {
            assert_eq!(tool_name, "llm", "Should use llm tool for direct handling");
            assert!(params.get("text").is_some(), "Should include text parameter");
            let text = params.get("text").and_then(|v| v.as_str()).unwrap();
            assert_eq!(text, "X is a programming language", "Should pass follow-up text to llm");
        },
        _ => panic!("Expected Local decision with llm tool"),
    }
}

#[tokio::test]
async fn test_input_required_llm_decision_human_input_needed() {
    // Create a mock LLM client that will return "NEED_HUMAN_INPUT"
    let llm = Arc::new(MockLlmClient::new().with_default_response("NEED_HUMAN_INPUT"));
    
    // Create our custom task repository
    let task_repository = Arc::new(MockTaskRouterWithTasks::new());
    
    // Create an agent registry
    let registry = Arc::new(AgentRegistry::new());
    
    // Provide a default list of enabled tools for the test
    let enabled_tools = Arc::new(vec!["llm".to_string(), "echo".to_string()]);

    // Create the router - now passing the task repository
    let router = BidirectionalTaskRouter::new(
        llm.clone(), 
        registry.clone(), 
        enabled_tools,
        Some(task_repository.clone()), // Add task repository
    );
    
    // Create a task that appears to be returning from another agent in InputRequired state
    let task_id = Uuid::new_v4().to_string();
    let task = Task {
        id: task_id.clone(),
        status: TaskStatus {
            state: TaskState::InputRequired,
            timestamp: Some(Utc::now()),
            message: Some(Message {
                role: Role::Agent,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "Please provide your personal preference for X".to_string(),
                    metadata: None,
                })],
                metadata: None,
            }),
        },
        history: Some(vec![
            Message {
                role: Role::User,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "What's the best approach for X?".to_string(),
                    metadata: None,
                })],
                metadata: None,
            },
            Message {
                role: Role::Agent,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "I need to know your personal preferences".to_string(),
                    metadata: None,
                })],
                metadata: None,
            }
        ]),
        artifacts: None,
        metadata: Some(json!({
            "remote_agent_id": "agent-2",
        }).as_object().unwrap().clone()),
        session_id: None,
    };
    
    // Add the task to our mock task repository
    task_repository.add_task(task).await;
    
    // Create a follow-up message
    let follow_up_message = Message {
        role: Role::User,
        parts: vec![Part::TextPart(TextPart {
            type_: "text".to_string(),
            text: "I prefer simple solutions".to_string(),
            metadata: None,
        })],
        metadata: None,
    };
    
    // Process the follow-up message
    let decision = router.process_follow_up(&task_id, &follow_up_message).await.unwrap();
    
    // Check that the decision is to get human input
    match decision {
        RoutingDecision::Local { tool_name, params } => {
            assert_eq!(tool_name, "human_input", "Should use human_input tool");
            assert!(params.get("text").is_some(), "Should include text parameter");
            let text = params.get("text").and_then(|v| v.as_str()).unwrap();
            assert_eq!(text, "I prefer simple solutions", "Should pass follow-up text");
            
            // Check that require_human_input flag is set
            assert!(params.get("require_human_input").is_some(), "Should include require_human_input parameter");
            assert!(params.get("require_human_input").and_then(|v| v.as_bool()).unwrap_or(false), 
                   "require_human_input should be true");
            
            // Check that prompt is included
            assert!(params.get("prompt").is_some(), "Should include prompt parameter");
            let prompt = params.get("prompt").and_then(|v| v.as_str()).unwrap();
            assert!(prompt.contains("Please provide your personal preference"), 
                   "Prompt should contain the original request for input");
        },
        _ => panic!("Expected Local decision with human_input tool"),
    }
}

#[tokio::test]
async fn test_input_required_non_remote_task_default_handling() {
    // Create a mock LLM client 
    let llm = Arc::new(MockLlmClient::new().with_default_response("WHATEVER"));
    
    // Create our custom task repository
    let task_repository = Arc::new(MockTaskRouterWithTasks::new());
    
    // Create an agent registry
    let registry = Arc::new(AgentRegistry::new());
    
    // Provide a default list of enabled tools for the test
    let enabled_tools = Arc::new(vec!["llm".to_string(), "echo".to_string()]);

    // Create the router - now passing the task repository
    let router = BidirectionalTaskRouter::new(
        llm.clone(), 
        registry.clone(), 
        enabled_tools,
        Some(task_repository.clone()), // Add task repository
    );
    
    // Create a task that is in InputRequired state but NOT returning from another agent
    // (i.e., no delegated_from or remote_agent_id metadata)
    let task_id = Uuid::new_v4().to_string();
    let task = Task {
        id: task_id.clone(),
        status: TaskStatus {
            state: TaskState::InputRequired,
            timestamp: Some(Utc::now()),
            message: Some(Message {
                role: Role::Agent,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "Please provide more information".to_string(),
                    metadata: None,
                })],
                metadata: None,
            }),
        },
        history: Some(vec![
            Message {
                role: Role::User,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "Help me with something".to_string(),
                    metadata: None,
                })],
                metadata: None,
            }
        ]),
        artifacts: None,
        metadata: None, // No metadata indicating a remote or delegated task
        session_id: None,
    };
    
    // Add the task to our mock task repository
    task_repository.add_task(task).await;
    
    // Create a follow-up message
    let follow_up_message = Message {
        role: Role::User,
        parts: vec![Part::TextPart(TextPart {
            type_: "text".to_string(),
            text: "Here's more information".to_string(),
            metadata: None,
        })],
        metadata: None,
    };
    
    // Process the follow-up message
    let decision = router.process_follow_up(&task_id, &follow_up_message).await.unwrap();
    
    // For non-remote tasks, it should default to using the llm tool
    match decision {
        RoutingDecision::Local { tool_name, params } => {
            assert_eq!(tool_name, "llm", "Should use llm tool for regular tasks");
            assert!(params.get("text").is_some(), "Should include text parameter");
            let text = params.get("text").and_then(|v| v.as_str()).unwrap();
            assert_eq!(text, "Here's more information", "Should pass follow-up text to llm");
        },
        _ => panic!("Expected Local decision with llm tool"),
    }
}

// Add the new test module to the test registry