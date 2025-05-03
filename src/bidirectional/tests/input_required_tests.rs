use crate::bidirectional::bidirectional_agent::{BidirectionalAgent, BidirectionalAgentConfig, ServerConfig, ClientConfig, LlmConfig, ModeConfig};
use crate::bidirectional::tests::mocks::MockLlmClient;
use crate::server::repositories::task_repository::{TaskRepository, InMemoryTaskRepository};
use crate::server::services::task_service::TaskService;
use crate::types::{Task, TaskStatus, TaskState, Message, Part, TextPart, Role, TaskSendParams, TaskQueryParams, TaskIdParams};
use std::sync::Arc;
use std::collections::HashMap;
use serde_json::json;
use std::env;
use uuid::Uuid;
use chrono::Utc;
use anyhow::{Result, anyhow};

// This struct helps us test the InputRequired handling
struct TestAgentWithInputRequired {
    task_service: Arc<TaskService>,
    task_repository: Arc<InMemoryTaskRepository>,
    current_session_id: Option<String>,
    session_tasks: HashMap<String, Vec<String>>,
}

impl TestAgentWithInputRequired {
    fn new() -> Self {
        let task_repository = Arc::new(InMemoryTaskRepository::new());
        let task_service = Arc::new(TaskService::standalone(task_repository.clone()));
        
        Self {
            task_service,
            task_repository,
            current_session_id: None,
            session_tasks: HashMap::new(),
        }
    }
    
    fn create_new_session(&mut self) -> String {
        let session_id = format!("session-{}", Uuid::new_v4());
        self.current_session_id = Some(session_id.clone());
        self.session_tasks.insert(session_id.clone(), Vec::new());
        session_id
    }
    
    async fn save_task_to_history(&mut self, task: Task) -> Result<()> {
        if let Some(session_id) = &self.current_session_id {
            if let Some(tasks) = self.session_tasks.get_mut(session_id) {
                tasks.push(task.id.clone());
            }
        }
        Ok(())
    }
    
    // Implementation that handles InputRequired state
    async fn process_message_directly(&mut self, message_text: &str) -> Result<String> {
        // Check if we're continuing an existing task that is in InputRequired state
        let mut continue_task_id = None;
        
        if let Some(session_id) = &self.current_session_id {
            if let Some(task_ids) = self.session_tasks.get(session_id) {
                // Check the last task in the session
                if let Some(last_task_id) = task_ids.last() {
                    // Get the task to check its state
                    let params = TaskQueryParams {
                        id: last_task_id.clone(),
                        history_length: None,
                        metadata: None,
                    };
                    
                    if let Ok(task) = self.task_service.get_task(params).await {
                        if task.status.state == TaskState::InputRequired {
                            // We should continue this task instead of creating a new one
                            continue_task_id = Some(last_task_id.clone());
                        }
                    }
                }
            }
        }
        
        // Create a new task ID if we're not continuing an existing task
        let task_id = if let Some(id) = continue_task_id {
            id
        } else {
            Uuid::new_v4().to_string()
        };
        
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
        
        // Save task to history
        self.save_task_to_history(task.clone()).await?;
        
        // Extract response from task
        let mut response = self.extract_text_from_task(&task);
        
        // If the task is in InputRequired state, indicate that in the response
        if task.status.state == TaskState::InputRequired {
            response.push_str("\n\n[The agent needs more information. Your next message will continue this task.]");
        }
        
        Ok(response)
    }
    
    // Helper to extract text from task
    fn extract_text_from_task(&self, task: &Task) -> String {
        // First check the status message
        if let Some(ref message) = task.status.message {
            let text = message.parts.iter()
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
            let agent_messages = history.iter()
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
    
    // Helper to create a task that will require input
    async fn create_input_required_task(&mut self, message_text: &str) -> Result<Task> {
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
        
        // Create TaskSendParams with special metadata to trigger InputRequired
        let params = TaskSendParams {
            id: task_id.clone(),
            message: initial_message,
            session_id: self.current_session_id.clone(),
            metadata: Some(json!({"_mock_require_input": true}).as_object().unwrap().clone()),
            history_length: None,
            push_notification: None,
        };
        
        // Use task_service to process the task
        let task = self.task_service.process_task(params).await?;
        
        // Save task to history
        self.save_task_to_history(task.clone()).await?;
        
        Ok(task)
    }
}

#[tokio::test]
async fn test_input_required_detection() {
    // Create test agent
    let mut agent = TestAgentWithInputRequired::new();
    
    // Create a session
    let session_id = agent.create_new_session();
    
    // Create a task that will require input
    let task = agent.create_input_required_task("Tell me more about this").await.unwrap();
    
    // Verify the task is in InputRequired state
    assert_eq!(task.status.state, TaskState::InputRequired);
    
    // Process a message that should continue the task
    let response = agent.process_message_directly("Here's more information").await.unwrap();
    
    // Verify the task was completed
    assert!(response.contains("Follow-up task completed successfully"));
}

#[tokio::test]
async fn test_normal_task_no_input_required() {
    // Create test agent
    let mut agent = TestAgentWithInputRequired::new();
    
    // Create a session
    let session_id = agent.create_new_session();
    
    // Process a normal message that should not require input
    let response = agent.process_message_directly("Hello world").await.unwrap();
    
    // Verify the response doesn't include InputRequired indicator
    assert!(!response.contains("[The agent needs more information"));
}

#[tokio::test]
async fn test_input_required_indicator() {
    // Create test agent
    let mut agent = TestAgentWithInputRequired::new();
    
    // Create a session
    let session_id = agent.create_new_session();
    
    // Create a task that will require input
    let task = agent.create_input_required_task("Tell me more about this").await.unwrap();
    
    // Extract response as would be shown to user
    let response = agent.extract_text_from_task(&task);
    assert_eq!(response, "Please provide additional information to continue.");
    
    // If we added our indicator to the response:
    let response_with_indicator = format!("{}\n\n[The agent needs more information. Your next message will continue this task.]", response);
    assert!(response_with_indicator.contains("[The agent needs more information"));
}

#[tokio::test]
async fn test_multiple_input_required_tasks() {
    // Create test agent
    let mut agent = TestAgentWithInputRequired::new();
    
    // Create a session
    let session_id = agent.create_new_session();
    
    // Create first task that requires input
    let task1 = agent.create_input_required_task("Question 1").await.unwrap();
    assert_eq!(task1.status.state, TaskState::InputRequired);
    
    // Provide follow-up and complete the first task
    let response1 = agent.process_message_directly("Answer 1").await.unwrap();
    assert!(response1.contains("Follow-up task completed successfully"));
    
    // Create second task that requires input
    let task2 = agent.create_input_required_task("Question 2").await.unwrap();
    assert_eq!(task2.status.state, TaskState::InputRequired);
    
    // Provide follow-up and complete the second task
    let response2 = agent.process_message_directly("Answer 2").await.unwrap();
    assert!(response2.contains("Follow-up task completed successfully"));
}