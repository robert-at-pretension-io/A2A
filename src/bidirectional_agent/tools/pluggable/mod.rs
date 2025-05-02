use async_trait::async_trait;
use log::{info, warn, error, debug};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use anyhow::Result;
use crate::bidirectional_agent::agent_directory::AgentDirectory;
use crate::bidirectional_agent::client_manager::ClientManager;
use crate::bidirectional_agent::tool_executor::ToolError;
use crate::bidirectional_agent::tools::Tool;
use crate::types::{AgentCard, AgentSkill, Part, TaskSendParams, Message, Role, TextPart, DataPart, FilePart, TaskState};
use serde_json::json;

// Remote tool registry to track tools exposed by remote agents
pub struct RemoteToolRegistry {
    agent_directory: Arc<AgentDirectory>,
    // Track tools by agent_id and tool_name
    tools: Mutex<HashMap<String, HashMap<String, RemoteToolInfo>>>,
}

/// Information about a tool provided by a remote agent
#[derive(Debug, Clone)]
pub struct RemoteToolInfo {
    pub name: String,
    pub description: String,
    pub agent_id: String,
    pub capabilities: Vec<String>,
    pub input_modes: Vec<String>,
    pub output_modes: Vec<String>,
    pub tags: Vec<String>,
    pub examples: Vec<String>,
}

impl RemoteToolRegistry {
    pub fn new(agent_directory: Arc<AgentDirectory>) -> Self {
        RemoteToolRegistry {
            agent_directory,
            tools: Mutex::new(HashMap::new()),
        }
    }

    /// Find the agent_id that provides a particular tool
    pub fn find_agent_for_tool(&self, tool_name: &str) -> Option<String> {
        let tools = self.tools.lock().unwrap();
        
        // Search all agents for the tool
        for (agent_id, agent_tools) in tools.iter() {
            if agent_tools.contains_key(tool_name) {
                return Some(agent_id.clone());
            }
        }
        None
    }

    /// Get information about a specific tool
    pub fn get_tool_info(&self, tool_name: &str) -> Option<RemoteToolInfo> {
        let tools = self.tools.lock().unwrap();
        
        // Search all agents for the tool
        for agent_tools in tools.values() {
            if let Some(tool_info) = agent_tools.get(tool_name) {
                return Some(tool_info.clone());
            }
        }
        None
    }

    /// Update tools for an agent based on its card
    pub async fn update_tools_from_card(&self, agent_id: &str, card: &AgentCard) -> Result<bool> {
        debug!("Updating tools from card for agent {}", agent_id);
        let mut tools = self.tools.lock().unwrap();
        let agent_tools = tools.entry(agent_id.to_string()).or_insert_with(HashMap::new);
        
        // Process each skill in the agent card as a potential tool
        for skill in &card.skills {
            // Convert the skill to a tool
            let tool_info = RemoteToolInfo {
                name: skill.id.clone(),
                description: skill.description.clone().unwrap_or_default(),
                agent_id: agent_id.to_string(),
                capabilities: skill.tags.clone().unwrap_or_default(),
                input_modes: skill.input_modes.clone().unwrap_or_else(|| card.default_input_modes.clone()),
                output_modes: skill.output_modes.clone().unwrap_or_else(|| card.default_output_modes.clone()),
                tags: skill.tags.clone().unwrap_or_default(),
                examples: skill.examples.clone().unwrap_or_default(),
            };
            
            agent_tools.insert(skill.id.clone(), tool_info);
            info!("Registered remote tool '{}' from agent '{}'", skill.id, agent_id);
        }
        
        Ok(true)
    }
    
    /// Update tools from an agent with specific tool names
    /// This is useful when you know an agent has certain tools but don't have the full card
    pub async fn update_tools_from_agent(&self, agent_id: &str, tool_names: &[String]) -> Result<bool> {
        info!("Registering tools {} from agent {}", tool_names.join(", "), agent_id);
        let mut tools = self.tools.lock().unwrap();
        let agent_tools = tools.entry(agent_id.to_string()).or_insert_with(HashMap::new);
        
        for tool_name in tool_names {
            if !agent_tools.contains_key(tool_name) {
                agent_tools.insert(tool_name.to_string(), RemoteToolInfo {
                    name: tool_name.to_string(),
                    description: format!("Tool '{}' provided by agent '{}'", tool_name, agent_id),
                    agent_id: agent_id.to_string(),
                    capabilities: vec!["remote_tool".to_string()],
                    input_modes: vec!["text/plain".to_string()],
                    output_modes: vec!["text/plain".to_string()],
                    tags: vec!["remote_tool".to_string()],
                    examples: vec![],
                });
                info!("Registered remote tool '{}' from agent '{}'", tool_name, agent_id);
            }
        }
        
        Ok(true)
    }
    
