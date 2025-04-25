//! Extensions to the TaskRepository trait for bidirectional agent features.

#![cfg(feature = "bidir-delegate")]

use crate::server::repositories::task_repository::TaskRepository;
use crate::server::ServerError;
use crate::bidirectional_agent::types::{TaskOrigin, TaskRelationships};
use async_trait::async_trait;

/// Extends the base TaskRepository with methods needed for delegation and decomposition.
#[async_trait]
pub trait TaskRepositoryExt: TaskRepository {
    /// Sets the origin of a task (Local, Delegated, Remote).
    async fn set_task_origin(&self, task_id: &str, origin: TaskOrigin) -> Result<(), ServerError>;

    /// Retrieves the origin of a task.
    async fn get_task_origin(&self, task_id: &str) -> Result<Option<TaskOrigin>, ServerError>;

    /// Links child tasks to a parent task.
    async fn link_tasks(&self, parent_task_id: &str, child_task_ids: &[String]) -> Result<(), ServerError>;

    /// Retrieves the relationships (parent/children) for a task.
    async fn get_task_relationships(&self, task_id: &str) -> Result<TaskRelationships, ServerError>;
}

// --- Blanket Implementation for InMemoryTaskRepository ---
// This requires modifying InMemoryTaskRepository to store origin/relationships.
// We assume the fields `task_origins` and `task_relationships` (Arc<DashMap<...>>)
// were added in Slice 1/2, guarded by `#[cfg(feature = "bidir-delegate")]`.

use crate::server::repositories::task_repository::InMemoryTaskRepository;
use std::sync::Arc;

#[async_trait]
impl TaskRepositoryExt for InMemoryTaskRepository {
    async fn set_task_origin(&self, task_id: &str, origin: TaskOrigin) -> Result<(), ServerError> {
        // Ensure the feature is enabled at compile time
        #[cfg(feature = "bidir-delegate")]
        {
            self.task_origins.insert(task_id.to_string(), origin);
            Ok(())
        }
        // If the feature is not enabled, this method shouldn't be callable,
        // but we provide a default Ok(()) to satisfy the trait bound if needed.
        #[cfg(not(feature = "bidir-delegate"))]
        {
             println!("Warning: set_task_origin called but bidir-delegate feature is not enabled.");
             Ok(())
        }
    }

    async fn get_task_origin(&self, task_id: &str) -> Result<Option<TaskOrigin>, ServerError> {
         #[cfg(feature = "bidir-delegate")]
         {
            Ok(self.task_origins.get(task_id).map(|entry| entry.value().clone()))
         }
         #[cfg(not(feature = "bidir-delegate"))]
         {
              println!("Warning: get_task_origin called but bidir-delegate feature is not enabled.");
              Ok(None)
         }
    }

    async fn link_tasks(&self, parent_task_id: &str, child_task_ids: &[String]) -> Result<(), ServerError> {
         #[cfg(feature = "bidir-delegate")]
         {
            // Update parent's children
            self.task_relationships.entry(parent_task_id.to_string())
                .or_default()
                .children.extend(child_task_ids.iter().map(|s| s.to_string()));

            // Update children's parent
            for child_id in child_task_ids {
                 self.task_relationships.entry(child_id.to_string())
                    .or_default()
                    .parent_id = Some(parent_task_id.to_string());
            }
            Ok(())
         }
          #[cfg(not(feature = "bidir-delegate"))]
         {
              println!("Warning: link_tasks called but bidir-delegate feature is not enabled.");
              Ok(())
         }
    }

    async fn get_task_relationships(&self, task_id: &str) -> Result<TaskRelationships, ServerError> {
         #[cfg(feature = "bidir-delegate")]
         {
             Ok(self.task_relationships.get(task_id)
                .map(|entry| entry.value().clone())
                .unwrap_or_default()) // Return default (empty relationships) if not found
         }
          #[cfg(not(feature = "bidir-delegate"))]
         {
              println!("Warning: get_task_relationships called but bidir-delegate feature is not enabled.");
              Ok(TaskRelationships::default())
         }
    }
}
