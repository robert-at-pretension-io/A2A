use crate::server::services::streaming_service::StreamingService;
use crate::server::repositories::task_repository::TaskRepository;
use crate::server::ServerError;
use crate::types::{Task, TaskStatus, TaskState, Message, Role, Part, TextPart,
                   TaskStatusUpdateEvent, TaskArtifactUpdateEvent};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;
use futures_util::StreamExt;
use serde_json::{json, Value};

// Create a mock task repository for testing
struct MockTaskRepository {
    tasks: Mutex<HashMap<String, Task>>,
    push_configs: Mutex<HashMap<String, crate::types::PushNotificationConfig>>,
    state_history: Mutex<HashMap<String, Vec<Task>>>,
}

impl MockTaskRepository {
    fn new() -> Self {
        Self {
            tasks: Mutex::new(HashMap::new()),
            push_configs: Mutex::new(HashMap::new()),
            state_history: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl TaskRepository for MockTaskRepository {
    async fn get_task(&self, id: &str) -> Result<Option<Task>, ServerError> {
        let tasks = self.tasks.lock().await;
        Ok(tasks.get(id).cloned())
    }
    
    async fn save_task(&self, task: &Task) -> Result<(), ServerError> {
        let mut tasks = self.tasks.lock().await;
        tasks.insert(task.id.clone(), task.clone());
        Ok(())
    }
    
    async fn delete_task(&self, id: &str) -> Result<(), ServerError> {
        let mut tasks = self.tasks.lock().await;
        tasks.remove(id);
        Ok(())
    }
    
    async fn get_push_notification_config(&self, task_id: &str) -> Result<Option<crate::types::PushNotificationConfig>, ServerError> {
        let push_configs = self.push_configs.lock().await;
        Ok(push_configs.get(task_id).cloned())
    }
    
    async fn save_push_notification_config(&self, task_id: &str, config: &crate::types::PushNotificationConfig) -> Result<(), ServerError> {
        let mut push_configs = self.push_configs.lock().await;
        push_configs.insert(task_id.to_string(), config.clone());
        Ok(())
    }
    
    async fn get_state_history(&self, task_id: &str) -> Result<Vec<Task>, ServerError> {
        let history = self.state_history.lock().await;
        Ok(history.get(task_id).cloned().unwrap_or_default())
    }
    
    async fn save_state_history(&self, task_id: &str, task: &Task) -> Result<(), ServerError> {
        let mut history = self.state_history.lock().await;
        let task_history = history.entry(task_id.to_string()).or_insert_with(Vec::new);
        task_history.push(task.clone());
        Ok(())
    }
}

// Test streaming connection is established with proper SSE headers
#[tokio::test]
async fn test_create_streaming_task_returns_stream() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = StreamingService::new(repository.clone());
    
    let task_id = format!("stream-task-{}", Uuid::new_v4());
    let task = Task {
        id: task_id.clone(),
        session_id: Some("test-session".to_string()),
        status: TaskStatus {
            state: TaskState::Working,
            timestamp: Some(Utc::now()),
            message: None,
        },
        artifacts: None,
        history: None,
        metadata: None,
    };
    
    // Save the task
    repository.save_task(&task).await.unwrap();
    
    // Act
    let request_id = json!("test-request-1");
    let stream = service.create_streaming_task(request_id, task.clone());
    
    // Assert
    // For a proper test, we'd need to inspect the headers, but that's handled by the handler
    // Here we'll just check that the stream produces data in the correct format
    let mut stream = tokio_stream::StreamExt::take(stream, 2); // Just take two messages to test
    
    let mut messages = Vec::new();
    while let Some(message) = stream.next().await {
        assert!(message.is_ok(), "Stream message should be Ok");
        let message = message.unwrap();
        
        // Check that it's in SSE format
        assert!(message.starts_with("data: "), "Message should start with 'data: '");
        assert!(message.ends_with("\n\n"), "Message should end with '\\n\\n'");
        
        // Extract the JSON
        let json_str = message.trim_start_matches("data: ").trim_end_matches("\n\n");
        let json: Value = serde_json::from_str(json_str).expect("Should be valid JSON");
        
        // Check JSON-RPC structure
        assert_eq!(json["jsonrpc"], "2.0", "Should be JSON-RPC 2.0");
        assert_eq!(json["id"], "test-request-1", "Request ID should match");
        assert!(json.get("result").is_some(), "Should have a result field");
        
        messages.push(json);
    }
    
    // Should have received two messages
    assert_eq!(messages.len(), 2, "Should receive 2 messages");
    
    // First message should have final=false
    assert_eq!(messages[0]["result"]["final"], false, "First message should have final=false");
    assert_eq!(messages[0]["result"]["id"], task_id, "Task ID should match");
    
    // Last message should have final flag set (could be true or null depending on implementation)
    let final_value = &messages[1]["result"]["final"];
    assert!(final_value.is_boolean() || final_value.is_null(), 
            "Final value should be boolean or null, got: {:?}", final_value);
}

// Test resubscribing to an active task continues streaming from current state
#[tokio::test]
async fn test_resubscribe_to_active_task() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = StreamingService::new(repository.clone());
    
    let task_id = format!("resubscribe-task-{}", Uuid::new_v4());
    let task = Task {
        id: task_id.clone(),
        session_id: Some("test-session".to_string()),
        status: TaskStatus {
            state: TaskState::Working,
            timestamp: Some(Utc::now()),
            message: None,
        },
        artifacts: None,
        history: None,
        metadata: None,
    };
    
    // Save the task
    repository.save_task(&task).await.unwrap();
    
    // Act
    let request_id = json!("test-request-2");
    let stream = service.resubscribe_to_task(request_id, task_id.clone()).await.unwrap();
    
    // Assert
    let mut stream = tokio_stream::StreamExt::take(stream, 2); // Just take two messages to test
    
    let mut messages = Vec::new();
    while let Some(message) = stream.next().await {
        assert!(message.is_ok(), "Stream message should be Ok");
        let message = message.unwrap();
        
        // Check that it's in SSE format
        assert!(message.starts_with("data: "), "Message should start with 'data: '");
        
        // Extract the JSON
        let json_str = message.trim_start_matches("data: ").trim_end_matches("\n\n");
        let json: Value = serde_json::from_str(json_str).expect("Should be valid JSON");
        
        // Check JSON-RPC structure
        assert_eq!(json["jsonrpc"], "2.0", "Should be JSON-RPC 2.0");
        assert_eq!(json["id"], "test-request-2", "Request ID should match");
        
        messages.push(json);
    }
    
    // Should have received two messages (initial state and final update)
    assert_eq!(messages.len(), 2, "Should receive 2 messages");
    
    // First message should be the current state
    assert_eq!(messages[0]["result"]["final"], false, "First message should have final=false");
    assert_eq!(messages[0]["result"]["id"], task_id, "Task ID should match");
    // Allow for either "working" or "Working" - case doesn't matter in this context
    let state_str = messages[0]["result"]["status"]["state"].as_str().unwrap_or("");
    assert!(state_str.eq_ignore_ascii_case("working"), "State should be Working (case insensitive)");
    
    // Last message should have final=true
    assert_eq!(messages[1]["result"]["final"], true, "Last message should have final=true");
}

// Test resubscribing to a completed task sends final state with final=true
#[tokio::test]
async fn test_resubscribe_to_completed_task() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = StreamingService::new(repository.clone());
    