    /// Get all registered tools
    pub fn get_all_tools(&self) -> HashMap<String, Vec<RemoteToolInfo>> {
        let tools = self.tools.lock().unwrap();
        let mut result = HashMap::new();
        
        for (agent_id, agent_tools) in tools.iter() {
            let tools_vec: Vec<RemoteToolInfo> = agent_tools.values().cloned().collect();
            result.insert(agent_id.clone(), tools_vec);
        }
        
        result
    }
    
    /// Register a tool manually (for testing purposes)
    pub fn register_tool(&self, agent_id: &str, tool_name: &str) {
        let mut tools = self.tools.lock().unwrap();
        let agent_tools = tools.entry(agent_id.to_string()).or_insert_with(HashMap::new);
        
        agent_tools.insert(tool_name.to_string(), RemoteToolInfo {
            name: tool_name.to_string(),
            description: format!("Test tool {}", tool_name),
            agent_id: agent_id.to_string(),
            capabilities: vec!["testing".to_string()],
            input_modes: vec!["text/plain".to_string()],
            output_modes: vec!["text/plain".to_string()],
            tags: vec!["testing".to_string()],
            examples: vec![],
        });
        
        info!("Manually registered remote tool '{}' from agent '{}'", tool_name, agent_id);
    }
}

/// Remote tool executor that forwards tool calls to remote agents
/// This component is responsible for executing tools that are provided by remote agents
/// using the A2A protocol. It formats and sends tool execution requests and processes
/// the responses.
#[derive(Clone)]
pub struct RemoteToolExecutor {
    client_manager: Arc<ClientManager>,
    registry: Arc<RemoteToolRegistry>,
}

impl RemoteToolExecutor {
    /// Create a new RemoteToolExecutor with the given client manager
    pub fn new(client_manager: Arc<ClientManager>, registry: Arc<RemoteToolRegistry>) -> Self {
        RemoteToolExecutor {
            client_manager,
            registry,
        }
    }

    /// Execute a tool on a remote agent
    /// This method is called when a tool needs to be executed by a remote agent.
    /// It formats the request properly according to the A2A protocol, sends it,
    /// and processes the response.
    pub async fn execute_tool(
        &self,
        agent_id: &str,
        tool_name: &str,
        params: Value,
    ) -> Result<Value, ToolError> {
        info!("Executing remote tool '{}' on agent '{}' with params: {}", tool_name, agent_id, params);
        
        // Create unique task ID for this tool execution
        let task_id = format!("tool-call-{}-{}", tool_name, uuid::Uuid::new_v4());
        
        // Get information about the tool if available
        let tool_info = self.registry.get_tool_info(tool_name);
        
        // Format the message parts based on the parameter type
        let parts = self.format_tool_call_parts(tool_name, &params, tool_info.as_ref());
        
        // Create task metadata
        let mut task_metadata = serde_json::Map::new();
        task_metadata.insert("tool_execution".to_string(), Value::Bool(true));
        task_metadata.insert("tool_name".to_string(), Value::String(tool_name.to_string()));
        
        // Create proper TaskSendParams
        let task_params = TaskSendParams {
            id: task_id.clone(),
            message: Message {
                role: Role::User,
                parts,
                metadata: Some(task_metadata.clone()),
            },
            history_length: None,
            metadata: Some(task_metadata),
            push_notification: None,
            session_id: None, // We could set this to track related tool calls if needed
        };
        
        // Execute the task on the remote agent
        match self.client_manager.send_task(agent_id, task_params).await {
            Ok(task) => {
                // Process the response based on task status
                match task.status.state {
                    TaskState::Completed => {
                        self.process_completed_task_response(task, tool_name)
                    },
                    TaskState::Failed => {
                        Err(ToolError::ExecutionFailed(
                            tool_name.to_string(), 
                            format!("Remote agent reported tool execution failed: {:?}", 
                                   task.status.message.map(|m| format!("{:?}", m)))
                        ))
                    },
                    TaskState::Canceled => {
                        Err(ToolError::ExecutionFailed(
                            tool_name.to_string(), 
                            "Remote tool execution was canceled".to_string()
                        ))
                    },
                    other_state => {
                        Err(ToolError::ExecutionFailed(
                            tool_name.to_string(), 
                            format!("Remote tool execution ended with unexpected state: {:?}", other_state)
                        ))
                    }
                }
            },
            Err(e) => {
                Err(ToolError::ExecutionFailed(
                    tool_name.to_string(), 
                    format!("Failed to execute tool on agent '{}': {}", agent_id, e)
                ))
            }
        }
    }
    
