//! Executes local tools with standard A2A protocol-compliant types and artifacts.

use crate::bidirectional_agent::error::AgentError;
use crate::bidirectional_agent::tools::Tool;
use crate::bidirectional_agent::types::{create_tool_call_part, format_tool_call_result};
use crate::types::{
    Task, TaskStatus, TaskState, Message, Role, Part, TextPart, DataPart, Artifact
};

use crate::bidirectional_agent::agent_directory::AgentDirectory;

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use chrono::Utc;
use log::{debug, info, warn, error};

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

// Implement conversion to AgentError
impl From<ToolError> for AgentError {
    fn from(error: ToolError) -> Self {
        match error {
            ToolError::NotFound(tool) => 
                AgentError::UnsupportedOperation(format!("Tool '{}' not found", tool)),
            ToolError::InvalidParams(tool, msg) => 
                AgentError::InvalidParameters(format!("Invalid parameters for tool '{}': {}", tool, msg)),
            ToolError::ExecutionFailed(tool, msg) => 
                AgentError::Internal(format!("Tool '{}' execution failed: {}", tool, msg)),
            ToolError::ConfigError(tool, msg) => 
                AgentError::ConfigError(format!("Tool '{}' configuration error: {}", tool, msg)),
            ToolError::IoError(msg) => 
                AgentError::Internal(format!("I/O error during tool execution: {}", msg)),
        }
    }
}

/// Manages and executes available local tools using standard A2A types.
#[derive(Clone)]
pub struct ToolExecutor {
    /// Registered tools that can be executed
    pub tools: Arc<HashMap<String, Box<dyn Tool>>>,
}

impl ToolExecutor {
    /// Creates a new ToolExecutor and registers available tools.
    /// Requires AgentDirectory if the directory tool is enabled.
    pub fn new(
        agent_directory: Arc<AgentDirectory>,
        remote_tool_executor: Option<Arc<crate::bidirectional_agent::RemoteToolExecutor>>
    ) -> Self {
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();

        // Register standard tools
        let shell_tool = crate::bidirectional_agent::tools::ShellTool::new();
        tools.insert(shell_tool.name().to_string(), Box::new(shell_tool));

        let http_tool = crate::bidirectional_agent::tools::HttpTool::new();
        tools.insert(http_tool.name().to_string(), Box::new(http_tool));

        // Register the DirectoryTool
        let directory_tool = crate::bidirectional_agent::tools::DirectoryTool::new(agent_directory);
        tools.insert(directory_tool.name().to_string(), Box::new(directory_tool));

        // Register special test tools
        let special_echo_1 = crate::bidirectional_agent::tools::SpecialEchoTool1;
        tools.insert(special_echo_1.name().to_string(), Box::new(special_echo_1));
        
        let special_echo_2 = crate::bidirectional_agent::tools::SpecialEchoTool2;
        tools.insert(special_echo_2.name().to_string(), Box::new(special_echo_2));
        
        // Register the RemoteToolExecutor if available
        if let Some(executor) = remote_tool_executor {
            tools.insert(executor.name().to_string(), Box::new(executor.clone()));
        }

        info!(target: "tool_executor", "Initialized with tools: {:?}", tools.keys());

        Self {
            tools: Arc::new(tools),
        }
    }
    
    /// Add a tool to the executor
    /// This method can be used to add tools after initialization
    pub fn add_tool(&mut self, tool: Box<dyn Tool>) {
        let name = tool.name().to_string();
        let mut tools_map = HashMap::new();
        
        // Copy existing tools from the Arc
        for (tool_name, tool_impl) in self.tools.as_ref() {
            tools_map.insert(tool_name.clone(), dyn_clone::clone_box(&**tool_impl));
        }
        
        // Add the new tool
        tools_map.insert(name.clone(), tool);
        
        // Update tools with the new map
        self.tools = Arc::new(tools_map);
        
        info!(target: "tool_executor", "Added tool: {}", name);
    }
    
