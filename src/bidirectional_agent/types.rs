//! Type definitions for bidirectional agent aligned with A2A protocol standards.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;

// Re-export standard A2A types to ensure consistent usage
pub use crate::types::{
    Part, TextPart, DataPart, FilePart,
    Message, Role, Task, TaskStatus, TaskState,
    Artifact, TaskSendParams, TaskQueryParams,
};

/// Namespace for custom metadata to avoid conflicts with standard A2A fields
pub const METADATA_NAMESPACE: &str = "a2a_test_suite";

/// Information about the origin of a task
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskOrigin {
    /// Task originated locally within this agent
    Local,
    /// Task was delegated to another agent
    Delegated {
        /// The ID of the agent that this task was delegated to
        agent_id: String,
        /// Optional URL of the agent
        agent_url: Option<String>,
        /// Timestamp of when the task was delegated
        delegated_at: String,
    },
    /// Task was received from another agent
    External {
        /// The ID of the agent that created this task
        agent_id: String,
        /// Optional URL of the agent
        agent_url: Option<String>,
        /// Timestamp of when the task was created
        created_at: String,
    },
}

/// Relationships between tasks in a workflow
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRelationships {
    /// Parent task IDs that led to this task
    pub parent_task_ids: Vec<String>,
    /// Child task IDs created from this task
    pub child_task_ids: Vec<String>,
    /// Related task IDs that are associated but not direct parents/children
    pub related_task_ids: HashMap<String, String>,
}

/// Extension metadata that can be stored in standard A2A metadata fields
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TaskExtensionMetadata {
    /// Origin information for the task
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<TaskOrigin>,
    /// Relationship information for the task
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relationships: Option<TaskRelationships>,
    /// Tool execution context for the task
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_context: Option<HashMap<String, Value>>,
}

/// Helper to store extension metadata in standard A2A metadata fields
pub fn store_extension_metadata(
    metadata: &mut Map<String, Value>,
    extension: &TaskExtensionMetadata,
) -> Result<(), crate::bidirectional_agent::error::AgentError> {
    match serde_json::to_value(extension) {
        Ok(Value::Object(ext_obj)) => {
            metadata.insert(METADATA_NAMESPACE.to_string(), Value::Object(ext_obj));
            Ok(())
        }
        Ok(_) => Err(crate::bidirectional_agent::error::AgentError::SerializationError(
            "Extension metadata must serialize to an object".to_string(),
        )),
        Err(e) => Err(crate::bidirectional_agent::error::AgentError::SerializationError(
            format!("Failed to serialize extension metadata: {}", e),
        )),
    }
}

/// Helper to extract extension metadata from standard A2A metadata fields
pub fn extract_extension_metadata(
    metadata: &Map<String, Value>,
) -> Result<TaskExtensionMetadata, crate::bidirectional_agent::error::AgentError> {
    if let Some(Value::Object(ext_obj)) = metadata.get(METADATA_NAMESPACE) {
        match serde_json::from_value::<TaskExtensionMetadata>(Value::Object(ext_obj.clone())) {
            Ok(extension) => Ok(extension),
            Err(e) => Err(crate::bidirectional_agent::error::AgentError::DeserializationError(
                format!("Failed to deserialize extension metadata: {}", e),
            )),
        }
    } else {
        // If no extension metadata is found, return the default empty structure
        Ok(TaskExtensionMetadata::default())
    }
}

/// Create a standard A2A data part containing a tool call
pub fn create_tool_call_part(
    tool_name: &str,
    params: Value,
    description: Option<&str>,
) -> DataPart {
    let tool_call = serde_json::json!({
        "name": tool_name,
        "params": params,
        "description": description
    });
    
    DataPart {
        type_: "data".to_string(),
        data: serde_json::json!({
            "contentType": "application/json",
            "kind": "toolCall",
            "content": tool_call
        }),
        metadata: None,
    }
}

