use std::error::Error;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::client::A2aClient;
use crate::types::{TaskStatus, TaskState, Message, Task, Part, TextPart, Role};

/// Represents a batch of tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskBatch {
    /// Unique identifier for the batch
    pub id: String,
    /// Name of the batch (optional)
    pub name: Option<String>,
    /// Timestamp when the batch was created
    pub created_at: DateTime<Utc>,
    /// List of task IDs included in this batch
    pub task_ids: Vec<String>,
    /// Metadata associated with the batch (optional)
    pub metadata: Option<serde_json::Map<String, Value>>,
}

/// Parameters for creating a new task batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchCreateParams {
    /// Optional batch ID (will be auto-generated if not provided)
    pub id: Option<String>,
    /// Optional name for the batch
    pub name: Option<String>,
    /// List of task inputs (text prompts)
    pub tasks: Vec<String>,
    /// Optional metadata for the batch
    pub metadata: Option<serde_json::Map<String, Value>>,
}

/// Summary status information for a batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchStatusSummary {
    /// The batch ID
    pub batch_id: String,
    /// Total number of tasks in the batch
    pub total_tasks: usize,
    /// Number of tasks in each state
    pub state_counts: HashMap<TaskState, usize>,
    /// Overall batch status (derived from tasks)
    pub overall_status: BatchStatus,
    /// Timestamp of the status check
    pub timestamp: DateTime<Utc>,
}

/// Overall status of a batch
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BatchStatus {
    /// All tasks in the batch have been submitted but not started
    Submitted,
    /// At least one task is working, none have failed/been canceled
    Working,
    /// At least one task requires input
    InputRequired,
    /// All tasks have completed successfully
    Completed,
    /// At least one task was canceled
    PartlyCanceled,
    /// All tasks were canceled
    Canceled,
    /// At least one task failed
    PartlyFailed,
    /// All tasks failed
    Failed,
    /// Batch in mixed state (some completed, some failed, etc.)
    Mixed,
}

/// Request parameters for batch operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchQueryParams {
    /// Batch ID to query
    pub id: String,
    /// Include full task details (not just IDs)
    pub include_tasks: Option<bool>,
    /// Include task history
    pub include_history: Option<bool>,
}

impl A2aClient {
    /// Create a new batch of tasks
    pub async fn create_task_batch(&mut self, params: BatchCreateParams) -> Result<TaskBatch, Box<dyn Error>> {
        // Generate task IDs
        let mut task_ids = Vec::with_capacity(params.tasks.len());
        
        // Create the tasks for the batch
        for task_text in &params.tasks {
            let task = self.send_task(task_text).await?;
            task_ids.push(task.id.clone());
        }
        
        // Create the batch object
        let batch = TaskBatch {
            id: params.id.unwrap_or_else(|| Uuid::new_v4().to_string()),
            name: params.name,
            created_at: Utc::now(),
            task_ids,
            metadata: params.metadata,
        };
        
        // Store the batch information via the API
        let batch_params = json!({
            "batch": batch,
        });
        
        self.send_jsonrpc::<TaskBatch>("batches/create", batch_params).await
    }
    
    /// Get a batch by ID
    pub async fn get_batch(&mut self, batch_id: &str, include_tasks: bool) -> Result<TaskBatch, Box<dyn Error>> {
        let params = BatchQueryParams {
            id: batch_id.to_string(),
            include_tasks: Some(include_tasks),
            include_history: None,
        };
        
        self.send_jsonrpc::<TaskBatch>("batches/get", serde_json::to_value(params)?).await
    }
    
    /// Get batch status summary
    pub async fn get_batch_status(&mut self, batch_id: &str) -> Result<BatchStatusSummary, Box<dyn Error>> {
        // First get the batch
        let batch = self.get_batch(batch_id, false).await?;
        
        // Create a status summary
        let mut summary = BatchStatusSummary {
            batch_id: batch_id.to_string(),
            total_tasks: batch.task_ids.len(),
            state_counts: HashMap::new(),
            overall_status: BatchStatus::Submitted, // Default, will be updated
            timestamp: Utc::now(),
        };
        
        // Initialize counts
        for state in [
            TaskState::Submitted,
            TaskState::Working,
            TaskState::InputRequired,
            TaskState::Completed,
            TaskState::Canceled,
            TaskState::Failed,
            TaskState::Unknown,
        ] {
            summary.state_counts.insert(state, 0);
        }
        
        // Fetch all tasks in the batch
        for task_id in &batch.task_ids {
            let task = self.get_task(task_id).await?;
            let state = task.status.state;
            
            // Increment the appropriate counter
            if let Some(count) = summary.state_counts.get_mut(&state) {
                *count += 1;
            }
        }
        
        // Determine overall status
        summary.overall_status = derive_batch_status(&summary.state_counts);
        
        Ok(summary)
    }
    
