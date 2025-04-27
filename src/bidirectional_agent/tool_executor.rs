//! Executes local tools based on task requirements.

#![cfg(feature = "bidir-local-exec")]

use crate::bidirectional_agent::error::AgentError;
use crate::types::{Task, TaskState, TaskStatus, Message, Role, Part, TextPart}; // Import necessary types
use serde_json::json;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result; // Use anyhow for tool execution results

/// Error type specific to tool execution.
#[derive(thiserror::Error, Debug)]
pub enum ToolError {
    #[error("Tool '{0}' not found")]
    NotFound(String),
    #[error("Invalid parameters for tool '{0}': {1}")]
    InvalidParams(String, String),
    #[error("Tool execution failed for '{0}': {1}")]
    ExecutionFailed(String, String),
    #[error("Tool configuration error for '{0}': {1}")]
    ConfigError(String, String),
}

// Implement conversion to AgentError
impl From<ToolError> for AgentError {
    fn from(error: ToolError) -> Self {
        AgentError::ToolError(error.to_string())
    }
}


/// Trait for tools that can be executed by the agent.
#[async_trait]
pub trait Tool: Send + Sync + 'static {
    /// Returns the unique name of the tool.
    fn name(&self) -> &str;
    /// Returns a description of what the tool does.
    fn description(&self) -> &str;
    /// Executes the tool with the given parameters.
    /// Parameters are typically derived from the task message parts.
    async fn execute(&self, params: Value) -> Result<Value, ToolError>;
    /// Returns a list of capabilities this tool provides (e.g., "shell_command", "http_request").
    fn capabilities(&self) -> &[&'static str];
}

/// Manages and executes available tools.
#[derive(Clone)]
pub struct ToolExecutor {
    tools: Arc<HashMap<String, Box<dyn Tool>>>,
}

impl ToolExecutor {
    /// Creates a new ToolExecutor and registers built-in tools.
    pub fn new() -> Self {
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();

        // Register built-in tools (implementations will be added later)
        // Example:
        // let shell_tool = crate::bidirectional_agent::tools::shell_tool::ShellTool::new();
        // tools.insert(shell_tool.name().to_string(), Box::new(shell_tool));
        println!("🔧 ToolExecutor initialized. (Tool registration placeholder)");

        Self {
            tools: Arc::new(tools),
        }
    }

    /// Executes a specific tool by name.
    pub async fn execute_tool(&self, tool_name: &str, params: Value) -> Result<Value, ToolError> {
        match self.tools.get(tool_name) {
            Some(tool) => tool.execute(params).await,
            None => Err(ToolError::NotFound(tool_name.to_string())),
        }
    }

