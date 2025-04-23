use crate::server::services::notification_service::NotificationService;
use crate::server::repositories::task_repository::TaskRepository;
use crate::server::ServerError;
use crate::types::{Task, TaskStatus, TaskState, PushNotificationConfig, 
                   TaskPushNotificationConfig, TaskIdParams};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::Mutex;
use async_trait::async_trait;
use chrono::Utc;
use uuid::Uuid;

// Create a mock task repository for testing
struct MockTaskRepository {
    tasks: Mutex<HashMap<String, Task>>,
    push_configs: Mutex<HashMap<String, PushNotificationConfig>>,
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
    
    async fn get_push_notification_config(&self, task_id: &str) -> Result<Option<PushNotificationConfig>, ServerError> {
        let push_configs = self.push_configs.lock().await;
        Ok(push_configs.get(task_id).cloned())
    }
    
    async fn save_push_notification_config(&self, task_id: &str, config: &PushNotificationConfig) -> Result<(), ServerError> {
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

// Test setting a webhook URL for a task succeeds
#[tokio::test]
async fn test_set_push_notification_succeeds() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = NotificationService::new(repository.clone());
    
    // Create a task first
    let task_id = format!("webhook-task-{}", Uuid::new_v4());
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
    
    // Create webhook config
    let webhook_url = "https://example.com/webhook";
    let config = PushNotificationConfig {
        url: webhook_url.to_string(),
        token: None,
        authentication: None,
    };
    
    let params = TaskPushNotificationConfig {
        id: task_id.clone(),
        push_notification_config: config.clone(),
    };
    
    // Act
    let result = service.set_push_notification(params).await;
    
    // Assert
    assert!(result.is_ok(), "Setting push notification should succeed");
    
    // Verify the config was saved
    let saved_config = repository.get_push_notification_config(&task_id).await.unwrap();
    assert!(saved_config.is_some(), "Push notification config should be saved");
    
    let saved_config = saved_config.unwrap();
    assert_eq!(saved_config.url, webhook_url, "Webhook URL should match");
}

// Test setting a webhook with authentication parameters is stored correctly
#[tokio::test]
async fn test_set_push_notification_with_auth_succeeds() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = NotificationService::new(repository.clone());
    
    // Create a task first
    let task_id = format!("webhook-auth-task-{}", Uuid::new_v4());
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
    
    // Create webhook config with authentication
    let webhook_url = "https://example.com/webhook";
    let config = PushNotificationConfig {
        url: webhook_url.to_string(),
        token: None,
        authentication: Some(crate::types::AuthenticationInfo {
            schemes: vec!["Bearer".to_string()],
            credentials: Some("test-token-123".to_string()),
            extra: serde_json::Map::new(),
        }),
    };
    
    let params = TaskPushNotificationConfig {
        id: task_id.clone(),
        push_notification_config: config.clone(),
    };
    
    // Act
    let result = service.set_push_notification(params).await;
    
    // Assert
    assert!(result.is_ok(), "Setting push notification with auth should succeed");
    
    // Verify the config was saved with auth details
    let saved_config = repository.get_push_notification_config(&task_id).await.unwrap().unwrap();
    assert_eq!(saved_config.url, webhook_url, "Webhook URL should match");
    
    let auth = saved_config.authentication.unwrap();
    assert!(auth.schemes.contains(&"Bearer".to_string()), "Auth scheme should match");
    assert_eq!(auth.credentials, Some("test-token-123".to_string()), "Auth credentials should match");
}

// Test setting a webhook for a non-existent task returns proper error
#[tokio::test]
async fn test_set_push_notification_nonexistent_task_returns_error() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = NotificationService::new(repository.clone());
    
    let non_existent_id = format!("non-existent-{}", Uuid::new_v4());
    
    // Create webhook config
    let config = PushNotificationConfig {
        url: "https://example.com/webhook".to_string(),
        token: None,
        authentication: None,
    };
    
    let params = TaskPushNotificationConfig {
        id: non_existent_id.clone(),
        push_notification_config: config,
    };
    
    // Act
    let result = service.set_push_notification(params).await;
    
    // Assert
    assert!(result.is_err(), "Setting push notification for non-existent task should fail");
    
    if let Err(err) = result {
        match err {
            ServerError::TaskNotFound(id) => {
                assert_eq!(id, non_existent_id, "Error should contain the task ID");
            },
            _ => panic!("Unexpected error type: {:?}", err),
        }
    }
}

// Test retrieving webhook configuration returns correct URL and auth details
#[tokio::test]
async fn test_get_push_notification_returns_correct_details() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = NotificationService::new(repository.clone());
    
    // Create a task first
    let task_id = format!("webhook-get-task-{}", Uuid::new_v4());
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
    
    // Save webhook config directly
    let webhook_url = "https://example.com/webhook";
    let config = PushNotificationConfig {
        url: webhook_url.to_string(),
        token: None,
        authentication: Some(crate::types::AuthenticationInfo {
            schemes: vec!["Bearer".to_string()],
            credentials: Some("test-token-123".to_string()),
            extra: serde_json::Map::new(),
        }),
    };
    
    repository.save_push_notification_config(&task_id, &config).await.unwrap();
    
    // Act
    let params = TaskIdParams {
        id: task_id.clone(),
        metadata: None,
    };
    let retrieved_config = service.get_push_notification(params).await.unwrap();
    
    // Assert
    assert_eq!(retrieved_config.url, webhook_url, "Webhook URL should match");
    
    let auth = retrieved_config.authentication.unwrap();
    assert!(auth.schemes.contains(&"Bearer".to_string()), "Auth scheme should match");
    assert_eq!(auth.credentials, Some("test-token-123".to_string()), "Auth credentials should match");
}

// Test retrieving webhook for a task without configuration returns appropriate error
#[tokio::test]
async fn test_get_push_notification_no_config_returns_error() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = NotificationService::new(repository.clone());
    
    // Create a task first (but no webhook config)
    let task_id = format!("no-webhook-task-{}", Uuid::new_v4());
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
    let params = TaskIdParams {
        id: task_id.clone(),
        metadata: None,
    };
    let result = service.get_push_notification(params).await;
    
    // Assert
    assert!(result.is_err(), "Getting non-existent push notification config should fail");
    
    if let Err(err) = result {
        match err {
            ServerError::InvalidParameters(msg) => {
                assert!(msg.contains("No push notification configuration found"), 
                       "Error should indicate no config found");
                assert!(msg.contains(&task_id), "Error should contain task ID");
            },
            _ => panic!("Unexpected error type: {:?}", err),
        }
    }
}

// Test retrieving webhook for a non-existent task returns proper error
#[tokio::test]
async fn test_get_push_notification_nonexistent_task_returns_error() {
    // Arrange
    let repository = Arc::new(MockTaskRepository::new());
    let service = NotificationService::new(repository.clone());
    
    let non_existent_id = format!("non-existent-{}", Uuid::new_v4());
    
    // Act
    let params = TaskIdParams {
        id: non_existent_id.clone(),
        metadata: None,
    };
    let result = service.get_push_notification(params).await;
    
    // Assert
    assert!(result.is_err(), "Getting push notification for non-existent task should fail");
    
    if let Err(err) = result {
        match err {
            ServerError::TaskNotFound(id) => {
                assert_eq!(id, non_existent_id, "Error should contain the task ID");
            },
            _ => panic!("Unexpected error type: {:?}", err),
        }
    }
}