    /// Format the message parts based on the parameter type and tool info
    fn format_tool_call_parts(
        &self, 
        tool_name: &str, 
        params: &Value,
        tool_info: Option<&RemoteToolInfo>,
    ) -> Vec<Part> {
        let mut parts = Vec::new();
        
        // Create metadata for tool execution
        let mut metadata = serde_json::Map::new();
        metadata.insert("tool_name".to_string(), Value::String(tool_name.to_string()));
        metadata.insert("tool_params".to_string(), params.clone());
        
        // Always include a text part with tool name for readability
        let tool_description = match tool_info {
            Some(info) => format!("Execute tool: {} - {}", tool_name, info.description),
            None => format!("Execute tool: {}", tool_name),
        };
        
        parts.push(Part::TextPart(TextPart {
            type_: "text".to_string(),
            text: tool_description,
            metadata: Some(metadata.clone()),
        }));
        
        // If the parameters are a complex object, also add a structured data part
        if params.is_object() || params.is_array() {
            parts.push(Part::DataPart(DataPart {
                type_: "data".to_string(),
                data: {
                    let mut data_map = serde_json::Map::new();
                    data_map.insert("tool_call".to_string(), json!({
                        "name": tool_name,
                        "params": params.clone()
                    }));
                    data_map
                },
                metadata: Some(metadata),
            }));
        }
        
        parts
    }
    
    /// Process a completed task response by extracting the results
    fn process_completed_task_response(&self, task: crate::types::Task, tool_name: &str) -> Result<Value, ToolError> {
        // Extract result from artifacts
        if let Some(artifacts) = task.artifacts {
            if !artifacts.is_empty() {
                // First try to extract structured data (JSON) if available
                for artifact in &artifacts {
                    for part in &artifact.parts {
                        match part {
                            Part::DataPart(data_part) => {
                                // Found a data part - perfect for structured tool results
                                return Ok(Value::Object(data_part.data.clone()));
                            },
                            _ => continue,
                        }
                    }
                }
                
                // If no structured data, try text
                for artifact in &artifacts {
                    for part in &artifact.parts {
                        match part {
                            Part::TextPart(text_part) => {
                                let text = &text_part.text;
                                // Try to parse as JSON if possible
                                if text.trim().starts_with('{') && text.trim().ends_with('}') {
                                    match serde_json::from_str::<Value>(text) {
                                        Ok(json_val) => return Ok(json_val),
                                        Err(_) => {} // Continue to next part if not valid JSON
                                    }
                                } else if text.trim().starts_with('[') && text.trim().ends_with(']') {
                                    match serde_json::from_str::<Value>(text) {
                                        Ok(json_val) => return Ok(json_val),
                                        Err(_) => {} // Continue to next part if not valid JSON
                                    }
                                }
                                
                                // Not JSON, return as string value
                                return Ok(Value::String(text.clone()));
                            },
                            _ => continue,
                        }
                    }
                }
                
                // If we reach here, we found artifacts but no usable content
                // Just return a generic success message with artifact count
                return Ok(Value::String(format!(
                    "Tool '{}' executed successfully, returned {} artifacts (but content couldn't be parsed)",
                    tool_name, artifacts.len()
                )));
            }
        }
        
        // Check for a result message in the task status
        if let Some(message) = &task.status.message {
            for part in &message.parts {
                if let Part::TextPart(text_part) = part {
                    return Ok(Value::String(text_part.text.clone()));
                }
            }
        }
        
        // If no useful artifacts or message, return a generic success message
        Ok(Value::String(format!("Tool '{}' executed successfully but no detailed results available", 
                               tool_name)))
    }
    
    /// Find the agent ID for a tool and execute it
    /// This is a convenience method that combines finding the agent for a tool
    /// and executing it in one step.
    pub async fn find_and_execute_tool(
        &self,
        tool_name: &str,
        params: Value,
    ) -> Result<Value, ToolError> {
        // Find which agent provides this tool
        if let Some(agent_id) = self.registry.find_agent_for_tool(tool_name) {
            // Execute the tool on that agent
            self.execute_tool(&agent_id, tool_name, params).await
        } else {
            Err(ToolError::NotFound(format!(
                "No agent found that provides tool '{}'", tool_name
            )))
        }
    }
    
