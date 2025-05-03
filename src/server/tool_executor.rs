//! Executes local tools with standard A2A protocol-compliant types and artifacts.

use crate::server::error::ServerError;
use crate::types::{
    Task, TaskStatus, TaskState, Message, Role, Part, TextPart, DataPart, Artifact
};

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use chrono::Utc;
use async_trait::async_trait;

/// Error type specific to tool execution.
#[derive(thiserror::Error, Debug, Clone)]
pub enum ToolError {
    #[error("Tool '{0}' not found")]
    NotFound(String),

    #[error("Invalid parameters for tool '{0}': {1}")]
    InvalidParams(String, String),

    #[error("Tool execution failed for '{0}': {1}")]
    ExecutionFailed(String, String),

    #[error("Tool configuration error for '{0}': {1}")]
    ConfigError(String, String),

    #[error("Input/Output Error during tool execution: {0}")]
    IoError(String),
}

// Implement conversion from std::io::Error
impl From<std::io::Error> for ToolError {
    fn from(e: std::io::Error) -> Self {
        ToolError::IoError(e.to_string())
    }
}

// Implement conversion to ServerError
impl From<ToolError> for ServerError {
    fn from(error: ToolError) -> Self {
        match error {
            ToolError::NotFound(tool) => 
                ServerError::UnsupportedOperation(format!("Tool '{}' not found", tool)),
            ToolError::InvalidParams(tool, msg) => 
                ServerError::InvalidParameters(format!("Invalid parameters for tool '{}': {}", tool, msg)),
            ToolError::ExecutionFailed(tool, msg) => 
                ServerError::ServerTaskExecutionFailed(format!("Tool '{}' execution failed: {}", tool, msg)),
            ToolError::ConfigError(tool, msg) => 
                ServerError::ConfigError(format!("Tool '{}' configuration error: {}", tool, msg)),
            ToolError::IoError(msg) => 
                ServerError::ServerTaskExecutionFailed(format!("I/O error during tool execution: {}", msg)),
        }
    }
}

/// Tool interface for executors
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the name of the tool
    fn name(&self) -> &str;
    
    /// Returns a description of the tool
    fn description(&self) -> &str;
    
    /// Executes the tool with the given parameters
    async fn execute(&self, params: Value) -> Result<Value, ToolError>;
    
    /// Returns a list of capability tags for the tool
    fn capabilities(&self) -> &[&'static str];
}

/// Echo tool implementation for basic testing
pub struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str { "echo" }
    
    fn description(&self) -> &str { "Echoes back the input text" }
    
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        if let Some(text) = params.get("text").and_then(|v| v.as_str()) {
            Ok(json!(format!("Echo: {}", text)))
        } else {
            Err(ToolError::InvalidParams("echo".to_string(), "Missing 'text' parameter".to_string()))
        }
    }
    
    fn capabilities(&self) -> &[&'static str] { &["echo", "text_manipulation"] }
}

/// Manages and executes available local tools using standard A2A types.
#[derive(Clone)]
pub struct ToolExecutor {
    /// Registered tools that can be executed
    pub tools: Arc<HashMap<String, Box<dyn Tool>>>,
}

impl ToolExecutor {
    /// Creates a new ToolExecutor with just the basic echo tool.
    pub fn new() -> Self {
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();
        
        // Register the echo tool
        let echo_tool = EchoTool;
        tools.insert(echo_tool.name().to_string(), Box::new(echo_tool));
        
        Self {
            tools: Arc::new(tools),
        }
    }

    /// Executes a specific tool by name with the given JSON parameters.
    pub async fn execute_tool(&self, tool_name: &str, params: Value) -> Result<Value, ToolError> {
        match self.tools.get(tool_name) {
            Some(tool) => tool.execute(params).await,
            None => Err(ToolError::NotFound(tool_name.to_string())),
        }
    }

    /// Helper function to extract parameters from a message for a specific tool
    fn extract_params_from_message(&self, message: &Message) -> Value {
        // Try to extract parameters from text parts
        for part in &message.parts {
            if let Part::TextPart(tp) = part {
                // Check if the text could be JSON
                if tp.text.trim().starts_with('{') && tp.text.trim().ends_with('}') {
                    // Try to parse as JSON
                    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&tp.text) {
                        return json_val;
                    }
                }
                
                // Default to simple text param
                return json!({"text": tp.text});
            }
        }
        
