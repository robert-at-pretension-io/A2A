//! Built-in tools for the Bidirectional Agent.

// Only compile if local execution feature is enabled
#![cfg(feature = "bidir-local-exec")]

use async_trait::async_trait;
use serde_json::Value;
use crate::bidirectional_agent::tool_executor::ToolError; // Use ToolError from executor

// Define the core Tool trait
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

// Re-export tool implementations
pub use shell_tool::ShellTool;
pub use http_tool::HttpTool;
#[cfg(feature = "bidir-core")]
pub use directory_tool::DirectoryTool;

// Import tool modules
mod shell_tool;
mod http_tool;
#[cfg(feature = "bidir-core")]
mod directory_tool;