    /// Get all available remote tools as AgentSkills
    pub fn get_remote_tools_as_skills(&self) -> Vec<crate::types::AgentSkill> {
        let all_tools = self.registry.get_all_tools();
        let mut skills = Vec::new();
        
        for (agent_id, tools) in all_tools {
            for tool in tools {
                let skill = crate::types::AgentSkill {
                    id: tool.name,
                    name: format!("{} (via {})", tool.description, agent_id),
                    description: Some(format!("Remote tool provided by agent: {}", agent_id)),
                    tags: Some(tool.tags),
                    examples: Some(tool.examples),
                    input_modes: Some(tool.input_modes),
                    output_modes: Some(tool.output_modes),
                };
                skills.push(skill);
            }
        }
        
        skills
    }
}

/// Implementation of the Tool trait for RemoteToolExecutor
/// This allows the executor to be used as a single tool in the ToolExecutor
#[async_trait]
impl Tool for RemoteToolExecutor {
    /// The name of this tool
    fn name(&self) -> &str {
        "remote_tool_executor"
    }
    
    /// Description of this tool
    fn description(&self) -> &str {
        "Executes tools provided by remote agents in the agent network"
    }
    
    /// Execute method required by the Tool trait
    /// This implementation routes the call to the appropriate agent
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        // Extract the tool name and parameters from the input
        let tool_name = params["tool_name"].as_str().ok_or_else(|| {
            ToolError::InvalidParams(
                "remote_tool_executor".to_string(),
                "Missing required 'tool_name' parameter".to_string()
            )
        })?;
        
        // Extract tool parameters, defaulting to empty object if not provided
        let tool_params = params.get("params").cloned().unwrap_or(json!({}));
        
        // Find and execute the tool
        self.find_and_execute_tool(tool_name, tool_params).await
    }
    
    /// Capabilities provided by this tool
    fn capabilities(&self) -> &[&'static str] {
        &["remote_execution", "agent_delegation"]
    }
}

/// A wrapper that makes a specific remote tool appear as a local tool
/// This allows each remote tool to be registered directly with the ToolExecutor
#[derive(Clone)]
pub struct RemoteToolWrapper {
    /// The remote tool executor that will handle the execution
    executor: Arc<RemoteToolExecutor>,
    /// Information about the remote tool
    tool_info: RemoteToolInfo,
}

impl RemoteToolWrapper {
    /// Create a new RemoteToolWrapper for a specific tool
    pub fn new(executor: Arc<RemoteToolExecutor>, tool_info: RemoteToolInfo) -> Self {
        Self {
            executor,
            tool_info,
        }
    }
}

#[async_trait]
impl Tool for RemoteToolWrapper {
    /// The name of this tool is the name of the remote tool
    fn name(&self) -> &str {
        &self.tool_info.name
    }
    
    /// Description of this tool
    fn description(&self) -> &str {
        &self.tool_info.description
    }
    
