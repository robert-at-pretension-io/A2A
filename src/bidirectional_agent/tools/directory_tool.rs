//! Tool for accessing the agent directory.

// Only compile if local execution feature is enabled
#![cfg(feature = "bidir-local-exec")]

use crate::bidirectional_agent::{
    agent_directory::AgentDirectory,
    tool_executor::ToolError,
    tools::Tool, // Import the Tool trait definition
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;


/// Tool for interacting with the agent directory.
pub struct DirectoryTool {
    agent_directory: Arc<AgentDirectory>,
}

impl DirectoryTool {
    /// Creates a new DirectoryTool instance.
    pub fn new(agent_directory: Arc<AgentDirectory>) -> Self {
        Self { agent_directory }
    }
}

/// Defines the actions the DirectoryTool can perform, derived from input parameters.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", tag = "action")] // Use 'action' field in JSON to determine enum variant
enum DirectoryAction {
    ListActive,
    ListInactive,
    GetInfo { agent_id: String },
}

#[async_trait]
impl Tool for DirectoryTool {
    fn name(&self) -> &str {
        "directory"
    }

    fn description(&self) -> &str {
        "Accesses the agent directory. Actions: list_active, list_inactive, get_info (requires agent_id)."
    }

    fn capabilities(&self) -> &[&'static str] {
        // List specific capabilities provided by this tool
        &["agent_directory", "list_agents", "get_agent_info"]
    }

    /// Executes the specified directory action based on the parameters.
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        // Handle both direct params and params embedded in a text field
        let params_to_use = if params.is_object() && params.get("text").is_some() && params["text"].is_string() {
            // Try to parse the text field as JSON
            let text = params["text"].as_str().unwrap();
            match serde_json::from_str::<Value>(text) {
                Ok(parsed) => parsed,
                Err(_) => params.clone(), // If parsing fails, use original params
            }
        } else {
            params.clone()
        };
        
        // Deserialize the JSON parameters into a DirectoryAction enum
        let action: DirectoryAction = serde_json::from_value(params_to_use.clone()) // Clone params for error reporting
            .map_err(|e| ToolError::InvalidParams(
                self.name().to_string(),
                format!("Invalid parameters format: {}. Expected JSON with 'action' tag (e.g., {{'action': 'list_active'}} or {{'action': 'get_info', 'agent_id': '...'}}). Received: {}", e, params_to_use)
            ))?;

        // Execute the corresponding AgentDirectory method based on the action
        match action {
            DirectoryAction::ListActive => {
                let agents = self.agent_directory.get_active_agents().await
                    .map_err(|e| ToolError::ExecutionFailed(
                        self.name().to_string(),
                        format!("Failed to get active agents: {}", e))
                    )?;

                // Format the result as JSON
                Ok(json!({
                    "active_agents": agents.into_iter().map(|agent| {
                        json!({ "id": agent.agent_id, "url": agent.url })
                    }).collect::<Vec<_>>()
                }))
            },
            DirectoryAction::ListInactive => {
                let agents = self.agent_directory.get_inactive_agents().await
                     .map_err(|e| ToolError::ExecutionFailed(
                        self.name().to_string(),
                        format!("Failed to get inactive agents: {}", e))
                    )?;

                Ok(json!({
                    "inactive_agents": agents.into_iter().map(|agent| {
                        json!({ "id": agent.agent_id, "url": agent.url })
                    }).collect::<Vec<_>>()
                }))
            },
            DirectoryAction::GetInfo { agent_id } => {
                let info = self.agent_directory.get_agent_info(&agent_id).await
                    .map_err(|e| {
                        // Check if the error indicates the agent wasn't found
                        if e.to_string().contains("not found") {
                             // Return InvalidParams error for non-existent agent ID
                             ToolError::InvalidParams(
                                 self.name().to_string(),
                                 format!("Agent ID '{}' not found in directory.", agent_id)
                             )
                        } else {
                            // Return ExecutionFailed for other errors
                            ToolError::ExecutionFailed(
                                self.name().to_string(),
                                format!("Failed to get agent info for '{}': {}", agent_id, e)
                            )
                        }
                    })?;

                // Return the detailed agent info as JSON
                Ok(json!({ "agent_info": info }))
            }
        }
    }
}