        // Default to empty params if no suitable text part is found
        json!({})
    }

    /// Executes a task locally using the specified tool(s).
    pub async fn execute_task_locally(&self, task: &mut Task, tool_names: &[String]) -> Result<(), ServerError> {
        // Use the echo tool as a default fallback if no tool is specified
        let default_tool_name = "echo";
        
        // For now, assume only one tool is specified or use the first one.
        let requested_tool_name = match tool_names.first() {
            Some(name) => name.as_str(),
            None => {
                // Update task with warning about no tool specified
                let warning_msg = format!("No tool specified - using default '{}' tool instead.", default_tool_name);
                
                // Add this warning to task history for debugging
                let mut history = task.history.clone().unwrap_or_default();
                history.push(Message {
                    role: Role::Agent, // Using Agent role as system role isn't available
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: warning_msg.clone(),
                        metadata: None,
                    })],
                    metadata: None,
                });
                task.history = Some(history);
                
                // Use the default tool
                default_tool_name
            }
        };
        
        // Check if the requested tool exists
        let tool_name = if self.tools.contains_key(requested_tool_name) {
            requested_tool_name
        } else {
            // Tool doesn't exist, use the default tool with a warning
            let warning_msg = format!("Tool '{}' not found - using default '{}' tool instead.", 
                                     requested_tool_name, default_tool_name);
            
            // Add this warning to task history for debugging
            let mut history = task.history.clone().unwrap_or_default();
            history.push(Message {
                role: Role::Agent, // Using Agent role as system role isn't available
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: warning_msg.clone(),
                    metadata: None,
                })],
                metadata: None,
            });
            task.history = Some(history);
            
            // Use the default tool
            default_tool_name
        };

        // Extract parameters from the task message
        let params = if let Some(message) = &task.status.message {
            self.extract_params_from_message(message)
        } else {
            json!({})
        };

        // Execute the tool
        match self.execute_tool(tool_name, params).await {
            Ok(result_value) => {
                // Create a text part for the result
                let text_part = TextPart {
                    type_: "text".to_string(),
                    text: result_value.to_string(),
                    metadata: None,
                };
                
                // Create a data part for the result
                let data_part = DataPart {
                    type_: "json".to_string(),
                    data: {
                        let mut map = serde_json::Map::new();
                        if result_value.is_object() {
                            map = result_value.as_object().unwrap().clone();
                        } else {
                            map.insert("result".to_string(), result_value.clone());
                        }
                        map
                    },
                    metadata: None,
                };
                
                // Create an artifact from the parts
                let artifact = Artifact {
                    parts: vec![Part::TextPart(text_part), Part::DataPart(data_part)],
                    index: task.artifacts.as_ref().map_or(0, |a| a.len()) as i64, // Next index
                    name: Some(format!("{}_result", tool_name)),
                    description: Some(format!("Result from tool '{}'", tool_name)),
                    append: None,
                    last_chunk: Some(true), // Mark as last chunk
                    metadata: None,
                };

                // Add artifact to the task
                task.artifacts.get_or_insert_with(Vec::new).push(artifact);

                // Update Task Status to Completed
                task.status = TaskStatus {
                    state: TaskState::Completed,
                    timestamp: Some(Utc::now()),
                    // Provide a confirmation message from the agent
                    message: Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Task completed successfully using tool '{}'.", tool_name),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
                Ok(())
            }
            Err(tool_error) => {
                // Update Task Status to Failed
                task.status = TaskStatus {
                    state: TaskState::Failed,
                    timestamp: Some(Utc::now()),
                    // Provide an error message from the agent
                    message: Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Tool execution failed: {}", tool_error),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
                // Convert ToolError to ServerError before returning
                Err(tool_error.into())
            }
        }
    }
    
    /// Process a follow-up message using the tool
    pub async fn process_follow_up(&self, task: &mut Task, message: Message) -> Result<(), ServerError> {
        // Default to echo tool for follow-up processing
        let tool_name = "echo";
        
        // Extract parameters from the message
        let params = self.extract_params_from_message(&message);
        
        // Execute the tool
        match self.execute_tool(tool_name, params).await {
            Ok(result_value) => {
                // Create a text part for the result
                let text_part = TextPart {
                    type_: "text".to_string(),
                    text: result_value.to_string(),
                    metadata: None,
                };
                
                // Create an artifact with the result
                let artifact = Artifact {
                    parts: vec![Part::TextPart(text_part)],
                    index: task.artifacts.as_ref().map_or(0, |a| a.len()) as i64,
                    name: Some("follow_up_result".to_string()),
                    description: Some("Result from follow-up message processing".to_string()),
                    append: None,
                    last_chunk: Some(true),
                    metadata: None,
                };
                
                // Add artifact to the task
                task.artifacts.get_or_insert_with(Vec::new).push(artifact);
                
                // Update task status
                task.status = TaskStatus {
                    state: TaskState::Completed,
                    timestamp: Some(Utc::now()),
                    message: Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Follow-up processed successfully: {}", result_value),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
                Ok(())
            }
            Err(tool_error) => {
                // Update Task Status to Failed
                task.status = TaskStatus {
                    state: TaskState::Failed,
                    timestamp: Some(Utc::now()),
                    message: Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Follow-up processing failed: {}", tool_error),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
                Err(tool_error.into())
            }
        }
    }
}