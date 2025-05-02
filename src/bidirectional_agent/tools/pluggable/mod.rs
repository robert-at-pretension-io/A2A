use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use anyhow::Result;
use crate::bidirectional_agent::agent_directory::AgentDirectory;
use crate::bidirectional_agent::client_manager::ClientManager;
use crate::bidirectional_agent::tool_executor::ToolError;
use crate::types::{AgentCard, Part, TaskSendParams, Message, Role, TextPart};

// Remote tool registry to track tools exposed by remote agents
pub struct RemoteToolRegistry {
    agent_directory: Arc<AgentDirectory>,
    // Track tools by agent_id and tool_name
    tools: Mutex<HashMap<String, HashMap<String, RemoteToolInfo>>>,
}

pub struct RemoteToolInfo {
    pub name: String,
    pub description: String,
    pub agent_id: String,
    pub capabilities: Vec<String>,
}

impl RemoteToolRegistry {
    pub fn new(agent_directory: Arc<AgentDirectory>) -> Self {
        RemoteToolRegistry {
            agent_directory,
            tools: Mutex::new(HashMap::new()),
        }
    }

    // Update tools for an agent based on its card
    pub async fn update_tools_from_card(&self, agent_id: &str, card: &AgentCard) -> Result<bool> {
        // This is a simplified implementation
        // In a real implementation, this would parse the agent card to extract tool information
        Ok(true)
    }
    
    // Update tools from an agent with specific tool names
    pub async fn update_tools_from_agent(&self, agent_id: &str, tool_names: &[String]) -> Result<bool> {
        // This is a simplified implementation for testing
        println!("Registering tools {} from agent {}", tool_names.join(", "), agent_id);
        Ok(true)
    }
    
    // For use in tests
    pub fn register_tool(&self, agent_id: &str, tool_name: &str) {
        let mut tools = self.tools.lock().unwrap();
        let agent_tools = tools.entry(agent_id.to_string()).or_insert_with(HashMap::new);
        
        agent_tools.insert(tool_name.to_string(), RemoteToolInfo {
            name: tool_name.to_string(),
            description: format!("Test tool {}", tool_name),
            agent_id: agent_id.to_string(),
            capabilities: vec!["testing".to_string()],
        });
    }
}

// Remote tool executor that forwards tool calls to remote agents
pub struct RemoteToolExecutor {
    client_manager: Arc<ClientManager>,
}

impl RemoteToolExecutor {
    pub fn new(client_manager: Arc<ClientManager>) -> Self {
        RemoteToolExecutor {
            client_manager,
        }
    }

    // Execute a tool on a remote agent
    pub async fn execute_tool(
        &self,
        agent_id: &str,
        tool_name: &str,
        params: Value,
    ) -> Result<Value, ToolError> {
        println!("Executing remote tool '{}' on agent '{}' with params: {}", tool_name, agent_id, params);
        
        // Create metadata for tool execution
        let mut metadata = serde_json::Map::new();
        metadata.insert("tool_name".to_string(), Value::String(tool_name.to_string()));
        metadata.insert("tool_params".to_string(), params.clone());
        
        // Create a unique task ID
        let task_id = format!("tool-call-{}-{}", tool_name, uuid::Uuid::new_v4());
        
        // Create a simple message to send to the agent
        let message = format!("Execute tool: {}", tool_name);
        
        // Execute the task on the remote agent using the client manager
        // Create proper TaskSendParams
        let task_params = TaskSendParams {
            id: task_id.clone(),
            message: Message {
                role: Role::User,
                parts: vec![
                    Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: message,
                        metadata: {
                            // Convert metadata to the expected Map<String, Value> type
                            let metadata_map: serde_json::Map<String, Value> = metadata.into_iter().collect();
                            Some(metadata_map)
                        },
                    }),
                ],
                metadata: None,
            },
            history_length: None,
            metadata: None,
            push_notification: None,
            session_id: None,
        };
        
        match self.client_manager.send_task(agent_id, task_params).await {
            Ok(task) => {
                // Check if task completed successfully
                if let Some(artifacts) = task.artifacts {
                    if !artifacts.is_empty() {
                        // Extract the result from the first artifact
                        if let Some(Part::TextPart(text_part)) = artifacts[0].parts.first().map(|p| p.clone()) {
                            let text = text_part.text;
                            // Try to parse as JSON if possible
                            if text.trim().starts_with('{') && text.trim().ends_with('}') {
                                match serde_json::from_str::<Value>(&text) {
                                    Ok(json_val) => return Ok(json_val),
                                    Err(_) => return Ok(Value::String(text)),
                                }
                            }
                            return Ok(Value::String(text));
                        }
                    }
                }
                
                // If no useful artifacts, return a generic success message
                Ok(Value::String(format!("Tool '{}' executed on agent '{}', but no detailed results available", 
                                       tool_name, agent_id)))
            },
            Err(e) => {
                Err(ToolError::ExecutionFailed(
                    tool_name.to_string(), 
                    format!("Failed to execute tool on agent '{}': {}", agent_id, e)
                ))
            }
        }
    }
}