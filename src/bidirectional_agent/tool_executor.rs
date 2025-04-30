//! Executes local tools based on task requirements.

#![cfg(feature = "bidir-local-exec")]

use crate::{
    bidirectional_agent::{
        error::AgentError,
        tools::Tool, // Import the Tool trait from the tools module

    },
    // Import ToolCall and ToolCallPart
    types::{Task, TaskState, TaskStatus, Message, Role, Part, TextPart, Artifact, ToolCall, ToolCallPart},
};

#[cfg(feature = "bidir-core")] // Conditionally import AgentDirectory
use crate::bidirectional_agent::agent_directory::AgentDirectory;
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc};
// Remove anyhow::Result if Tool::execute returns Result<Value, ToolError>
// Remove async_trait import as Tool trait is now in tools/mod.rs

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
    #[error("Input/Output Error during tool execution: {0}")] // Add IO Error variant if tools interact with FS etc.
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
        // Provide more context in the AgentError if possible
        AgentError::ToolError(format!("Tool execution failed: {}", error))
    }
}

/// Manages and executes available local tools.
#[derive(Clone)]
pub struct ToolExecutor {
    // Store tools in an Arc<HashMap> for thread-safe sharing and cloning.
    // Use Box<dyn Tool> for dynamic dispatch.
    tools: Arc<HashMap<String, Box<dyn Tool>>>,
}

impl ToolExecutor {
    /// Creates a new ToolExecutor and registers available tools.
    /// Requires AgentDirectory if the directory tool is enabled.
    pub fn new(
        #[cfg(feature = "bidir-core")] // Only require directory if feature is enabled
        agent_directory: Arc<AgentDirectory>
    ) -> Self {
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();

        // Register standard tools
        let shell_tool = crate::bidirectional_agent::tools::ShellTool::new();
        tools.insert(shell_tool.name().to_string(), Box::new(shell_tool));

        let http_tool = crate::bidirectional_agent::tools::HttpTool::new();
        tools.insert(http_tool.name().to_string(), Box::new(http_tool));

        // Register the DirectoryTool conditionally
        #[cfg(feature = "bidir-core")]
        {
            let directory_tool = crate::bidirectional_agent::tools::DirectoryTool::new(agent_directory);
            tools.insert(directory_tool.name().to_string(), Box::new(directory_tool));
        }

        log::info!(target: "tool_executor", "Initialized with tools: {:?}", tools.keys());

        Self {
            tools: Arc::new(tools),
        }
    }

    /// Executes a specific tool by name with the given JSON parameters.
    pub async fn execute_tool(&self, tool_name: &str, params: Value) -> Result<Value, ToolError> {
        log::debug!(target: "tool_executor", tool=%tool_name, params=%params, "Executing tool");
        match self.tools.get(tool_name) {
            Some(tool) => {
                tool.execute(params).await.map_err(|e| {
                    // Log the specific tool error before returning
                    log::error!(target: "tool_executor", tool=%tool_name, error=?e, "Tool execution failed");
                    e // Return the original ToolError
                })
            },
            None => {
                 log::error!(target: "tool_executor", tool=%tool_name, "Tool not found");
                 Err(ToolError::NotFound(tool_name.to_string()))
            }
        }
    }

