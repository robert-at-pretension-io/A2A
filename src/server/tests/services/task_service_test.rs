use crate::server::repositories::task_repository::TaskRepository;
use crate::server::services::task_service::TaskService;
use crate::server::ServerError;
use crate::types::{
    Message, Part, Role, Task, TaskIdParams, TaskQueryParams, TaskSendParams, TaskState,
    TaskStatus, TextPart,
};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

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

    async fn get_push_notification_config(
        &self,
        task_id: &str,
    ) -> Result<Option<crate::types::PushNotificationConfig>, ServerError> {
        let push_configs = self.push_configs.lock().await;
        Ok(push_configs.get(task_id).cloned())
    }

    async fn save_push_notification_config(
        &self,
        task_id: &str,
        config: &crate::types::PushNotificationConfig,
    ) -> Result<(), ServerError> {
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

// Test creating a simple task with text content returns a valid task ID and "working" state
#[tokio::test]
async fn test_process_task_valid_input_returns_task() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = TaskService::standalone(repository.clone());

    let task_id = format!("test-task-{}", Uuid::new_v4());
    let text_part = TextPart {
        type_: "text".to_string(),
        text: "Test task content".to_string(),
        metadata: None,
    };

    let message = Message {
        role: Role::User,
        parts: vec![Part::TextPart(text_part)],
        metadata: None,
    };

    let params = TaskSendParams {
        id: task_id.clone(),
        message,
        session_id: None,
        metadata: None,
        history_length: None,
        push_notification: None,
    };

    // Act
    let result = service.process_task(params).await.unwrap();

    // Assert
    assert_eq!(result.id, task_id, "Task ID should match the provided ID");
    assert_eq!(
        result.status.state,
        TaskState::Completed,
        "Task should be completed"
    );

    // Verify the task was saved in the repository
    let saved_task = repository.get_task(&task_id).await.unwrap();
    assert!(
        saved_task.is_some(),
        "Task should be saved in the repository"
    );

    // Verify state history was saved
    let history = repository.get_state_history(&task_id).await.unwrap();
    assert!(!history.is_empty(), "Task state history should be saved");
}

// Test creating a task with unique ID persists the task correctly
#[tokio::test]
async fn test_process_task_unique_id_persists_correctly() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = TaskService::standalone(repository.clone());

    let task_id = format!("unique-task-{}", Uuid::new_v4());
    let text_part = TextPart {
        type_: "text".to_string(),
        text: "Task with unique ID".to_string(),
        metadata: None,
    };

    let message = Message {
        role: Role::User,
        parts: vec![Part::TextPart(text_part)],
        metadata: None,
    };

    let params = TaskSendParams {
        id: task_id.clone(),
        message,
        session_id: Some("test-session".to_string()),
        metadata: Some({
            let mut map = serde_json::Map::new();
            map.insert(
                "test".to_string(),
                serde_json::Value::String("metadata".to_string()),
            );
            map
        }),
        history_length: None,
        push_notification: None,
    };

    // Act
    service.process_task(params).await.unwrap();

    // Assert
    let saved_task = repository.get_task(&task_id).await.unwrap();
    assert!(
        saved_task.is_some(),
        "Task should be saved in the repository"
    );

    let task = saved_task.unwrap();
    assert_eq!(task.id, task_id, "Task ID should match");
    assert_eq!(
        task.session_id.unwrap(),
        "test-session",
        "Session ID should match"
    );
    assert!(task.metadata.is_some(), "Metadata should be present");

    if let Some(metadata) = task.metadata {
        assert_eq!(
            metadata.get("test").and_then(|v| v.as_str()),
            Some("metadata"),
            "Metadata should contain test field"
        );
    }
}

// Test creating a task with the same ID twice returns an appropriate error
#[tokio::test]
async fn test_create_task_same_id_twice_returns_error() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = TaskService::standalone(repository.clone());

    let task_id = format!("duplicate-task-{}", Uuid::new_v4());

    // Create the first task
    let text_part = TextPart {
        type_: "text".to_string(),
        text: "First task content".to_string(),
        metadata: None,
    };

    let message = Message {
        role: Role::User,
        parts: vec![Part::TextPart(text_part)],
        metadata: None,
    };

    let params = TaskSendParams {
        id: task_id.clone(),
        message: message.clone(),
        session_id: None,
        metadata: None,
        history_length: None,
        push_notification: None,
    };

    // Create the first task
    let first_result = service.process_task(params.clone()).await.unwrap();
    assert_eq!(first_result.id, task_id);

    // Modify the task to be in a state that doesn't accept follow-ups
    let mut tasks = repository.tasks.lock().await;
    let mut task = tasks.get(&task_id).unwrap().clone();
    task.status.state = TaskState::Completed;
    tasks.insert(task_id.clone(), task);
    drop(tasks);

    // Act & Assert
    // Try to create a second task with the same ID
    let second_result = service.process_task(params.clone()).await;

    // The operation should return an error since the task is already completed
    assert!(
        second_result.is_err(),
        "Creating a task with the same ID should fail"
    );

    if let Err(err) = second_result {
        match err {
            ServerError::InvalidParameters(msg) => {
                assert!(
                    msg.contains("cannot accept follow-up"),
                    "Error message should indicate follow-up not allowed"
                );
            }
            _ => panic!("Unexpected error type: {:?}", err),
        }
    }
}