    /// Executes a task locally using the appropriate tool(s).
    /// This is a simplified version for Slice 2. A more robust implementation
    /// would involve selecting tools based on task content/metadata.
    pub async fn execute_task_locally(&self, task: &mut Task) -> Result<(), AgentError> {
        println!("⚙️ Attempting local execution for task '{}'", task.id);

        // --- Simplified Tool Selection Logic for Slice 2 ---
        // In a real scenario, analyze task.message.parts, task.metadata etc.
        // For now, let's assume a simple "echo" tool if no specific tool is requested.
        let tool_name = "echo"; // Placeholder

        // --- Parameter Extraction (Simplified) ---
        // Extract text from the first part as parameter
        let params = task.history.as_ref()
            .and_then(|h| h.last()) // Get the latest message (usually the user input)
            .and_then(|msg| msg.parts.first())
            .and_then(|part| match part {
                Part::TextPart(tp) => Some(json!({"text": tp.text})),
                _ => None,
            })
            .unwrap_or_else(|| json!({"text": "No input provided"})); // Default params

        // --- Execute the selected tool ---
        match self.execute_tool(tool_name, params).await {
            Ok(result_value) => {
                println!("  ✅ Tool '{}' executed successfully.", tool_name);
                // --- Format Result into Task Artifact ---
                let result_text = result_value.as_str().unwrap_or("Tool returned non-text data").to_string();
                 let result_part = Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: result_text,
                    metadata: None,
                });
                let artifact = crate::types::Artifact {
                    parts: vec![result_part],
                    index: 0, // Assuming single artifact for now
                    name: Some(format!("{}_result", tool_name)),
                    description: Some(format!("Result of {} execution", tool_name)),
                    append: None,
                    last_chunk: None,
                    metadata: None,
                };
                task.artifacts = Some(vec![artifact]);

                // --- Update Task Status to Completed ---
                 task.status = TaskStatus {
                    state: TaskState::Completed,
                    timestamp: Some(chrono::Utc::now()),
                    message: Some(Message { // Add agent message confirming completion
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Local execution with tool '{}' completed.", tool_name),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
                Ok(())
            }
            Err(e) => {
                 println!("  ❌ Tool '{}' execution failed: {}", tool_name, e);
                // --- Update Task Status to Failed ---
                 task.status = TaskStatus {
                    state: TaskState::Failed,
                    timestamp: Some(chrono::Utc::now()),
                    message: Some(Message { // Add agent message explaining failure
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Local execution failed: {}", e),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
                // Convert ToolError to AgentError before returning
                Err(e.into())
            }
        }
    }

     /// Generates AgentSkill descriptions from registered tools.
     pub fn generate_agent_skills(&self) -> Vec<crate::types::AgentSkill> {
         self.tools.values().map(|tool| {
             crate::types::AgentSkill {
                 id: tool.name().to_string(),
                 name: tool.name().to_string(), // Use tool name as skill name for now
                 description: Some(tool.description().to_string()),
                 tags: Some(tool.capabilities().iter().map(|s| s.to_string()).collect()),
                 examples: None, // Add examples later if needed
                 input_modes: None, // Define based on tool params later
                 output_modes: None, // Define based on tool output later
             }
         }).collect()
     }
}

// Basic tests
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Mock Tool for testing
    struct MockEchoTool;

    #[async_trait]
    impl Tool for MockEchoTool {
        fn name(&self) -> &str { "echo" }
        fn description(&self) -> &str { "Echoes back the input text" }
        async fn execute(&self, params: Value) -> Result<Value, ToolError> {
            let text = params.get("text").and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::InvalidParams("echo".to_string(), "Missing 'text' parameter".to_string()))?;
            Ok(json!(format!("Echo: {}", text)))
        }
         fn capabilities(&self) -> &[&'static str] { &["echo_capability"] }
    }

     struct MockFailTool;

    #[async_trait]
    impl Tool for MockFailTool {
        fn name(&self) -> &str { "fail" }
        fn description(&self) -> &str { "Always fails execution" }
        async fn execute(&self, _params: Value) -> Result<Value, ToolError> {
            Err(ToolError::ExecutionFailed("fail".to_string(), "Simulated failure".to_string()))
        }
         fn capabilities(&self) -> &[&'static str] { &["fail_capability"] }
    }


    fn create_test_executor() -> ToolExecutor {
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();
        tools.insert("echo".to_string(), Box::new(MockEchoTool));
        tools.insert("fail".to_string(), Box::new(MockFailTool));
        ToolExecutor { tools: Arc::new(tools) }
    }

    #[tokio::test]
    async fn test_execute_tool_success() {
        let executor = create_test_executor();
        let params = json!({"text": "hello"});
        let result = executor.execute_tool("echo", params).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), json!("Echo: hello"));
    }

    #[tokio::test]
    async fn test_execute_tool_not_found() {
        let executor = create_test_executor();
        let params = json!({});
        let result = executor.execute_tool("unknown_tool", params).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::NotFound(name) => assert_eq!(name, "unknown_tool"),
            _ => panic!("Expected NotFound error"),
        }
    }

     #[tokio::test]
    async fn test_execute_tool_invalid_params() {
        let executor = create_test_executor();
        let params = json!({}); // Missing "text"
        let result = executor.execute_tool("echo", params).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidParams(name, msg) => {
                assert_eq!(name, "echo");
                assert!(msg.contains("Missing 'text' parameter"));
            },
            _ => panic!("Expected InvalidParams error"),
        }
    }

     #[tokio::test]
    async fn test_execute_tool_execution_failed() {
        let executor = create_test_executor();
        let params = json!({});
        let result = executor.execute_tool("fail", params).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionFailed(name, msg) => {
                 assert_eq!(name, "fail");
                 assert!(msg.contains("Simulated failure"));
            },
            _ => panic!("Expected ExecutionFailed error"),
        }
    }
}
//! Executes local tools based on task requirements.

