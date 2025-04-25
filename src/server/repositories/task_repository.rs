use crate::types::{Task, PushNotificationConfig};
use crate::server::ServerError;
use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use dashmap::DashMap; // Use DashMap for concurrent side-tables

// Import new types conditionally
#[cfg(feature = "bidir-delegate")]
use crate::bidirectional_agent::types::{TaskOrigin, TaskRelationships};


/// Task repository trait for storing and retrieving tasks
///
/// Note: This trait cannot be used with dyn pointers due to async fn in traits.
/// Instead, use the specific implementation directly (InMemoryTaskRepository).
#[async_trait]
pub trait TaskRepository: Send + Sync + 'static {
    async fn get_task(&self, id: &str) -> Result<Option<Task>, ServerError>;
    async fn save_task(&self, task: &Task) -> Result<(), ServerError>;
    async fn delete_task(&self, id: &str) -> Result<(), ServerError>;
    async fn get_push_notification_config(&self, task_id: &str) -> Result<Option<PushNotificationConfig>, ServerError>;
    async fn save_push_notification_config(&self, task_id: &str, config: &PushNotificationConfig) -> Result<(), ServerError>;
    async fn get_state_history(&self, task_id: &str) -> Result<Vec<Task>, ServerError>;
    async fn save_state_history(&self, task_id: &str, task: &Task) -> Result<(), ServerError>;
}

/// In-memory implementation of the task repository
pub struct InMemoryTaskRepository {
    // Use Mutex for primary task data as updates might involve complex logic
    tasks: Arc<Mutex<HashMap<String, Task>>>,
    push_configs: Arc<Mutex<HashMap<String, PushNotificationConfig>>>,
    state_history: Arc<Mutex<HashMap<String, Vec<Task>>>>,

    // Use DashMap for side-tables accessed frequently and concurrently by bidirectional agent
    #[cfg(feature = "bidir-delegate")]
    task_origins: Arc<DashMap<String, TaskOrigin>>,
    #[cfg(feature = "bidir-delegate")]
    task_relationships: Arc<DashMap<String, TaskRelationships>>,
}

impl InMemoryTaskRepository {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            push_configs: Arc::new(Mutex::new(HashMap::new())),
            state_history: Arc::new(Mutex::new(HashMap::new())),
            // Initialize side-tables only if feature is enabled
            #[cfg(feature = "bidir-delegate")]
            task_origins: Arc::new(DashMap::new()),
            #[cfg(feature = "bidir-delegate")]
            task_relationships: Arc::new(DashMap::new()),
        }
    }
}

#[async_trait]
impl TaskRepository for InMemoryTaskRepository {
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