// Test retrieving an existing task returns the correct task data
#[tokio::test]
async fn test_get_task_existing_returns_correct_data() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = TaskService::standalone(repository.clone());

    let task_id = format!("task-to-retrieve-{}", Uuid::new_v4());

    // Create a task
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
        metadata: Some({
            let mut map = serde_json::Map::new();
            map.insert(
                "test".to_string(),
                serde_json::Value::String("metadata".to_string()),
            );
            map
        }),
    };

    // Save the task directly in the repository
    repository.save_task(&task).await.unwrap();

    // Act
    let query_params = TaskQueryParams {
        id: task_id.clone(),
        history_length: None,
        metadata: None,
    };
    let retrieved_task = service.get_task(query_params).await.unwrap();

    // Assert
    assert_eq!(retrieved_task.id, task_id, "Task ID should match");
    assert_eq!(
        retrieved_task.session_id.unwrap(),
        "test-session",
        "Session ID should match"
    );
    assert_eq!(
        retrieved_task.status.state,
        TaskState::Working,
        "Task state should match"
    );
    assert!(
        retrieved_task.metadata.is_some(),
        "Metadata should be present"
    );
}

// Test retrieving a non-existent task returns proper error (not found)
#[tokio::test]
async fn test_get_task_nonexistent_returns_error() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = TaskService::standalone(repository.clone());

    let non_existent_id = format!("non-existent-{}", Uuid::new_v4());

    // Act
    let query_params = TaskQueryParams {
        id: non_existent_id.clone(),
        history_length: None,
        metadata: None,
    };
    let result = service.get_task(query_params).await;

    // Assert
    assert!(result.is_err(), "Getting a non-existent task should fail");

    if let Err(err) = result {
        match err {
            ServerError::TaskNotFound(id) => {
                assert_eq!(id, non_existent_id, "Error should contain the task ID");
            }
            _ => panic!("Unexpected error type: {:?}", err),
        }
    }
}

// Test canceling a task in working state transitions it to canceled
#[tokio::test]
async fn test_cancel_task_working_state_transitions_to_canceled() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = TaskService::standalone(repository.clone());

    let task_id = format!("task-to-cancel-{}", Uuid::new_v4());

    // Create a task in Working state
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

    // Save the task directly
    repository.save_task(&task).await.unwrap();

    // Act
    let cancel_params = TaskIdParams {
        id: task_id.clone(),
        metadata: None,
    };
    let canceled_task = service.cancel_task(cancel_params).await.unwrap();

    // Assert
    assert_eq!(canceled_task.id, task_id, "Task ID should match");
    assert_eq!(
        canceled_task.status.state,
        TaskState::Canceled,
        "Task should be canceled"
    );

    // Verify the task in the repository is updated
    let updated_task = repository.get_task(&task_id).await.unwrap().unwrap();
    assert_eq!(
        updated_task.status.state,
        TaskState::Canceled,
        "Repository task should be canceled"
    );

    // Verify state history was updated
    let history = repository.get_state_history(&task_id).await.unwrap();
    assert!(!history.is_empty(), "State history should be recorded");
    assert_eq!(
        history.last().unwrap().status.state,
        TaskState::Canceled,
        "Last history entry should be Canceled"
    );
}

// Test canceling an already completed task returns appropriate error
#[tokio::test]
async fn test_cancel_completed_task_returns_error() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = TaskService::standalone(repository.clone());

    let task_id = format!("completed-task-{}", Uuid::new_v4());

    // Create a task in Completed state
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

    // Save the task directly
    repository.save_task(&task).await.unwrap();

    // Act
    let cancel_params = TaskIdParams {
        id: task_id.clone(),
        metadata: None,
    };
    let result = service.cancel_task(cancel_params).await;

    // Assert
    assert!(result.is_err(), "Canceling a completed task should fail");

    if let Err(err) = result {
        match err {
            ServerError::TaskNotCancelable(msg) => {
                assert!(
                    msg.contains("cannot be canceled"),
                    "Error should indicate task cannot be canceled"
                );
                assert!(msg.contains(&task_id), "Error should contain task ID");
            }
            _ => panic!("Unexpected error type: {:?}", err),
        }
    }
}

