//! Executes local tools with standard A2A protocol-compliant types and artifacts.

use crate::server::error::ServerError;
use crate::bidirectional::bidirectional_agent::{LlmClient, AgentDirectory}; // Import LlmClient and AgentDirectory
use crate::types::{
    Task, TaskStatus, TaskState, Message, Role, Part, TextPart, DataPart, Artifact, AgentCard
};
use anyhow; // Import anyhow for error handling in new tools

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

    #[error("Tool execution failed due to external error: {0}")]
    ExternalError(String), // For wrapping anyhow::Error
}

// Implement conversion from std::io::Error
impl From<std::io::Error> for ToolError {
    fn from(e: std::io::Error) -> Self {
        ToolError::IoError(e.to_string())
    }
}

// Implement conversion from anyhow::Error
impl From<anyhow::Error> for ToolError {
    fn from(e: anyhow::Error) -> Self {
        // Capture the context of the anyhow error
        ToolError::ExternalError(format!("{:?}", e))
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
            ToolError::ExternalError(msg) => // Handle new error variant
                ServerError::ServerTaskExecutionFailed(format!("External error during tool execution: {}", msg)),
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


// --- New Tools ---

/// Tool that provides information about all known agents
pub struct ListAgentsTool {
    agent_directory: Arc<AgentDirectory>,
}

impl ListAgentsTool {
    pub fn new(agent_directory: Arc<AgentDirectory>) -> Self {
        Self { agent_directory }
    }
}

#[async_trait]
impl Tool for ListAgentsTool {
    fn name(&self) -> &str { "list_agents" }
    
    fn description(&self) -> &str { "Lists all known agents and their capabilities" }
    
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        // Get format parameter if provided (default to "detailed")
        let format = params.get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("detailed");
        
        // Get all agents
        let all_agents = self.agent_directory.list_all_agents();
        
        if all_agents.is_empty() {
            return Ok(json!({
                "count": 0,
                "message": "No agents found in the directory"
            }));
        }
        
        match format {
            "simple" => {
                // Just return agent IDs and names
                let simple_list: Vec<HashMap<String, String>> = all_agents
                    .iter()
                    .map(|(id, card)| {
                        let mut map = HashMap::new();
                        map.insert("id".to_string(), id.clone());
                        map.insert("name".to_string(), card.name.clone());
                        map
                    })
                    .collect();
                
                Ok(json!({
                    "count": simple_list.len(),
                    "agents": simple_list
                }))
            },
            "detailed" | _ => {
                // Return full agent details
                let detailed_list: Vec<serde_json::Value> = all_agents
                    .iter()
                    .map(|(id, card)| {
                        let mut agent_obj = serde_json::to_value(card).unwrap_or(json!({}));
                        
                        // Add the ID since it's not part of the card
                        if let Some(obj) = agent_obj.as_object_mut() {
                            obj.insert("id".to_string(), json!(id));
                        }
                        
                        agent_obj
                    })
                    .collect();
                
                Ok(json!({
                    "count": detailed_list.len(),
                    "agents": detailed_list
                }))
            }
        }
    }
    
    fn capabilities(&self) -> &[&'static str] { &["agent_directory", "agent_discovery"] }
}

/// Tool that directly uses the LLM based on input text
pub struct LlmTool { llm: Arc<dyn LlmClient> }
impl LlmTool { pub fn new(llm: Arc<dyn LlmClient>) -> Self { Self { llm } } }

#[async_trait]
impl Tool for LlmTool {
    fn name(&self) -> &str { "llm" }
    fn description(&self) -> &str { "Ask the LLM directly using the provided text as a prompt" }
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let prompt = params.get("text").and_then(|v| v.as_str()).unwrap_or_default();
        if prompt.is_empty() {
            return Err(ToolError::InvalidParams("llm".to_string(), "'text' parameter cannot be empty".to_string()));
        }
        // Use ? to propagate anyhow::Error, which will be converted by From<anyhow::Error> for ToolError
        let result = self.llm.complete(prompt).await?;
        Ok(json!(result))
    }
    fn capabilities(&self) -> &[&'static str] { &["llm", "chat", "general_purpose"] }
}

/// Tool that asks the LLM to summarize the input text
pub struct SummarizeTool { llm: Arc<dyn LlmClient> }
impl SummarizeTool { pub fn new(llm: Arc<dyn LlmClient>) -> Self { Self { llm } } }

#[async_trait]
impl Tool for SummarizeTool {
    fn name(&self) -> &str { "summarize" }
    fn description(&self) -> &str { "Summarize the received text using the LLM" }
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let text = params.get("text").and_then(|v| v.as_str()).unwrap_or_default();
         if text.is_empty() {
            return Err(ToolError::InvalidParams("summarize".to_string(), "'text' parameter cannot be empty".to_string()));
        }
        let prompt = format!("Briefly summarize the following text:\n\n{}", text);
        // Use ? to propagate anyhow::Error
        let result = self.llm.complete(&prompt).await?;
        Ok(json!(result))
    }
    fn capabilities(&self) -> &[&'static str] { &["summarize", "text_manipulation"] }
}

// --- End New Tools ---


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

    /// Creates a new ToolExecutor with tools enabled via configuration.
    pub fn with_enabled_tools(
        enabled: &[String],
        llm: Arc<dyn LlmClient>, // LLM client needed for some tools
        agent_directory: Option<Arc<AgentDirectory>>, // Optional agent directory for agent-related tools
    ) -> Self {
        let mut map: HashMap<String, Box<dyn Tool>> = HashMap::new();

        // Always register the echo tool as a fallback
        map.insert("echo".into(), Box::new(EchoTool));
        tracing::debug!("Tool 'echo' registered.");

        for name in enabled {
            match name.as_str() {
                "llm" => {
                    if !map.contains_key("llm") {
                        map.insert("llm".into(), Box::new(LlmTool::new(llm.clone())));
                        tracing::debug!("Tool 'llm' registered.");
                    }
                }
                "summarize" => {
                     if !map.contains_key("summarize") {
                        map.insert("summarize".into(), Box::new(SummarizeTool::new(llm.clone())));
                        tracing::debug!("Tool 'summarize' registered.");
                    }
                }
                "list_agents" => {
                    if !map.contains_key("list_agents") {
                        if let Some(dir) = agent_directory.clone() {
                            map.insert("list_agents".into(), Box::new(ListAgentsTool::new(dir)));
                            tracing::debug!("Tool 'list_agents' registered.");
                        } else {
                            tracing::warn!("Cannot register 'list_agents' tool: agent_directory not provided.");
                        }
                    }
                }
                "echo" => { /* already registered */ }
                unknown => {
                    tracing::warn!("Unknown tool '{}' in config [tools].enabled, ignoring.", unknown);
                }
            }
        }
        
        tracing::info!("ToolExecutor initialized with tools: {:?}", map.keys());
        Self { tools: Arc::new(map) }
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
        
        // Keep track of the tool name for specialized handling
        let using_llm_tool = requested_tool_name == "llm" || requested_tool_name == "summarize" || (
            !self.tools.contains_key(requested_tool_name) && 
            (default_tool_name == "llm" || default_tool_name == "summarize")
        );
        
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
                    // Provide a response message that includes the actual tool result
                    message: Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            // Format the response based on the tool and result
                            text: if tool_name == "list_agents" {
                                // Special handling for list_agents tool
                                let mut response = String::new();
                                
                                // Try to parse as JSON and create a formatted list
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&result_value.to_string()) {
                                    // Check if it contains agent count and list
                                    if let (Some(count), Some(agents)) = (json.get("count"), json.get("agents")) {
                                        response.push_str(&format!("Found {} agents:\n\n", count));
                                        
                                        if let Some(agents_array) = agents.as_array() {
                                            for (i, agent) in agents_array.iter().enumerate() {
                                                // Get agent details
                                                let id = agent.get("id").and_then(|v| v.as_str()).unwrap_or("Unknown ID");
                                                let name = agent.get("name").and_then(|v| v.as_str()).unwrap_or("Unnamed Agent");
                                                let desc = agent.get("description").and_then(|v| v.as_str()).unwrap_or("");
                                                
                                                response.push_str(&format!("{}. Agent: {} ({})\n", i+1, name, id));
                                                if !desc.is_empty() {
                                                    response.push_str(&format!("   Description: {}\n", desc));
                                                }
                                                response.push_str("\n");
                                            }
                                        }
                                    } else {
                                        // Fallback if JSON structure is unexpected
                                        response = format!("Agent Directory: {}", result_value);
                                    }
                                } else {
                                    // Fallback if not valid JSON
                                    response = result_value.to_string();
                                }
                                response
                            } else if using_llm_tool {
                                // For LLM, the response is already formatted text
                                result_value.to_string().trim_matches('"').to_string()
                            } else {
                                // Default formatting for other tools
                                match result_value.to_string().trim_matches('"') {
                                    // If it's just a JSON string with quotes, unwrap it
                                    s if s.starts_with("{") && s.ends_with("}") => s.to_string(),
                                    // Otherwise, use the result directly
                                    s => s.to_string(),
                                }
                            },
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
                            // Include the tool result in the response message, not just a confirmation
                            text: match result_value.to_string().trim_matches('"') {
                                // If it's just a JSON string with quotes, unwrap it
                                s if s.starts_with("{") && s.ends_with("}") => s.to_string(),
                                // Otherwise, use the result directly
                                s => s.to_string(),
                            },
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
