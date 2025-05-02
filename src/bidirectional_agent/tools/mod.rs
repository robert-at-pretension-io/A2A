//! Built-in tools for the Bidirectional Agent.

use async_trait::async_trait;
use dyn_clone::DynClone;
use serde_json::Value;
use crate::bidirectional_agent::tool_executor::ToolError; // Use ToolError from executor

// Define the core Tool trait
#[async_trait]
pub trait Tool: Send + Sync + DynClone + 'static {
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

// Enable cloning of trait objects
dyn_clone::clone_trait_object!(Tool);

// Re-export tool implementations
pub use shell_tool::ShellTool;
pub use http_tool::HttpTool;
pub use directory_tool::DirectoryTool;
pub use special_tool::{SpecialEchoTool1, SpecialEchoTool2};

// For remote tool execution
pub use pluggable::{RemoteToolRegistry, RemoteToolExecutor};

// Import tool modules
mod shell_tool;
mod http_tool;
mod directory_tool;
mod special_tool;
pub mod pluggable;
