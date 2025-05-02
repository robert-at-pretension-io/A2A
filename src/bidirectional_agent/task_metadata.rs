/// Task metadata handling for bidirectional agents
///
/// This module provides utilities for managing task metadata, including
/// tracking task origins, execution context, and other properties.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use uuid::Uuid;

/// Origin of a task (local or delegated)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskOrigin {
    /// Task originated from the local agent
    Local,
    
    /// Task was delegated from another agent
    Delegated {
        /// ID of the agent that delegated the task
        agent_id: String,
        
        /// ID of the original task on the delegating agent
        original_task_id: String,
    },
}

/// Manager for task metadata
#[derive(Debug)]
pub struct MetadataManager {
    /// Internal storage for metadata
    metadata_store: HashMap<String, Map<String, Value>>,
}

impl MetadataManager {
    /// Create a new metadata manager
    pub fn new() -> Self {
        Self {
            metadata_store: HashMap::new(),
        }
    }
    
    /// Get metadata for a task
    pub fn get(&self, task_id: &str) -> Option<&Map<String, Value>> {
        self.metadata_store.get(task_id)
    }
    
    /// Set metadata for a task
    pub fn set(&mut self, task_id: &str, metadata: Map<String, Value>) {
        self.metadata_store.insert(task_id.to_string(), metadata);
    }
    
    /// Update metadata for a task
    pub fn update(&mut self, task_id: &str, key: &str, value: Value) -> bool {
        if let Some(metadata) = self.metadata_store.get_mut(task_id) {
            metadata.insert(key.to_string(), value);
            true
        } else {
            false
        }
    }
    
    /// Remove metadata for a task
    pub fn remove(&mut self, task_id: &str) -> Option<Map<String, Value>> {
        self.metadata_store.remove(task_id)
    }
    
    /// Create a new metadata entry with a task origin
    pub fn create_with_origin(&mut self, task_id: &str, origin: TaskOrigin) -> Map<String, Value> {
        let mut metadata = Map::new();
        
        // Serialize the origin and add to metadata
        if let Ok(origin_value) = serde_json::to_value(origin) {
            metadata.insert("origin".to_string(), origin_value);
        }
        
        // Add other default metadata
        metadata.insert("created_at".to_string(), Value::String(chrono::Utc::now().to_rfc3339()));
        metadata.insert("metadata_id".to_string(), Value::String(Uuid::new_v4().to_string()));
        
        // Store and return
        self.metadata_store.insert(task_id.to_string(), metadata.clone());
        metadata
    }
}

impl Default for MetadataManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for working with metadata

/// Get a value from a metadata map with type conversion
pub fn get_metadata_value<T>(metadata: &Map<String, Value>, key: &str) -> Option<T>
where
    T: for<'de> Deserialize<'de>,
{
    metadata.get(key).and_then(|value| serde_json::from_value::<T>(value.clone()).ok())
}

/// Set a value in a metadata map with type conversion
pub fn set_metadata_value<T>(metadata: &mut Map<String, Value>, key: &str, value: T) -> bool
where
    T: Serialize,
{
    match serde_json::to_value(value) {
        Ok(json_value) => {
            metadata.insert(key.to_string(), json_value);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_metadata_manager() {
        let mut manager = MetadataManager::new();
        let task_id = "test-task-1";
        
        // Test creating metadata with origin
        let origin = TaskOrigin::Local;
        let metadata = manager.create_with_origin(task_id, origin);
        
        // Verify metadata was created
        assert!(metadata.contains_key("origin"));
        assert!(metadata.contains_key("created_at"));
        assert!(metadata.contains_key("metadata_id"));
        
        // Test getting metadata
        let retrieved = manager.get(task_id).unwrap();
        assert_eq!(retrieved.len(), metadata.len());
        
        // Test updating metadata
        manager.update(task_id, "test_key", Value::String("test_value".to_string()));
        let updated = manager.get(task_id).unwrap();
        assert_eq!(updated.get("test_key").unwrap().as_str().unwrap(), "test_value");
        
        // Test removing metadata
        let removed = manager.remove(task_id).unwrap();
        assert_eq!(removed.len(), updated.len());
        assert!(manager.get(task_id).is_none());
    }
    
    #[test]
    fn test_metadata_helpers() {
        let mut metadata = Map::new();
        
        // Test setting string
        assert!(set_metadata_value(&mut metadata, "string_key", "string_value"));
        
        // Test setting number
        assert!(set_metadata_value(&mut metadata, "number_key", 42));
        
        // Test getting string
        let string_value: Option<String> = get_metadata_value(&metadata, "string_key");
        assert_eq!(string_value.unwrap(), "string_value");
        
        // Test getting number
        let number_value: Option<i32> = get_metadata_value(&metadata, "number_key");
        assert_eq!(number_value.unwrap(), 42);
        
        // Test getting missing value
        let missing_value: Option<String> = get_metadata_value(&metadata, "missing_key");
        assert!(missing_value.is_none());
    }
}