    /// Register remote tools
    /// This method is used to register tools provided by remote agents
    pub fn register_remote_tools(&mut self, registry: &crate::bidirectional_agent::RemoteToolRegistry, 
                             executor: Arc<crate::bidirectional_agent::RemoteToolExecutor>) {
        use crate::bidirectional_agent::tools::pluggable::RemoteToolWrapper;
        
        let all_tools = registry.get_all_tools();
        let mut tools_map = HashMap::new();
        
        // Copy existing tools from the Arc
        for (tool_name, tool_impl) in self.tools.as_ref() {
            tools_map.insert(tool_name.clone(), dyn_clone::clone_box(&**tool_impl));
        }
        
        // Add all remote tools
        for (agent_id, tools) in all_tools {
            for tool_info in tools {
                let wrapper = RemoteToolWrapper::new(executor.clone(), tool_info.clone());
                let name = wrapper.name().to_string();
                tools_map.insert(name.clone(), Box::new(wrapper) as Box<dyn Tool>);
                info!(target: "tool_executor", "Registered remote tool '{}' from agent '{}'", name, agent_id);
            }
        }
        
        // Update tools with the new map
        self.tools = Arc::new(tools_map);
    }

    /// Executes a specific tool by name with the given JSON parameters.
    pub async fn execute_tool(&self, tool_name: &str, params: Value) -> Result<Value, ToolError> {
        debug!(target: "tool_executor", "Executing tool: {} with params: {}", tool_name, params);
        
        // Regular tool handling
        match self.tools.get(tool_name) {
            Some(tool) => {
                tool.execute(params.clone()).await.map_err(|e| {
                    // Log the specific tool error before returning
                    error!(target: "tool_executor", "Tool execution failed for {}: {:?}", tool_name, e);
                    e // Return the original ToolError
                })
            },
            None => {
                // If the tool isn't found directly, check if it's a remote tool via the remote_tool_executor
                if let Some(executor) = self.tools.get("remote_tool_executor") {
                    // Wrap the parameters to include tool name
                    let mut remote_params = serde_json::Map::new();
                    remote_params.insert("tool_name".to_string(), Value::String(tool_name.to_string()));
                    remote_params.insert("params".to_string(), params);
                    
                    // Try executing via the remote tool executor
                    info!(target: "tool_executor", "Attempting to execute '{}' as a remote tool", tool_name);
                    executor.execute(Value::Object(remote_params)).await
                } else {
                    // No remote tool executor available
                    error!(target: "tool_executor", "Tool not found and no remote tool executor available: {}", tool_name);
                    Err(ToolError::NotFound(tool_name.to_string()))
                }
            }
        }
    }

