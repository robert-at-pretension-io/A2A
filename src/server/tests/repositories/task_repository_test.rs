use crate::server::repositories::task_repository::{TaskRepository, InMemoryTaskRepository};
use crate::types::{Task, TaskStatus, TaskState, PushNotificationConfig};
use chrono::Utc;
use uuid::Uuid;

#[tokio::test]
async fn test_save_and_get_task() {
    // Arrange
    let repository = InMemoryTaskRepository::new();
    let task_id = format!("test-task-{}", Uuid::new_v4());
    
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
    
    // Act
    repository.save_task(&task).await.unwrap();
    let retrieved_task = repository.get_task(&task_id).await.unwrap();
    
    // Assert
    assert!(retrieved_task.is_some());
    let retrieved_task = retrieved_task.unwrap();
    assert_eq!(retrieved_task.id, task_id);
    assert_eq!(retrieved_task.status.state, TaskState::Working);
}

#[tokio::test]
async fn test_delete_task() {
    // Arrange
    let repository = InMemoryTaskRepository::new();
    let task_id = format!("test-task-{}", Uuid::new_v4());
    
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
    
    // Act - Save the task
    repository.save_task(&task).await.unwrap();
    
    // Verify it was saved
    let retrieved_task = repository.get_task(&task_id).await.unwrap();
    assert!(retrieved_task.is_some());
    
    // Act - Delete the task
    repository.delete_task(&task_id).await.unwrap();
    
    // Assert
    let deleted_task = repository.get_task(&task_id).await.unwrap();
    assert!(deleted_task.is_none());
}

#[tokio::test]
async fn test_save_and_get_push_notification_config() {
    // Arrange
    let repository = InMemoryTaskRepository::new();
    let task_id = format!("test-task-{}", Uuid::new_v4());
    
    let config = PushNotificationConfig {
        url: "https://example.com/webhook".to_string(),
        authentication: None,
        token: None,
    };
    
    // Act
    repository.save_push_notification_config(&task_id, &config).await.unwrap();
    let retrieved_config = repository.get_push_notification_config(&task_id).await.unwrap();
    
    // Assert
    assert!(retrieved_config.is_some());
    let retrieved_config = retrieved_config.unwrap();
    assert_eq!(retrieved_config.url, "https://example.com/webhook");
}

#[tokio::test]
async fn test_get_missing_push_notification_config() {
    // Arrange
    let repository = InMemoryTaskRepository::new();
    let task_id = format!("test-task-{}", Uuid::new_v4());
    
    // Act
    let retrieved_config = repository.get_push_notification_config(&task_id).await.unwrap();
    
    // Assert
    assert!(retrieved_config.is_none());
}

#[tokio::test]
async fn test_save_and_get_state_history() {
    // Arrange
    let repository = InMemoryTaskRepository::new();
    let task_id = format!("test-task-{}", Uuid::new_v4());
    
    // Create a task with "Submitted" state
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
    
    // Create the same task with "Working" state
    let task2 = Task {
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
    
    // Act
    repository.save_state_history(&task_id, &task1).await.unwrap();
    repository.save_state_history(&task_id, &task2).await.unwrap();
    let history = repository.get_state_history(&task_id).await.unwrap();
    
    // Assert
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].status.state, TaskState::Submitted);
    assert_eq!(history[1].status.state, TaskState::Working);
}

#[tokio::test]
async fn test_get_empty_state_history() {
    // Arrange
    let repository = InMemoryTaskRepository::new();
    let task_id = format!("test-task-{}", Uuid::new_v4());
    
    // Act
    let history = repository.get_state_history(&task_id).await.unwrap();
    
    // Assert
    assert!(history.is_empty());
}