/// Extracts tool call information from a message part
pub fn extract_tool_call(part: &Part) -> Option<(String, Value)> {
    match part {
        Part::DataPart(data_part) => {
            if let Ok(obj) = serde_json::from_value::<serde_json::Map<String, Value>>(data_part.data.clone()) {
                // Check if this is a tool call data part
                if obj.get("kind").and_then(|v| v.as_str()) == Some("toolCall") {
                    if let Some(content) = obj.get("content") {
                        // Extract name and params from content
                        if let Ok(tool_call) = serde_json::from_value::<serde_json::Map<String, Value>>(content.clone()) {
                            let name = tool_call.get("name").and_then(|v| v.as_str()).unwrap_or_default().to_string();
                            let params = tool_call.get("params").cloned().unwrap_or(Value::Null);
                            return Some((name, params));
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Format a tool call result as standard A2A parts (both text and data)
pub fn format_tool_call_result(tool_name: &str, result: &Value) -> Vec<Part> {
    let result_text = match result {
        Value::String(s) => s.clone(),
        _ => serde_json::to_string_pretty(result).unwrap_or_else(|_| format!("{:?}", result)),
    };
    
    // Create text part for human-readable representation
    let text_part = Part::TextPart(TextPart {
        type_: "text".to_string(),
        text: format!("Result from tool '{}': {}", tool_name, result_text),
        metadata: None,
    });
    
    // Create data part for machine-readable representation
    let data_part = Part::DataPart(DataPart {
        type_: "data".to_string(),
        data: serde_json::json!({
            "contentType": "application/json",
            "kind": "toolResult",
            "content": {
                "name": tool_name,
                "result": result
            }
        }),
        metadata: None,
    });
    
    vec![text_part, data_part]
}

/// Create a standard A2A text message from a string with specified role
pub fn create_text_message_with_role(role: Role, text: &str) -> Message {
    Message {
        role,
        parts: vec![Part::TextPart(TextPart {
            type_: "text".to_string(),
            text: text.to_string(),
            metadata: None,
        })],
        metadata: None,
    }
}

/// Create a standard A2A text message from a string (with Agent role)
pub fn create_text_message(text: &str) -> Message {
    create_text_message_with_role(Role::Agent, text)
}

/// Get a value from a namespaced metadata key
pub fn get_metadata_ext(metadata: &Map<String, Value>, key: &str) -> Option<Value> {
    if let Some(Value::Object(ext_obj)) = metadata.get(METADATA_NAMESPACE) {
        ext_obj.get(key).cloned()
    } else {
        None
    }
}

/// Set a value for a namespaced metadata key
pub fn set_metadata_ext(metadata: &mut Map<String, Value>, key: &str, value: Value) -> Result<(), crate::bidirectional_agent::error::AgentError> {
    // Get or create the namespace object
    let ext_obj = if let Some(Value::Object(obj)) = metadata.get_mut(METADATA_NAMESPACE) {
        obj
    } else {
        metadata.insert(METADATA_NAMESPACE.to_string(), Value::Object(Map::new()));
        if let Some(Value::Object(obj)) = metadata.get_mut(METADATA_NAMESPACE) {
            obj
        } else {
            return Err(crate::bidirectional_agent::error::AgentError::SerializationError(
                "Failed to create metadata namespace".to_string(),
            ));
        }
    };
    
    // Set the value
    ext_obj.insert(key.to_string(), value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Part;
    use chrono::Utc;
    use serde_json::json;
    
    #[test]
    fn test_store_extension_metadata() {
        // Create test extension metadata
        let extension = TaskExtensionMetadata {
            origin: Some(TaskOrigin::Local),
            relationships: Some(TaskRelationships {
                parent_task_ids: vec!["parent-1".to_string()],
                child_task_ids: vec!["child-1".to_string()],
                related_task_ids: HashMap::new(),
            }),
            tool_context: Some(HashMap::new()),
        };
        
        // Create test metadata map
        let mut metadata = Map::new();
        
        // Store extension metadata
        let result = store_extension_metadata(&mut metadata, &extension);
        
        // Verify result
        assert!(result.is_ok(), "Failed to store extension metadata: {:?}", result);
        assert!(metadata.contains_key(METADATA_NAMESPACE), "Metadata map does not contain extension namespace");
    }
    
    #[test]
    fn test_extract_extension_metadata() {
        // Create test metadata with extension data
        let mut metadata = Map::new();
        let extension = TaskExtensionMetadata {
            origin: Some(TaskOrigin::Delegated {
                agent_id: "agent-1".to_string(),
                agent_url: Some("http://localhost:8000".to_string()),
                delegated_at: Utc::now().to_rfc3339(),
            }),
            relationships: None,
            tool_context: None,
        };
        
        // Store extension in metadata
        let ext_value = serde_json::to_value(extension.clone()).unwrap();
        if let Value::Object(obj) = ext_value {
            metadata.insert(METADATA_NAMESPACE.to_string(), Value::Object(obj));
        }
        
        // Extract extension
        let result = extract_extension_metadata(&metadata);
        
        // Verify result
        assert!(result.is_ok(), "Failed to extract extension metadata: {:?}", result);
        let extracted = result.unwrap();
        
        if let Some(TaskOrigin::Delegated { agent_id, .. }) = extracted.origin {
            assert_eq!(agent_id, "agent-1", "Extracted agent_id doesn't match");
        } else {
            panic!("Extracted origin doesn't match expected type");
        }
    }
    
    #[test]
    fn test_create_tool_call_part() {
        let tool_name = "calculator";
        let params = json!({
            "operation": "add",
            "values": [5, 7]
        });
        let description = Some("Calculate the sum of two numbers");
        
        let part = create_tool_call_part(tool_name, params.clone(), description);
        
        // Verify part structure
        assert_eq!(part.type_, "data", "Part type should be 'data'");
        
        // Extract and verify the content
        if let Ok(obj) = serde_json::from_value::<serde_json::Map<String, Value>>(part.data.clone()) {
            assert_eq!(obj.get("contentType").and_then(|v| v.as_str()), Some("application/json"), "Content type should be application/json");
            assert_eq!(obj.get("kind").and_then(|v| v.as_str()), Some("toolCall"), "Kind should be toolCall");
            
            if let Some(content) = obj.get("content") {
                if let Ok(tool_call) = serde_json::from_value::<serde_json::Map<String, Value>>(content.clone()) {
                    assert_eq!(tool_call.get("name").and_then(|v| v.as_str()), Some(tool_name), "Tool name doesn't match");
                    assert_eq!(tool_call.get("params"), Some(&params), "Tool params don't match");
                    assert_eq!(tool_call.get("description").and_then(|v| v.as_str()), description, "Tool description doesn't match");
                } else {
                    panic!("Content is not a valid object");
                }
            } else {
                panic!("Missing content field");
            }
        } else {
            panic!("Part data is not a valid object");
        }
    }
    
    #[test]
    fn test_extract_tool_call() {
        // Create a tool call data part
        let data_part = DataPart {
            type_: "data".to_string(),
            data: json!({
                "contentType": "application/json",
                "kind": "toolCall",
                "content": {
                    "name": "http",
                    "params": {
                        "url": "https://example.com",
                        "method": "GET"
                    }
                }
            }),
            metadata: None,
        };
        
        let part = Part::DataPart(data_part);
        
        // Extract tool call
        let result = extract_tool_call(&part);
        
        // Verify result
        assert!(result.is_some(), "Failed to extract tool call");
        let (name, params) = result.unwrap();
        assert_eq!(name, "http", "Tool name doesn't match");
        assert_eq!(params, json!({
            "url": "https://example.com",
            "method": "GET"
        }), "Tool params don't match");
    }
    
    #[test]
    fn test_format_tool_call_result() {
        let tool_name = "http";
        let result = json!({
            "status": 200,
            "body": "Hello, world!"
        });
        
        let parts = format_tool_call_result(tool_name, &result);
        
        // Verify parts
        assert_eq!(parts.len(), 2, "Should create 2 parts");
        
        // Verify text part
        match &parts[0] {
            Part::TextPart(text_part) => {
                assert_eq!(text_part.type_, "text", "First part should be text");
                assert!(text_part.text.contains("Result from tool 'http'"), "Text part should contain tool name");
                assert!(text_part.text.contains("Hello, world!"), "Text part should contain result");
            },
            _ => panic!("First part should be a TextPart"),
        }
        
        // Verify data part
        match &parts[1] {
            Part::DataPart(data_part) => {
                assert_eq!(data_part.type_, "data", "Second part should be data");
                
                if let Ok(obj) = serde_json::from_value::<serde_json::Map<String, Value>>(data_part.data.clone()) {
                    assert_eq!(obj.get("kind").and_then(|v| v.as_str()), Some("toolResult"), "Kind should be toolResult");
                    
                    if let Some(content) = obj.get("content") {
                        if let Ok(tool_result) = serde_json::from_value::<serde_json::Map<String, Value>>(content.clone()) {
                            assert_eq!(tool_result.get("name").and_then(|v| v.as_str()), Some(tool_name), "Tool name doesn't match");
                            assert_eq!(tool_result.get("result"), Some(&result), "Tool result doesn't match");
                        } else {
                            panic!("Content is not a valid object");
                        }
                    } else {
                        panic!("Missing content field");
                    }
                } else {
                    panic!("Part data is not a valid object");
                }
            },
            _ => panic!("Second part should be a DataPart"),
        }
    }
}