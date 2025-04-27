//! Example Shell Tool implementation.

#![cfg(feature = "bidir-local-exec")]

use super::Tool; // Import the Tool trait from the parent module
use crate::bidirectional_agent::tool_executor::ToolError;
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

/// A tool for executing shell commands (with safety precautions).
pub struct ShellTool {
    // Add configuration if needed (e.g., allowed commands, timeout)
    allowed_commands: Vec<String>,
    default_timeout_ms: u64,
}

impl ShellTool {
    pub fn new() -> Self {
        // In a real implementation, load allowed commands from config
        Self {
            allowed_commands: vec!["ls".to_string(), "echo".to_string(), "pwd".to_string()],
            default_timeout_ms: 5000, // 5 second default timeout
        }
    }

    /// Basic validation to prevent obviously dangerous commands.
    /// This is NOT a substitute for proper sandboxing.
    fn validate_command(&self, command: &str, args: &[String]) -> Result<(), ToolError> {
        // 1. Check if command is in allowlist
        if !self.allowed_commands.contains(&command.to_string()) {
            return Err(ToolError::ExecutionFailed(
                self.name().to_string(),
                format!("Command '{}' is not allowed.", command),
            ));
        }

        // 2. Check for dangerous characters/patterns in args
        let dangerous_patterns = ["|", ";", "`", "$(", "&&", "||", ">", "<", "&"];
        for arg in args {
            if dangerous_patterns.iter().any(|p| arg.contains(p)) {
                return Err(ToolError::ExecutionFailed(
                    self.name().to_string(),
                    format!("Argument '{}' contains potentially dangerous characters.", arg),
                ));
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Tool for ShellTool {
    fn name(&self) -> &str {
        "shell"
    }

    fn description(&self) -> &str {
        "Executes allowed shell commands (e.g., ls, echo, pwd)."
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let command = params
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParams(self.name().to_string(), "Missing 'command' parameter".to_string())
            })?;

        let args: Vec<String> = params
            .get("args")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Validate the command and arguments
        self.validate_command(command, &args)?;

        println!("🐚 Executing shell command: {} {}", command, args.join(" "));

        // Execute the command with Tokio's Command
        let execution_timeout = Duration::from_millis(self.default_timeout_ms);
        match timeout(execution_timeout, Command::new(command).args(&args).output()).await {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code();

                println!("  Exit Code: {:?}", exit_code);
                println!("  Stdout: {}", stdout.trim());
                println!("  Stderr: {}", stderr.trim());

                if output.status.success() {
                    Ok(json!({
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": exit_code
                    }))
                } else {
                    Err(ToolError::ExecutionFailed(
                        self.name().to_string(),
                        format!(
                            "Command failed with exit code {:?}. Stderr: {}",
                            exit_code, stderr
                        ),
                    ))
                }
            }
            Ok(Err(e)) => Err(ToolError::ExecutionFailed(
                self.name().to_string(),
                format!("Failed to execute command: {}", e),
            )),
            Err(_) => Err(ToolError::ExecutionFailed(
                self.name().to_string(),
                format!("Command timed out after {}ms", self.default_timeout_ms),
            )),
        }
    }

    fn capabilities(&self) -> &[&'static str] {
        &["shell_command", "filesystem_read"] // Example capabilities
    }
}