// Only compile if local execution feature is enabled
#![cfg(feature = "bidir-local-exec")]

use crate::bidirectional_agent::error::AgentError;
use crate::types::{Task, TaskState, TaskStatus, Message, Role, Part, TextPart};
use crate::bidirectional_agent::tools::Tool; // Import the Tool trait
use serde_json::{json, Value};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use anyhow::Result; // Use anyhow for tool execution results

/// Error type specific to tool execution.
#[derive(thiserror::Error, Debug, Clone)] // Added Clone
pub enum ToolError {
    #[error("Tool '{0}' not found")]
    NotFound(String),
    #[error("Invalid parameters for tool '{0}': {1}")]
    InvalidParams(String, String),
    #[error("Tool execution failed for '{0}': {1}")]
    ExecutionFailed(String, String),
    #[error("Tool configuration error for '{0}': {1}")]
    ConfigError(String, String),
}

// Implement conversion to AgentError
impl From<ToolError> for AgentError {
    fn from(error: ToolError) -> Self {
        AgentError::ToolError(error.to_string())
    }
}

/// Manages and executes available tools.
#[derive(Clone)]
pub struct ToolExecutor {
    // Use Arc for shared ownership, allowing ToolExecutor to be Clone
    tools: Arc<HashMap<String, Box<dyn Tool>>>,
}

impl ToolExecutor {
    /// Creates a new ToolExecutor and registers built-in tools.
    pub fn new() -> Self {
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();

        // Register built-in tools
        let shell_tool = crate::bidirectional_agent::tools::ShellTool::new();
        tools.insert(shell_tool.name().to_string(), Box::new(shell_tool));

        let http_tool = crate::bidirectional_agent::tools::HttpTool::new();
        tools.insert(http_tool.name().to_string(), Box::new(http_tool));

        println!("🔧 ToolExecutor initialized with tools: {:?}", tools.keys());

        Self {
            tools: Arc::new(tools),
        }
    }

    /// Executes a specific tool by name.
    pub async fn execute_tool(&self, tool_name: &str, params: Value) -> Result<Value, ToolError> {
        match self.tools.get(tool_name) {
            Some(tool) => tool.execute(params).await,
            None => Err(ToolError::NotFound(tool_name.to_string())),
        }
    }