    /// Cancel all tasks in a batch
    pub async fn cancel_batch(&mut self, batch_id: &str) -> Result<BatchStatusSummary, Box<dyn Error>> {
        // First get the batch
        let batch = self.get_batch(batch_id, false).await?;
        
        // Cancel each task
        for task_id in &batch.task_ids {
            // Ignore errors for individual cancellations, as some tasks might already be completed
            let _ = self.cancel_task(task_id).await;
        }
        
        // Return the updated status
        self.get_batch_status(batch_id).await
    }
    
    /// Get all tasks in a batch
    pub async fn get_batch_tasks(&mut self, batch_id: &str) -> Result<Vec<Task>, Box<dyn Error>> {
        // First get the batch
        let batch = self.get_batch(batch_id, false).await?;
        
        // Fetch all tasks
        let mut tasks = Vec::with_capacity(batch.task_ids.len());
        for task_id in &batch.task_ids {
            let task = self.get_task(task_id).await?;
            tasks.push(task);
        }
        
        Ok(tasks)
    }
}

/// Helper function to derive overall batch status from task state counts
fn derive_batch_status(state_counts: &HashMap<TaskState, usize>) -> BatchStatus {
    let total_tasks: usize = state_counts.values().sum();
    
    // All tasks completed
    if state_counts.get(&TaskState::Completed).copied().unwrap_or(0) == total_tasks {
        return BatchStatus::Completed;
    }
    
    // All tasks failed
    if state_counts.get(&TaskState::Failed).copied().unwrap_or(0) == total_tasks {
        return BatchStatus::Failed;
    }
    
    // All tasks canceled
    if state_counts.get(&TaskState::Canceled).copied().unwrap_or(0) == total_tasks {
        return BatchStatus::Canceled;
    }
    
    // At least one task failed
    if state_counts.get(&TaskState::Failed).copied().unwrap_or(0) > 0 {
        return BatchStatus::PartlyFailed;
    }
    
    // At least one task canceled
    if state_counts.get(&TaskState::Canceled).copied().unwrap_or(0) > 0 {
        return BatchStatus::PartlyCanceled;
    }
    
    // At least one task needs input
    if state_counts.get(&TaskState::InputRequired).copied().unwrap_or(0) > 0 {
        return BatchStatus::InputRequired;
    }
    
    // At least one task is working
    if state_counts.get(&TaskState::Working).copied().unwrap_or(0) > 0 {
        return BatchStatus::Working;
    }
    
    // At least one task is completed and others are in different states
    if state_counts.get(&TaskState::Completed).copied().unwrap_or(0) > 0 {
        return BatchStatus::Mixed;
    }
    
    // Default: all tasks are still in submitted state
    BatchStatus::Submitted
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    #[test]
    fn test_derive_batch_status() {
        // Test case 1: All tasks completed
        let mut counts = HashMap::new();
        counts.insert(TaskState::Completed, 5);
        counts.insert(TaskState::Failed, 0);
        counts.insert(TaskState::Working, 0);
        
        assert_eq!(derive_batch_status(&counts), BatchStatus::Completed);
        
        // Test case 2: Mixed state with failures
        let mut counts = HashMap::new();
        counts.insert(TaskState::Completed, 3);
        counts.insert(TaskState::Failed, 2);
        counts.insert(TaskState::Working, 0);
        
        assert_eq!(derive_batch_status(&counts), BatchStatus::PartlyFailed);
        
        // Test case 3: All working
        let mut counts = HashMap::new();
        counts.insert(TaskState::Completed, 0);
        counts.insert(TaskState::Failed, 0);
        counts.insert(TaskState::Working, 5);
        
        assert_eq!(derive_batch_status(&counts), BatchStatus::Working);
    }
}