    /// Executes a task locally using the specified tool(s).
    /// Assumes routing has already determined this task is for local execution
    /// and identified the necessary tool(s).
    ///
    /// Currently handles only a single tool execution per task for simplicity.
    pub async fn execute_task_locally(&self, task: &mut Task, tool_names: &[String]) -> Result<(), AgentError> {
        // For now, assume only one tool is specified or use the first one.
        // TODO: Handle multiple tool executions if needed (chaining, parallel?).
        let tool_name = match tool_names.first() {
            Some(name) => name.as_str(),
            None => {
                log::error!(target: "tool_executor", task_id=%task.id, "No tool specified for local execution");
                // Update task status to Failed
                task.status = TaskStatus {
                    state: TaskState::Failed,
                    timestamp: Some(chrono::Utc::now()),
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

        log::info!(target: "tool_executor", task_id=%task.id, tool=%tool_name, "Attempting local execution");

        // --- Parameter Extraction ---
        // TODO: Implement more robust parameter extraction based on tool requirements
        // and task message structure (e.g., extracting from specific parts or metadata).
        // For now, extract parameters from the last user message's text part,
        // or potentially from a ToolCallPart if present.
        let params = task.history.as_ref()
            .and_then(|h| h.last()) // Get the latest message
            .and_then(|msg| {
                // Prioritize ToolCallPart if available
                msg.parts.iter().find_map(|part| match part {
                    Part::ToolCallPart(tcp) => {
                        // Attempt to parse the tool_call Value to see if it matches the expected tool
                        if let Ok(parsed_call) = serde_json::from_value::<crate::types::ToolCall>(tcp.tool_call.clone()) {
                            if parsed_call.name == tool_name {
                                Some(parsed_call.params)
                            } else { None }
                        } else { None }
                    },
                    _ => None,
                })
                // Fallback to TextPart
                .or_else(|| msg.parts.iter().find_map(|part| match part {
                    Part::TextPart(tp) => Some(json!({"text": tp.text})), // Simple text param
                    _ => None,
                }))
            })
            .unwrap_or_else(|| {
                log::warn!(target: "tool_executor", task_id=%task.id, tool=%tool_name, "No suitable parameters found in task history, using default.");
                json!({}) // Default to empty params if none found
            });


        // --- Execute the selected tool ---
        match self.execute_tool(tool_name, params).await {
            Ok(result_value) => {
                log::info!(target: "tool_executor", task_id=%task.id, tool=%tool_name, "Tool executed successfully.");

                // --- Format Result into Task Artifact ---
                // Attempt to serialize the result value back to a string or keep as JSON
                let result_text = serde_json::to_string_pretty(&result_value)
                    .unwrap_or_else(|_| format!("{:?}", result_value)); // Fallback to debug format

                 let result_part = Part::TextPart(TextPart {
                    type_: "text".to_string(), // Or potentially "json" if result is complex
                    text: result_text,
                    metadata: None,
                });

                // Create an artifact containing the result part
                let artifact = Artifact {
                    parts: vec![result_part],
                    index: task.artifacts.as_ref().map_or(0, |a| a.len()) as i64, // Next index
                    name: Some(format!("{}_result", tool_name)),
                    description: Some(format!("Result of local execution using tool '{}'", tool_name)),
                    append: None,
                    last_chunk: None, // Assuming non-streaming tool result
                    metadata: None,
                };

                // Add artifact to the task
                task.artifacts.get_or_insert_with(Vec::new).push(artifact);

                // --- Update Task Status to Completed ---
                 task.status = TaskStatus {
                    state: TaskState::Completed,
                    timestamp: Some(chrono::Utc::now()),
                    // Provide a confirmation message from the agent
                    message: Some(Message { // Use struct initialization
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
            Err(tool_error) => {
                 log::error!(target: "tool_executor", task_id=%task.id, tool=%tool_name, error=?tool_error, "Tool execution failed");
                // --- Update Task Status to Failed ---
                 task.status = TaskStatus {
                    state: TaskState::Failed,
                    timestamp: Some(chrono::Utc::now()),
                     // Provide an error message from the agent
                    message: Some(Message { // Use struct initialization
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Local execution failed: {}", tool_error),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        bidirectional_agent::tools::{Tool}, // Removed directory_tool import as it's conditional
        types::{Task, Message, TextPart, TaskStatus, TaskState}, // Import Task types
    };
    use serde_json::json;
    use std::sync::Arc;
    // Conditionally import directory test helpers
    #[cfg(feature = "bidir-core")]
    use crate::bidirectional_agent::agent_registry::tests::create_test_registry_with_real_dir;


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
    // Helper to create a ToolExecutor with mock tools
    fn create_test_executor_with_mocks() -> ToolExecutor {
        let mut tools: HashMap<String, Box<dyn Tool>> = HashMap::new();
        tools.insert("echo".to_string(), Box::new(MockEchoTool));
        tools.insert("fail".to_string(), Box::new(MockFailTool));
        // We don't need AgentDirectory here as DirectoryTool is not registered
        ToolExecutor { tools: Arc::new(tools) }
    }

     // Helper to create a ToolExecutor with real tools (requires real directory)
     #[cfg(feature = "bidir-core")]
     async fn create_real_tool_executor() -> ToolExecutor {
         // This requires the directory setup from registry tests
         let (_registry, directory) = create_test_registry_with_real_dir().await;
         ToolExecutor::new(directory) // Use the real constructor
     }


    // --- execute_tool Tests ---
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
    async fn test_execute_tool_not_found() {
        let executor = create_test_executor_with_mocks();
        let params = json!({});
        let result = executor.execute_tool("nonexistent_tool", params).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::NotFound(name) => assert_eq!(name, "nonexistent_tool"),
            e => panic!("Expected NotFound error, got {:?}", e),
        }
    }

     #[tokio::test]
    async fn test_execute_tool_echo_invalid_params() {
        let executor = create_test_executor_with_mocks();
        let params = json!({"wrong_param": "hello"}); // Missing "text"
        let result = executor.execute_tool("echo", params).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::InvalidParams(name, msg) => {
                assert_eq!(name, "echo");
                assert!(msg.contains("Missing 'text' parameter"));
            },
            e => panic!("Expected InvalidParams error, got {:?}", e),
        }
    }

    // --- execute_task_locally Tests ---
    #[tokio::test]
    async fn test_execute_task_locally_success() {
        let executor = create_test_executor_with_mocks();
        let mut task = Task {
            id: "task-local-succ".to_string(),
            history: Some(vec![Message::user("echo this message")]), // Provide input message
            status: TaskStatus::initial(), // Start with initial status
            artifacts: None,
            metadata: None,
            relationships: None,
        };
        let tool_names = vec!["echo".to_string()];

        let result = executor.execute_task_locally(&mut task, &tool_names).await;

        assert!(result.is_ok());
        assert_eq!(task.status.state, TaskState::Completed);
        assert!(task.status.message.is_some());
        assert!(task.status.message.unwrap().text().contains("completed"));
        assert!(task.artifacts.is_some());
        let artifacts = task.artifacts.unwrap();
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].name.as_deref(), Some("echo_result"));
        assert!(artifacts[0].parts[0].text().contains("Echo: echo this message"));
    }

     #[tokio::test]
    async fn test_execute_task_locally_tool_failure() {
        let executor = create_test_executor_with_mocks();
         let mut task = Task {
            id: "task-local-fail".to_string(),
            history: Some(vec![Message::user("run failing tool")]),
            status: TaskStatus::initial(),
            artifacts: None,
            metadata: None,
            relationships: None,
        };
        let tool_names = vec!["fail".to_string()]; // Specify the failing tool

        let result = executor.execute_task_locally(&mut task, &tool_names).await;

        assert!(result.is_err());
        assert_eq!(task.status.state, TaskState::Failed);
        assert!(task.status.message.is_some());
        let agent_msg = task.status.message.unwrap().text();
        assert!(agent_msg.contains("Local execution failed"));
        assert!(agent_msg.contains("Simulated tool execution failure")); // Include tool error
        assert!(task.artifacts.is_none()); // No artifacts on failure
    }

     #[tokio::test]
    async fn test_execute_task_locally_tool_not_found() {
        let executor = create_test_executor_with_mocks();
         let mut task = Task {
            id: "task-local-notfound".to_string(),
            history: Some(vec![Message::user("run unknown tool")]),
            status: TaskStatus::initial(),
            artifacts: None,
            metadata: None,
            relationships: None,
        };
        let tool_names = vec!["unknown".to_string()]; // Specify non-existent tool

        let result = executor.execute_task_locally(&mut task, &tool_names).await;

        assert!(result.is_err());
        assert_eq!(task.status.state, TaskState::Failed);
        assert!(task.status.message.is_some());
        assert!(task.status.message.unwrap().text().contains("Tool not found"));
        assert!(task.artifacts.is_none());
    }

    // --- generate_agent_skills Test ---
    #[tokio::test]
    #[cfg(feature = "bidir-core")]
    async fn test_generate_agent_skills() {
         // Use real executor to test skill generation from real tools
         let executor = create_real_tool_executor().await;
         let skills = executor.generate_agent_skills();

         println!("Generated Skills: {:?}", skills); // Debug output

         assert!(skills.len() >= 3, "Expected at least shell, http, and directory tools"); // Adjust based on registered tools

         // Check for specific tools
         assert!(skills.iter().any(|s| s.id == "shell"));
         assert!(skills.iter().any(|s| s.id == "http"));
         // Directory tool is conditional but guaranteed by this test being cfg-gated
         assert!(skills.iter().any(|s| s.id == "directory"));

         // Check content of one skill (directory)
         let dir_skill = skills.iter().find(|s| s.id == "directory").unwrap();
         assert_eq!(dir_skill.name, "directory");
         assert!(dir_skill.description.is_some());
         assert!(dir_skill.description.as_ref().unwrap().contains("agent directory"));
         assert!(dir_skill.tags.is_some());
         assert!(dir_skill.tags.as_ref().unwrap().contains(&"agent_directory".to_string()));
    }
}