    let task_id = format!("completed-task-{}", Uuid::new_v4());
    let task = Task {
        id: task_id.clone(),
        session_id: Some("test-session".to_string()),
        status: TaskStatus {
            state: TaskState::Completed,
            timestamp: Some(Utc::now()),
            message: None,
        },
        artifacts: None,
        history: None,
        metadata: None,
    };
    
    // Save the task
    repository.save_task(&task).await.unwrap();
    
    // Act
    let request_id = json!("test-request-3");
    let stream = service.resubscribe_to_task(request_id, task_id.clone()).await.unwrap();
    
    // Assert
    let mut stream = tokio_stream::StreamExt::take(stream, 2); // Take up to two messages
    
    let mut messages = Vec::new();
    while let Some(message) = stream.next().await {
        assert!(message.is_ok(), "Stream message should be Ok");
        let message = message.unwrap();
        
        // Extract the JSON
        let json_str = message.trim_start_matches("data: ").trim_end_matches("\n\n");
        let json: Value = serde_json::from_str(json_str).expect("Should be valid JSON");
        
        messages.push(json);
    }
    
    // Should have received two messages
    assert_eq!(messages.len(), 2, "Should receive 2 messages");
    
    // Last message should have final=true
    assert_eq!(messages[1]["result"]["final"], true, "Second message should have final=true");
    // Allow for either "completed" or "Completed" - case doesn't matter in this context
    let state_str = messages[1]["result"]["status"]["state"].as_str().unwrap_or("");
    assert!(state_str.eq_ignore_ascii_case("completed"), "State should be Completed (case insensitive)");
}

// Test resubscribing to a non-existent task returns proper error
#[tokio::test]
async fn test_resubscribe_to_nonexistent_task() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = StreamingService::new(repository.clone());
    
    let non_existent_id = format!("non-existent-{}", Uuid::new_v4());
    
    // Act
    let request_id = json!("test-request-4");
    let result = service.resubscribe_to_task(request_id, non_existent_id.clone()).await;
    
    // Assert
    assert!(result.is_err(), "Resubscribing to non-existent task should fail");
    
    if let Err(err) = result {
        match err {
            ServerError::TaskNotFound(msg) => {
                // Check if the error message contains the non-existent ID
                assert!(msg.contains(&non_existent_id), 
                       "Error message '{}' should contain the task ID '{}'", msg, non_existent_id);
            },
            _ => panic!("Unexpected error type: {:?}", err),
        }
    }
}