    /// Executes a task locally using the appropriate tool(s).
    /// This is a simplified version for Slice 2. A more robust implementation
    /// would involve selecting tools based on task content/metadata.
    pub async fn execute_task_locally(&self, task: &mut Task) -> Result<(), AgentError> {
        println!("⚙️ Attempting local execution for task '{}'", task.id);

        // --- Simplified Tool Selection Logic for Slice 2 ---
        // Extract tool name from metadata if present, otherwise default to "echo"
        let tool_name = task.metadata.as_ref()
            .and_then(|m| m.get("_tool_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("echo"); // Default to echo if not specified

        // --- Parameter Extraction (Simplified) ---
        // Extract text from the first part as parameter
        let params = task.history.as_ref()
            .and_then(|h| h.last()) // Get the latest message (usually the user input)
            .and_then(|msg| msg.parts.first())
            .and_then(|part| match part {
                Part::TextPart(tp) => Some(json!({"text": tp.text})),
                // Add handling for other part types if needed
                _ => None,
            })
            .unwrap_or_else(|| json!({"text": "No input provided"})); // Default params

        // --- Execute the selected tool ---
        match self.execute_tool(tool_name, params).await {
            Ok(result_value) => {
                println!("  ✅ Tool '{}' executed successfully.", tool_name);
                // --- Format Result into Task Artifact ---
                // Try to get string representation, otherwise use debug format
                let result_text = result_value.as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("{:?}", result_value));

                 let result_part = Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: result_text,
                    metadata: None,
                });
                let artifact = crate::types::Artifact {
                    parts: vec![result_part],
                    index: 0, // Assuming single artifact for now
                    name: Some(format!("{}_result", tool_name)),
                    description: Some(format!("Result of {} execution", tool_name)),
                    append: None,
                    last_chunk: None,
                    metadata: None,
                };
                // Add artifact to the task
                task.artifacts.get_or_insert_with(Vec::new).push(artifact);


                // --- Update Task Status to Completed ---
                 task.status = TaskStatus {
                    state: TaskState::Completed,
                    timestamp: Some(chrono::Utc::now()),
                    message: Some(Message { // Add agent message confirming completion
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Local execution with tool '{}' completed.", tool_name),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
                Ok(())
            }
            Err(e) => {
                 println!("  ❌ Tool '{}' execution failed: {}", tool_name, e);
                // --- Update Task Status to Failed ---
                 task.status = TaskStatus {
                    state: TaskState::Failed,
                    timestamp: Some(chrono::Utc::now()),
                    message: Some(Message { // Add agent message explaining failure
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Local execution failed: {}", e),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
                // Convert ToolError to AgentError before returning
                Err(e.into())
            }
        }
    }

     /// Generates AgentSkill descriptions from registered tools.
     pub fn generate_agent_skills(&self) -> Vec<crate::types::AgentSkill> {
         self.tools.values().map(|tool| {
             crate::types::AgentSkill {
                 id: tool.name().to_string(),
                 name: tool.name().to_string(), // Use tool name as skill name for now
                 description: Some(tool.description().to_string()),
                 tags: Some(tool.capabilities().iter().map(|s| s.to_string()).collect()),
                 examples: None, // Add examples later if needed
                 input_modes: None, // Define based on tool params later
                 output_modes: None, // Define based on tool output later
             }
         }).collect()
     }
}

// Basic tests
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Mock Tool for testing
    struct MockEchoTool;

    #[async_trait]
    impl Tool for MockEchoTool {
        fn name(&self) -> &str { "echo" }
        fn description(&self) -> &str { "Echoes back the input text" }
        async fn execute(&self, params: Value) -> Result<Value, ToolError> {
            let text = params.get("text").and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::InvalidParams("echo".to_string(), "Missing 'text' parameter".to_string()))?;
            Ok(json!(format!("Echo: {}", text)))
        }
         fn capabilities(&self) -> &[&'static str] { &["echo_capability"] }
    }

     struct MockFailTool;

    #[async_trait]
    impl Tool for MockFailTool {
        fn name(&self) -> &str { "fail" }
        fn description(&self) -> &str { "Always fails execution" }
        async fn execute(&self, _params: Value) -> Result<Value, ToolError> {
            Err(ToolError::ExecutionFailed("fail".to_string(), "Simulated failure".to_string()))
        }
         fn capabilities(&self) -> &[&'static str] { &["fail_capability"] }
    }


    fn create_test_executor() -> ToolExecutor {
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();
        tools.insert("echo".to_string(), Box::new(MockEchoTool));
        tools.insert("fail".to_string(), Box::new(MockFailTool));
        ToolExecutor { tools: Arc::new(tools) }
    }

    #[tokio::test]
    async fn test_execute_tool_success() {
        let executor = create_test_executor();
        let params = json!({"text": "hello"});
        let result = executor.execute_tool("echo", params).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), json!("Echo: hello"));
    }

    #[tokio::test]
    async fn test_execute_tool_not_found() {
        let executor = create_test_executor();
        let params = json!({});
        let result = executor.execute_tool("unknown_tool", params).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::NotFound(name) => assert_eq!(name, "unknown_tool"),
            _ => panic!("Expected NotFound error"),
        }
    }

     #[tokio::test]
    async fn test_execute_tool_invalid_params() {
        let executor = create_test_executor();
        let params = json!({}); // Missing "text"
        let result = executor.execute_tool("echo", params).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidParams(name, msg) => {
                assert_eq!(name, "echo");
                assert!(msg.contains("Missing 'text' parameter"));
            },
            _ => panic!("Expected InvalidParams error"),
        }
    }

     #[tokio::test]
    async fn test_execute_tool_execution_failed() {
        let executor = create_test_executor();
        let params = json!({});
        let result = executor.execute_tool("fail", params).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionFailed(name, msg) => {
                 assert_eq!(name, "fail");
                 assert!(msg.contains("Simulated failure"));
            },
            _ => panic!("Expected ExecutionFailed error"),
        }
    }
}
