//! Extensions for TaskRepository to support tracking task origins.



use crate::bidirectional_agent::error::AgentError;
use crate::bidirectional_agent::types::TaskOrigin;
use crate::bidirectional_agent::types::TaskRelationships;
use crate::server::repositories::task_repository::TaskRepository;
use crate::types::Task;
use async_trait::async_trait;
use std::collections::HashMap;

/// Extension trait for TaskRepository to track task origins.
#[async_trait]
pub trait TaskRepositoryExt: TaskRepository {
    /// Sets the origin of a task (local, remote, decomposed).
    async fn set_task_origin(&self, task_id: &str, origin: TaskOrigin) -> Result<(), AgentError>;

    /// Gets the origin of a task.
    async fn get_task_origin(&self, task_id: &str) -> Result<Option<TaskOrigin>, AgentError>;

    /// Adds parent/child relationship between tasks.
    async fn add_task_relationship(
        &self,
        parent_task_id: &str,
        child_id: &str,
    ) -> Result<(), AgentError>;

    /// Gets task relationships (parent/children).
    async fn get_task_relationships(&self, task_id: &str) -> Result<Option<TaskRelationships>, AgentError>;
}

/// Implementation for InMemoryTaskRepository that stores origins in a separate map.
#[async_trait]
impl TaskRepositoryExt for crate::server::repositories::task_repository::InMemoryTaskRepository {
    async fn set_task_origin(&self, task_id: &str, origin: TaskOrigin) -> Result<(), AgentError> {
        // Store in a private field or HashMap extension
        // Using a dummy implementation for now
        println!("Setting task origin for {}: {:?}", task_id, origin);
        Ok(())
    }

    async fn get_task_origin(&self, task_id: &str) -> Result<Option<TaskOrigin>, AgentError> {
        // Retrieve from a private field or HashMap extension
        // Using a dummy implementation for now
        println!("Getting task origin for {}", task_id);
        Ok(None)
    }

    async fn add_task_relationship(
        &self,
        parent_task_id: &str,
        child_id: &str,
    ) -> Result<(), AgentError> {
        // Add bidirectional relationship
        println!("Adding relationship: {} is parent of {}", parent_task_id, child_id);
        Ok(())
    }

    async fn get_task_relationships(
        &self,
        task_id: &str,
    ) -> Result<Option<TaskRelationships>, AgentError> {
        // Retrieve from a private field or HashMap extension
        // Using a dummy implementation for now
        println!("Getting relationships for {}", task_id);
        Ok(Some(TaskRelationships {
            parent_task_ids: vec![],
            child_task_ids: vec![],
            related_task_ids: std::collections::HashMap::new(),
        }))
    }
}
