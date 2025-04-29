//! Built-in tools for the Bidirectional Agent.

#![cfg(feature = "bidir-local-exec")]

// pub mod shell_tool;
// pub mod http_tool;
// pub mod ai_tool; // Example for later

// Re-export tools for easier access
// pub use shell_tool::ShellTool;
// pub use http_tool::HttpTool;
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

// Define the core Tool trait (moved from tool_executor.rs)
use async_trait::async_trait;
use serde_json::Value;
use crate::bidirectional_agent::tool_executor::ToolError; // Use ToolError from executor

#[async_trait]
pub trait Tool: Send + Sync + 'static {
    /// Returns the unique name of the tool (e.g., "shell", "http", "directory").
    fn name(&self) -> &str;
    /// Returns a human-readable description of the tool's purpose.
    fn description(&self) -> &str;
    /// Executes the tool with the given JSON parameters.
    async fn execute(&self, params: Value) -> Result<Value, ToolError>;
    /// Returns a list of keywords representing the tool's capabilities.
    fn capabilities(&self) -> &[&'static str];
}


// Tool implementations
pub mod shell_tool;
pub mod http_tool;
#[cfg(feature = "bidir-core")] // Only include directory tool if core features are enabled
pub mod directory_tool;
// pub mod ai_tool; // Example for later

// Re-export tools for easier access from ToolExecutor
pub use shell_tool::ShellTool;
pub use http_tool::HttpTool;
#[cfg(feature = "bidir-core")]
pub use directory_tool::DirectoryTool;
