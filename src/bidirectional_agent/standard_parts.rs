/// Standard parts handling for A2A protocol messages
///
/// This module provides utilities for working with standardized A2A message parts,
/// including tool calls, responses, and other structured formats.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use uuid::Uuid;

use crate::types::{Part, DataPart, Message, Role};

/// Standard tool call structure for A2A messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardToolCall {
    /// Name of the tool
    pub name: String,
    
    /// Parameters for the tool call
    pub parameters: Value,
    
    /// ID for the tool call
    pub id: String,
    
    /// Optional metadata for the tool call
    pub metadata: Option<HashMap<String, Value>>,
}

impl StandardToolCall {
    /// Create a new standard tool call
    pub fn new(name: &str, parameters: Value) -> Self {
        Self {
            name: name.to_string(),
            parameters,
            id: format!("toolcall-{}", Uuid::new_v4()),
            metadata: None,
        }
    }
    
    /// Convert to an A2A DataPart
    pub fn to_data_part(&self) -> DataPart {
        DataPart {
            type_: "tool_call".to_string(),
            mime_type: "application/json".to_string(),
            data: json!({
                "name": self.name,
                "parameters": self.parameters,
                "id": self.id,
                "metadata": self.metadata,
            }),
            metadata: None,
        }
    }
}

/// Format a tool call message with the given tool calls
pub fn format_tool_call_message(tool_calls: Vec<StandardToolCall>) -> Message {
    let mut parts = Vec::new();
    
    for tool_call in tool_calls {
        parts.push(Part::DataPart(tool_call.to_data_part()));
    }
    
    Message {
        role: Role::Agent,
        parts,
        metadata: None,
    }
}

/// Extract tool calls from message parts
pub fn extract_tool_calls_from_parts(parts: &[Part]) -> Vec<StandardToolCall> {
    let mut tool_calls = Vec::new();
    
    for part in parts {
        if let Part::DataPart(data_part) = part {
            if data_part.type_ == "tool_call" {
                if let Some(name) = data_part.data.get("name").and_then(|v| v.as_str()) {
                    if let Some(parameters) = data_part.data.get("parameters") {
                        let id = data_part.data.get("id")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("toolcall-{}", Uuid::new_v4()));
                            
                        let metadata = data_part.data.get("metadata")
                            .and_then(|v| v.as_object())
                            .map(|obj| {
                                obj.iter()
                                    .map(|(k, v)| (k.clone(), v.clone()))
                                    .collect::<HashMap<String, Value>>()
                            });
                            
                        tool_calls.push(StandardToolCall {
                            name: name.to_string(),
                            parameters: parameters.clone(),
                            id,
                            metadata,
                        });
                    }
                }
            }
        }
    }
    
    tool_calls
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    
    #[test]
    fn test_standard_tool_call() {
        let tool_call = StandardToolCall::new("test_tool", json!({
            "param1": "value1",
            "param2": 42
        }));
        
        assert_eq!(tool_call.name, "test_tool");
        assert_eq!(tool_call.parameters["param1"], "value1");
        assert_eq!(tool_call.parameters["param2"], 42);
        assert!(tool_call.id.starts_with("toolcall-"));
    }
    
    #[test]
    fn test_tool_call_to_data_part() {
        let tool_call = StandardToolCall::new("test_tool", json!({
            "param1": "value1",
            "param2": 42
        }));
        
        let data_part = tool_call.to_data_part();
        
        assert_eq!(data_part.type_, "tool_call");
        assert_eq!(data_part.mime_type, "application/json");
        assert_eq!(data_part.data["name"], "test_tool");
        assert_eq!(data_part.data["parameters"]["param1"], "value1");
        assert_eq!(data_part.data["parameters"]["param2"], 42);
    }
    
    #[test]
    fn test_format_tool_call_message() {
        let tool_call = StandardToolCall::new("test_tool", json!({
            "param1": "value1",
            "param2": 42
        }));
        
        let message = format_tool_call_message(vec![tool_call]);
        
        assert_eq!(message.role, Role::Assistant);
        assert_eq!(message.parts.len(), 1);
        
        if let Part::DataPart(data_part) = &message.parts[0] {
            assert_eq!(data_part.type_, "tool_call");
            assert_eq!(data_part.data["name"], "test_tool");
        } else {
            panic!("Expected DataPart");
        }
    }
    
    #[test]
    fn test_extract_tool_calls() {
        let tool_call = StandardToolCall::new("test_tool", json!({
            "param1": "value1",
            "param2": 42
        }));
        
        let data_part = tool_call.to_data_part();
        let part = Part::DataPart(data_part);
        
        let extracted = extract_tool_calls_from_parts(&[part]);
        
        assert_eq!(extracted.len(), 1);
        assert_eq!(extracted[0].name, "test_tool");
        assert_eq!(extracted[0].parameters["param1"], "value1");
        assert_eq!(extracted[0].parameters["param2"], 42);
    }
}