//! Helpers for task flow metadata management

use serde_json::{Map, Value};
use crate::bidirectional_agent::error::AgentError;
use crate::types::Task;
use crate::bidirectional_agent::types::{set_metadata_ext as set_metadata_ext_base};

/// Namespace for TaskFlow-specific metadata extensions
pub const TASKFLOW_NAMESPACE: &str = "a2a.bidirectional.taskflow";

/// Metadata keys used by TaskFlow
pub struct TaskFlowKeys;

impl TaskFlowKeys {
    /// Task origin type (local, delegated, etc.)
    pub const ORIGIN: &'static str = "origin";
    /// Remote agent ID for delegated tasks
    pub const REMOTE_AGENT_ID: &'static str = "remote_agent_id";
    /// Remote task ID for delegated tasks
    pub const REMOTE_TASK_ID: &'static str = "remote_task_id";
    /// Remote agent URL for delegated tasks
    pub const REMOTE_AGENT_URL: &'static str = "remote_agent_url";
    /// Timestamp when task was delegated
    pub const DELEGATED_AT: &'static str = "delegated_at";
    /// Status of the task processing
    pub const PROCESSING_STATUS: &'static str = "processing_status";
}

/// Wrapper for set_metadata_ext that works directly with Tasks
pub fn set_task_metadata(task: &mut Task, key: &str, value: Value) -> Result<(), AgentError> {
    // Initialize metadata if needed
    if task.metadata.is_none() {
        task.metadata = Some(Map::new());
    }
    
    // Get mutable reference to metadata
    if let Some(metadata) = &mut task.metadata {
        set_metadata_ext_base(metadata, key, value)?;
        Ok(())
    } else {
        // This should never happen due to the initialization above
        Err(AgentError::TaskFlowError("Failed to initialize task metadata".to_string()))
    }
}

/// Get metadata from task with namespace and key
pub fn get_task_metadata(task: &Task, namespace: &str, key: &str) -> Option<Value> {
    task.metadata.as_ref().and_then(|metadata| {
        if let Some(Value::Object(ns_obj)) = metadata.get(namespace) {
            ns_obj.get(key).cloned()
        } else {
            None
        }
    })
}

/// Set task metadata with namespace and key
pub fn set_task_metadata_ext(task: &mut Task, namespace: &str, key: &str, value: Value) -> Result<(), AgentError> {
    // Initialize metadata if needed
    if task.metadata.is_none() {
        task.metadata = Some(Map::new());
    }
    
    // Get the namespace object or create it
    if let Some(metadata) = &mut task.metadata {
        let ns_obj = if let Some(Value::Object(obj)) = metadata.get_mut(namespace) {
            obj
        } else {
            metadata.insert(namespace.to_string(), Value::Object(Map::new()));
            if let Some(Value::Object(obj)) = metadata.get_mut(namespace) {
                obj
            } else {
                return Err(AgentError::SerializationError(
                    "Failed to create metadata namespace".to_string(),
                ));
            }
        };
        
        // Set the value
        ns_obj.insert(key.to_string(), value);
        Ok(())
    } else {
        // This should never happen due to the initialization above
        Err(AgentError::TaskFlowError("Failed to initialize task metadata".to_string()))
    }
}