    /// Execute the remote tool
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        // Just forward the parameters to the remote tool executor
        self.executor.execute_tool(
            &self.tool_info.agent_id,
            &self.tool_info.name,
            params
        ).await
    }
    
    /// Capabilities are those provided by the remote tool
    fn capabilities(&self) -> &[&'static str] {
        // This is a bit of a hack since Tool expects &'static str but we have Vec<String>
        // Convert at runtime and leak the memory (acceptable for this use case)
        static CAPABILITIES: &[&'static str] = &[];
        CAPABILITIES
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    use crate::bidirectional_agent::agent_directory::AgentDirectory;
    use crate::bidirectional_agent::config::DirectoryConfig;
    use crate::bidirectional_agent::client_manager::ClientManager;
    use crate::bidirectional_agent::config::BidirectionalAgentConfig;
    use mockito::Server;
    use tempfile;
    
    async fn setup_test_environment() -> (
        Arc<AgentDirectory>,
        Arc<RemoteToolRegistry>,
        Arc<ClientManager>,
        Arc<RemoteToolExecutor>
    ) {
        // Create a temporary directory for the agent directory database
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = temp_dir.path().join("test_remote_tools.db");
        
        // Create the directory config
        let dir_config = DirectoryConfig {
            db_path: db_path.to_string_lossy().to_string(),
            ..Default::default()
        };
        
        // Initialize the agent directory
        let directory = Arc::new(AgentDirectory::new(&dir_config).await.unwrap());
        
        // Create agent registry
        let registry = Arc::new(crate::bidirectional_agent::agent_registry::AgentRegistry::new(directory.clone()));
        
        // Create a basic config 
        let agent_config = Arc::new(BidirectionalAgentConfig::default());
        
        // Create client manager
        let client_manager = Arc::new(ClientManager::new(registry.clone(), agent_config).unwrap());
        
        // Create remote tool registry
        let remote_registry = Arc::new(RemoteToolRegistry::new(directory.clone()));
        
        // Create remote tool executor
        let executor = Arc::new(RemoteToolExecutor::new(client_manager.clone(), remote_registry.clone()));
        
        (directory, remote_registry, client_manager, executor)
    }
    
    #[tokio::test]
    async fn test_remote_tool_registration() {
        let (_directory, registry, _client_manager, _executor) = setup_test_environment().await;
        
        // Register a test tool
        let agent_id = "test-agent";
        let tool_name = "test-tool";
        registry.register_tool(agent_id, tool_name);
        
        // Verify tool is registered
        let agent_id_for_tool = registry.find_agent_for_tool(tool_name);
        assert!(agent_id_for_tool.is_some());
        assert_eq!(agent_id_for_tool.unwrap(), agent_id);
        
        // Get tool info
        let tool_info = registry.get_tool_info(tool_name);
        assert!(tool_info.is_some());
        assert_eq!(tool_info.unwrap().name, tool_name);
    }
    
    #[tokio::test]
    async fn test_remote_tool_wrapper() {
        let (_directory, registry, _client_manager, executor) = setup_test_environment().await;
        
        // Register a test tool
        let agent_id = "test-agent";
        let tool_name = "test-tool";
        registry.register_tool(agent_id, tool_name);
        
        // Get tool info
        let tool_info = registry.get_tool_info(tool_name).unwrap();
        
        // Create a wrapper
        let wrapper = RemoteToolWrapper::new(executor.clone(), tool_info);
        
        // Verify wrapper properties
        assert_eq!(wrapper.name(), tool_name);
        assert!(wrapper.description().contains("Test tool"));
    }
    
    #[tokio::test]
    async fn test_format_tool_call_parts() {
        let (_directory, _registry, _client_manager, executor) = setup_test_environment().await;
        
        // Test with simple parameters
        let tool_name = "test-tool";
        let params = json!({ "text": "Hello, world!" });
        let parts = executor.format_tool_call_parts(tool_name, &params, None);
        
        // Should have both text and data parts
        assert_eq!(parts.len(), 2);
        
        // Check text part
        match &parts[0] {
            Part::TextPart(text_part) => {
                assert!(text_part.text.contains(tool_name));
                assert!(text_part.metadata.is_some());
            },
            _ => panic!("Expected TextPart"),
        }
        
        // Check data part
        match &parts[1] {
            Part::DataPart(data_part) => {
                assert!(data_part.data.contains_key("tool_call"));
                let tool_call = &data_part.data["tool_call"];
                assert_eq!(tool_call["name"].as_str().unwrap(), tool_name);
                assert_eq!(tool_call["params"]["text"].as_str().unwrap(), "Hello, world!");
            },
            _ => panic!("Expected DataPart"),
        }
    }
    
    #[tokio::test]
    async fn test_process_completed_task_response() {
        let (_directory, _registry, _client_manager, executor) = setup_test_environment().await;
        
        // Create a test task with text artifact
        let task_id = "test-task";
        let text_result = "Test result";
        
        let task = crate::types::Task {
            id: task_id.to_string(),
            session_id: Some("test".to_string()),
            status: crate::types::TaskStatus {
                state: TaskState::Completed,
                timestamp: Some(chrono::Utc::now()),
                message: None,
            },
            artifacts: Some(vec![
                crate::types::Artifact {
                    parts: vec![
                        Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: text_result.to_string(),
                            metadata: None,
                        }),
                    ],
                    index: 0,
                    name: Some("test".to_string()),
                    description: None,
                    append: None,
                    last_chunk: None,
                    metadata: None,
                },
            ]),
            history: None,
            metadata: None,
        };
        
        // Process the response
        let result = executor.process_completed_task_response(task, "test-tool");
        
        // Should extract text result
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value.as_str().unwrap(), text_result);
    }
}