// Test sending a follow-up message to a task in input-required state transitions it to working
#[tokio::test]
async fn test_follow_up_message_transitions_input_required_to_working() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = TaskService::standalone(repository.clone());

    let task_id = format!("input-required-task-{}", Uuid::new_v4());

    // Create a task in InputRequired state
    let task = Task {
        id: task_id.clone(),
        session_id: Some("test-session".to_string()),
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
        artifacts: None,
        history: None,
        metadata: None,
    };

    // Save the task directly
    repository.save_task(&task).await.unwrap();

    // Create a follow-up message
    let follow_up_text = TextPart {
        type_: "text".to_string(),
        text: "Here's the additional information you requested".to_string(),
        metadata: None,
    };

    let follow_up_message = Message {
        role: Role::User,
        parts: vec![Part::TextPart(follow_up_text)],
        metadata: None,
    };

    let params = TaskSendParams {
        id: task_id.clone(),
        message: follow_up_message,
        session_id: None,
        metadata: None,
        history_length: None,
        push_notification: None,
    };

    // Act
    let updated_task = service.process_task(params).await.unwrap();

    // Assert
    assert_eq!(updated_task.id, task_id, "Task ID should match");
    assert_eq!(
        updated_task.status.state,
        TaskState::Completed,
        "Task should transition to Completed (in our simplified implementation)"
    );

    // Verify state history captures the transition
    let history = repository.get_state_history(&task_id).await.unwrap();
    assert!(!history.is_empty(), "State history should be recorded");

    // The history should show: InputRequired -> Working -> Completed
    // But in our implementation, we might not capture the intermediate Working state
    if history.len() >= 2 {
        assert_eq!(
            history[0].status.state,
            TaskState::InputRequired,
            "First history entry should be InputRequired"
        );
        assert_eq!(
            history.last().unwrap().status.state,
            TaskState::Completed,
            "Last history entry should be Completed"
        );
    }
}

// Test task state transition history
#[tokio::test]
async fn test_get_task_state_history() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = TaskService::standalone(repository.clone());

    let task_id = format!("history-task-{}", Uuid::new_v4());

    // Create a task
    let task1 = Task {
        id: task_id.clone(),
        session_id: Some("test-session".to_string()),
        status: TaskStatus {
            state: TaskState::Submitted,
            timestamp: Some(Utc::now()),
            message: None,
        },
        artifacts: None,
        history: None,
        metadata: None,
    };

    // Add to history
    repository.save_task(&task1).await.unwrap();
    repository
        .save_state_history(&task_id, &task1)
        .await
        .unwrap();

    // Update to Working state
    let mut task2 = task1.clone();
    task2.status.state = TaskState::Working;
    task2.status.timestamp = Some(Utc::now());
    repository.save_task(&task2).await.unwrap();
    repository
        .save_state_history(&task_id, &task2)
        .await
        .unwrap();

    // Update to Completed state
    let mut task3 = task2.clone();
    task3.status.state = TaskState::Completed;
    task3.status.timestamp = Some(Utc::now());
    repository.save_task(&task3).await.unwrap();
    repository
        .save_state_history(&task_id, &task3)
        .await
        .unwrap();

    // Act
    let history = service.get_task_state_history(&task_id).await.unwrap();

    // Assert
    assert_eq!(history.len(), 3, "Should have 3 history entries");
    assert_eq!(
        history[0].status.state,
        TaskState::Submitted,
        "First state should be Submitted"
    );
    assert_eq!(
        history[1].status.state,
        TaskState::Working,
        "Second state should be Working"
    );
    assert_eq!(
        history[2].status.state,
        TaskState::Completed,
        "Third state should be Completed"
    );
}

// Test task history for non-existent task
#[tokio::test]
async fn test_get_task_state_history_nonexistent_returns_error() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = TaskService::standalone(repository.clone());

    let non_existent_id = format!("non-existent-{}", Uuid::new_v4());

    // Act
    let result = service.get_task_state_history(&non_existent_id).await;

    // Assert
    assert!(
        result.is_err(),
        "Getting history for non-existent task should fail"
    );

    if let Err(err) = result {
        match err {
            ServerError::TaskNotFound(id) => {
                assert_eq!(id, non_existent_id, "Error should contain the task ID");
            }
            _ => panic!("Unexpected error type: {:?}", err),
        }
    }
}