    /// Executes a task locally using the specified tool(s).
    /// This implementation creates standardized A2A-compliant artifacts with proper
    /// text and data parts.
    pub async fn execute_task_locally(&self, task: &mut Task, tool_names: &[String]) -> Result<(), AgentError> {
        // For now, assume only one tool is specified or use the first one.
        let tool_name = match tool_names.first() {
            Some(name) => name.as_str(),
            None => {
                error!(target: "tool_executor", "No tool specified for local execution for task {}", task.id);
                // Update task status to Failed
                task.status = TaskStatus {
                    state: TaskState::Failed,
                    timestamp: Some(Utc::now()),
                    message: Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: "Local execution failed: No tool specified.".to_string(),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
                return Err(AgentError::ToolError("No tool specified for local execution".to_string()));
            }
        };

        info!(target: "tool_executor", "Attempting local execution of task {} with tool {}", task.id, tool_name);

        // --- Extract Tool Call Parameters ---
        // First, try to find a tool call in the message parts
        let params = if let Some(history) = &task.history {
            if let Some(last_message) = history.last() {
                // Check if any part contains a tool call
                let mut tool_call_params = None;
                
                for part in &last_message.parts {
                    if let Some((name, params)) = crate::bidirectional_agent::types::extract_tool_call(part) {
                        if name == tool_name {
                            tool_call_params = Some(params);
                            break;
                        }
                    }
                }
                
                // If we found a tool call, use its parameters
                if let Some(params) = tool_call_params {
                    params
                } else {
                    // Otherwise, extract from text parts as a fallback
                    self.extract_params_from_message(last_message, tool_name)
                }
            } else {
                // No messages in history, use empty params
                json!({})
            }
        } else {
            // No history, use empty params
            json!({})
        };

        // --- Execute the Tool ---
        match self.execute_tool(tool_name, params).await {
            Ok(result_value) => {
                info!(target: "tool_executor", "Tool {} executed successfully for task {}", tool_name, task.id);

                // --- Create A2A-compliant parts from the tool result ---
                // Use the helper function to create both text and data parts
                let result_parts = format_tool_call_result(tool_name, &result_value);
                
                // Create an artifact from the parts
                let artifact = Artifact {
                    parts: result_parts,
                    index: task.artifacts.as_ref().map_or(0, |a| a.len()) as i64, // Next index
                    name: Some(format!("{}_result", tool_name)),
                    description: Some(format!("Result from tool '{}'", tool_name)),
                    append: None,
                    last_chunk: Some(true), // Mark as last chunk
                    metadata: None,
                };

                // Add artifact to the task
                task.artifacts.get_or_insert_with(Vec::new).push(artifact);

                // --- Update Task Status to Completed ---
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
                error!(target: "tool_executor", "Tool {} execution failed for task {}: {:?}", tool_name, task.id, tool_error);
                // --- Update Task Status to Failed ---
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
                // Convert ToolError to AgentError before returning
                Err(tool_error.into())
            }
        }
    }

    /// Helper function to extract parameters from a message for a specific tool
    fn extract_params_from_message(&self, message: &Message, tool_name: &str) -> Value {
        // Try to extract parameters from text parts
        for part in &message.parts {
            if let Part::TextPart(tp) = part {
                // Special case for directory tool
                if tool_name == "directory" {
                    let lower_text = tp.text.to_lowercase();
                    if lower_text.contains("list_active") || lower_text.contains("list active") {
                        return json!({"action": "list_active"});
                    } else if lower_text.contains("list_inactive") || lower_text.contains("list inactive") {
                        return json!({"action": "list_inactive"});
                    } else {
                        // Default directory action
                        return json!({"action": "list_active"});
                    }
                }
                
                // Check if the text could be JSON
                if tp.text.trim().starts_with('{') && tp.text.trim().ends_with('}') {
                    // Try to parse as JSON
                    if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&tp.text) {
                        debug!(target: "tool_executor", "Extracted JSON parameters from text: {}", json_val);
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

    /// Generate AgentSkill descriptions from registered tools.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bidirectional_agent::tools::Tool;
    use serde_json::json;
    use std::sync::Arc;
    
    // --- Mock Tools for Testing ---
    struct MockEchoTool;
    #[async_trait::async_trait]
    impl Tool for MockEchoTool {
        fn name(&self) -> &str { "echo" }
        fn description(&self) -> &str { "Echoes back the input text" }
        async fn execute(&self, params: Value) -> Result<Value, ToolError> {
            let text = params.get("text").and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::InvalidParams("echo".to_string(), "Missing 'text' parameter".to_string()))?;
            Ok(json!(format!("Echo: {}", text))) // Return simple JSON string
        }
        fn capabilities(&self) -> &[&'static str] { &["echo", "text_manipulation"] }
    }

    struct MockFailTool;
    #[async_trait::async_trait]
    impl Tool for MockFailTool {
        fn name(&self) -> &str { "fail" }
        fn description(&self) -> &str { "Always fails execution" }
        async fn execute(&self, _params: Value) -> Result<Value, ToolError> {
            Err(ToolError::ExecutionFailed("fail".to_string(), "Simulated tool execution failure".to_string()))
        }
        fn capabilities(&self) -> &[&'static str] { &["testing", "failure_simulation"] }
    }

    // --- Test Setup ---
    fn create_test_executor_with_mocks() -> ToolExecutor {
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();
        tools.insert("echo".to_string(), Box::new(MockEchoTool));
        tools.insert("fail".to_string(), Box::new(MockFailTool));
        ToolExecutor { tools: Arc::new(tools) }
    }

    #[tokio::test]
    async fn test_execute_tool_echo_success() {
        let executor = create_test_executor_with_mocks();
        let params = json!({"text": "world"});
        let result = executor.execute_tool("echo", params).await;
        assert!(result.is_ok(), "Echo tool failed: {:?}", result.err());
        assert_eq!(result.unwrap(), json!("Echo: world"));
    }

    #[tokio::test]
    async fn test_execute_tool_fail_error() {
        let executor = create_test_executor_with_mocks();
        let params = json!({});
        let result = executor.execute_tool("fail", params).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ExecutionFailed(name, msg) => {
                assert_eq!(name, "fail");
                assert!(msg.contains("Simulated tool execution failure"));
            },
            e => panic!("Expected ExecutionFailed error, got {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_execute_task_locally_with_tool_call() {
        let executor = create_test_executor_with_mocks();
        
        // Create a task with a tool call in DataPart format
        let tool_call_part = create_tool_call_part("echo", json!({"text": "world"}), Some("Echo the text"));
        
        let mut task = Task {
            id: "task-with-tool-call".to_string(),
            session_id: Some("test-session".to_string()),
            history: Some(vec![Message {
                role: Role::User,
                parts: vec![
                    // Include both text part and data part
                    Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: "Please echo 'world'".to_string(),
                        metadata: None,
                    }),
                    Part::DataPart(tool_call_part),
                ],
                metadata: None,
            }]),
            status: TaskStatus {
                state: TaskState::Working,
                timestamp: Some(Utc::now()),
                message: None,
            },
            artifacts: None,
            metadata: None,
        };
        
        // Execute task locally
        let result = executor.execute_task_locally(&mut task, &["echo".to_string()]).await;
        
        // Check result
        assert!(result.is_ok(), "Task execution failed: {:?}", result.err());
        assert_eq!(task.status.state, TaskState::Completed);
        
        // Check artifacts
        assert!(task.artifacts.is_some());
        let artifacts = task.artifacts.unwrap();
        assert_eq!(artifacts.len(), 1);
        
        // There should be multiple parts in the artifact (text and data)
        assert!(artifacts[0].parts.len() >= 2);
        
        // Check text part
        let has_text_part = artifacts[0].parts.iter().any(|part| {
            if let Part::TextPart(text_part) = part {
                text_part.text.contains("Echo: world")
            } else {
                false
            }
        });
        assert!(has_text_part, "No text part with expected content found in artifact");
        
        // Check data part 
        let has_data_part = artifacts[0].parts.iter().any(|part| {
            if let Part::DataPart(_) = part {
                true
            } else {
                false
            }
        });
        assert!(has_data_part, "No data part found in artifact");
    }

    #[tokio::test]
    async fn test_execute_task_locally_from_text() {
        let executor = create_test_executor_with_mocks();
        
        // Create a task with only a text message (no explicit tool call)
        let mut task = Task {
            id: "task-with-text".to_string(),
            session_id: Some("test-session".to_string()),
            history: Some(vec![Message {
                role: Role::User,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "Please echo this message".to_string(),
                    metadata: None,
                })],
                metadata: None,
            }]),
            status: TaskStatus {
                state: TaskState::Working,
                timestamp: Some(Utc::now()),
                message: None,
            },
            artifacts: None,
            metadata: None,
        };
        
        // Execute task locally
        let result = executor.execute_task_locally(&mut task, &["echo".to_string()]).await;
        
        // Check result
        assert!(result.is_ok(), "Task execution failed: {:?}", result.err());
        assert_eq!(task.status.state, TaskState::Completed);
        
        // Check artifacts
        assert!(task.artifacts.is_some());
        let artifacts = task.artifacts.unwrap();
        assert_eq!(artifacts.len(), 1);
        
        // Check content of the artifact
        let contains_echo = artifacts[0].parts.iter().any(|part| {
            if let Part::TextPart(text_part) = part {
                text_part.text.contains("Echo: Please echo this message")
            } else {
                false
            }
        });
        assert!(contains_echo, "Expected artifact to contain 'Echo: Please echo this message'");
    }

    #[tokio::test]
    async fn test_execute_task_locally_failure() {
        let executor = create_test_executor_with_mocks();
        
        // Create a simple task 
        let mut task = Task {
            id: "task-with-failure".to_string(),
            session_id: Some("test-session".to_string()),
            history: Some(vec![Message {
                role: Role::User,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "This should fail".to_string(),
                    metadata: None,
                })],
                metadata: None,
            }]),
            status: TaskStatus {
                state: TaskState::Working,
                timestamp: Some(Utc::now()),
                message: None,
            },
            artifacts: None,
            metadata: None,
        };
        
        // Execute task with the fail tool
        let result = executor.execute_task_locally(&mut task, &["fail".to_string()]).await;
        
        // Check that execution failed
        assert!(result.is_err(), "Expected task execution to fail");
        assert_eq!(task.status.state, TaskState::Failed);
        
        // Check error message
        let error_message = match &task.status.message {
            Some(message) => match &message.parts[0] {
                Part::TextPart(text_part) => &text_part.text,
                _ => "",
            },
            None => "",
        };
        assert!(error_message.contains("Tool execution failed"), "Error message doesn't contain expected content");
        assert!(error_message.contains("Simulated tool execution failure"), "Error message doesn't contain failure reason");
    }
}