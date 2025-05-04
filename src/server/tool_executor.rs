//! Executes local tools with standard A2A protocol-compliant types and artifacts.

use crate::server::error::ServerError;
use crate::bidirectional::bidirectional_agent::LlmClient; // Import LlmClient (AgentDirectory removed)
use crate::types::{
    Task, TaskStatus, TaskState, Message, Role, Part, TextPart, DataPart, Artifact, AgentCard
};
use anyhow; // Import anyhow for error handling in new tools
use dashmap::DashMap; // For known_servers sync

use serde_json::{json, Value};
use tracing::{instrument, debug, info};
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
    fn from(e: std::io::Error) -> Self { // Corrected parameter type
        ToolError::IoError(e.to_string()) // Use IoError variant
    }
}

// Implement conversion from ServerError (for registry errors etc.)
impl From<ServerError> for ToolError {
    fn from(e: ServerError) -> Self {
        // Convert ServerError (like registry discovery errors) into a ToolError
        ToolError::ExternalError(format!("Server error during tool execution: {}", e))
    }
}

// Implement conversion from anyhow::Error (for LLM client errors etc.)
impl From<anyhow::Error> for ToolError {
    fn from(e: anyhow::Error) -> Self {
        ToolError::ExternalError(e.to_string())
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

/// Tool that provides information about all known agents from the canonical registry
pub struct ListAgentsTool {
    agent_registry: Arc<crate::server::agent_registry::AgentRegistry>, // Use canonical registry
}

impl ListAgentsTool {
    // Constructor now takes the canonical registry
    pub fn new(agent_registry: Arc<crate::server::agent_registry::AgentRegistry>) -> Self {
        Self { agent_registry }
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
        
        // Get all agents from the canonical registry
        let all_agents = self.agent_registry.list_all_agents(); // Use registry method
        
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

/// Tool to remember another agent by storing its card in the canonical AgentRegistry
/// and updating the list of known servers
pub struct RememberAgentTool {
    // This tool needs access to the canonical registry
    agent_registry: Arc<crate::server::agent_registry::AgentRegistry>, // Use canonical registry type
    // Also needs access to known_servers to keep them in sync
    known_servers: Option<Arc<DashMap<String, String>>>,
}

impl RememberAgentTool {
    // Constructor requires the AgentRegistry and optionally the known_servers map
    pub fn new(
        agent_registry: Arc<crate::server::agent_registry::AgentRegistry>,
        known_servers: Option<Arc<DashMap<String, String>>>
    ) -> Self {
        Self { 
            agent_registry,
            known_servers,
        }
    }
}

#[async_trait]
impl Tool for RememberAgentTool {
    fn name(&self) -> &str { "remember_agent" }

    fn description(&self) -> &str { "Discovers and stores/updates another agent's information in the registry using its base URL." }

    #[instrument(skip(self, params), name="tool_remember_agent")]
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        debug!("Executing 'remember_agent' tool."); // Changed to debug
        tracing::trace!(?params, "Tool parameters received.");

        // Extract the agent_base_url parameter
        tracing::debug!("Extracting 'agent_base_url' parameter.");
        let agent_url = params.get("agent_base_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                tracing::warn!("Missing or invalid 'agent_base_url' parameter.");
                ToolError::InvalidParams(self.name().to_string(), "Missing or invalid string parameter: 'agent_base_url'".to_string())
            })?
            .trim(); // Trim whitespace

        if agent_url.is_empty() {
             tracing::warn!("'agent_base_url' parameter is empty.");
             return Err(ToolError::InvalidParams(self.name().to_string(), "'agent_base_url' cannot be empty".to_string()));
        }
        // Basic URL validation (can be enhanced)
        if !agent_url.starts_with("http://") && !agent_url.starts_with("https://") {
            tracing::warn!(%agent_url, "Invalid URL format for 'agent_base_url'.");
            return Err(ToolError::InvalidParams(self.name().to_string(), "'agent_base_url' must start with http:// or https://".to_string()));
        }
        tracing::debug!(%agent_url, "Agent base URL extracted and validated.");

        // Use the registry's discover method. This verifies the agent is reachable
        // and fetches the latest card from the source URL before storing/updating.
        debug!(%agent_url, "Attempting to discover and register/update agent via AgentRegistry."); // Changed to debug
        
        // Use ? to propagate ServerError, which will be converted to ToolError::ExternalError
        // The discover method now returns the agent name/ID on success
        let agent_name = self.agent_registry.discover(agent_url).await?;
        
        // If we have known_servers, update it with the agent name returned by discover
        if let Some(known_servers) = &self.known_servers {
            debug!(%agent_url, %agent_name, "Updating known_servers map to keep in sync with agent registry."); // Changed to debug
            known_servers.insert(agent_url.to_string(), agent_name);
        }

        // Return a success message using the URL.
        let response_text = format!("Successfully discovered and remembered agent from URL '{}'.", agent_url);
        info!(%response_text); // Keep info for successful remembering
        Ok(json!(response_text)) // Return simple text confirmation
    }

    fn capabilities(&self) -> &[&'static str] { &["agent_registry", "agent_discovery", "agent_management"] }
}

/// Tool to execute internal agent commands based on natural language requests.
/// Note: This tool attempts the command logic but cannot directly modify the core agent state
/// (like self.client or self.current_session_id) due to ownership constraints.
/// It returns textual feedback about the attempt.
pub struct ExecuteCommandTool {
    agent_registry: Arc<crate::server::agent_registry::AgentRegistry>,
    known_servers: Arc<DashMap<String, String>>,
    // We need the agent's own ID and potentially name/port for some commands like 'card'
    agent_id: String,
    agent_name: String,
    bind_address: String,
    port: u16,
    // We need the LLM for the 'remote' command simulation
    llm: Option<Arc<dyn LlmClient>>,
    // We need the TaskService to interact with tasks ('task', 'artifacts', 'cancelTask', 'history')
    // This creates a potential dependency cycle if not handled carefully.
    // Let's omit TaskService interaction for now and limit the tool's scope.
    // task_service: Arc<crate::server::services::task_service::TaskService>,
}

impl ExecuteCommandTool {
    pub fn new(
        agent_registry: Arc<crate::server::agent_registry::AgentRegistry>,
        known_servers: Arc<DashMap<String, String>>,
        agent_id: String,
        agent_name: String,
        bind_address: String,
        port: u16,
        llm: Option<Arc<dyn LlmClient>>,
        // task_service: Arc<crate::server::services::task_service::TaskService>,
    ) -> Self {
        Self {
            agent_registry,
            known_servers,
            agent_id,
            agent_name,
            bind_address,
            port,
            llm,
            // task_service,
        }
    }

    // --- Helper methods mirroring BidirectionalAgent handlers (simplified) ---
    // These helpers perform the logic but return String results instead of modifying agent state.

    fn handle_list_servers_logic(&self) -> Result<String, ToolError> {
        debug!("Handling list_servers command logic within tool.");
        if self.known_servers.is_empty() {
            Ok("📡 No known servers found.".to_string())
        } else {
            let mut output = String::from("\n📡 Known Servers:\n");
            let mut server_list: Vec<(String, String)> = self.known_servers.iter()
                .map(|entry| (entry.value().clone(), entry.key().clone()))
                .collect();
            server_list.sort_by(|a, b| a.0.cmp(&b.0));
            for (i, (name, url)) in server_list.iter().enumerate() {
                // Cannot mark connected server as tool doesn't know current client state
                output.push_str(&format!("  {}: {} - {}\n", i + 1, name, url));
            }
            Ok(output)
        }
    }

    async fn handle_connect_logic(&self, target: &str) -> Result<String, ToolError> {
        debug!(target = %target, "Handling connect command logic within tool.");
        // Simplified: Only handle URL connection attempts for now
        let target_url = target.split_whitespace()
            .find(|s| s.starts_with("http://") || s.starts_with("https://"))
            .map(|s| s.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '/').to_string());

        if target_url.is_none() || target_url.as_deref() == Some("") {
            return Err(ToolError::InvalidParams("execute_command(connect)".to_string(), "No valid URL found in arguments.".to_string()));
        }
        let url = target_url.unwrap();

        let mut client = A2aClient::new(&url);
        match client.get_agent_card().await {
            Ok(card) => {
                let name = card.name.clone();
                // Update known servers (this is safe as it's Arc<DashMap>)
                self.known_servers.insert(url.clone(), name.clone());
                // Update registry (also safe)
                match self.agent_registry.discover(&url).await {
                    Ok(discovered_id) => {
                         if discovered_id != name {
                             self.known_servers.insert(url.clone(), discovered_id);
                         }
                    },
                    Err(e) => warn!("Failed to update registry during connect tool logic: {}", e),
                }
                // Cannot update self.client here
                Ok(format!("✅ Connection attempt to '{}' ({}) successful. Agent card retrieved. Use ':connect {}' in REPL to make it the active connection.", name, url, url))
            },
            Err(e) => {
                 // Add to known servers even on failure
                 if !self.known_servers.contains_key(&url) {
                     self.known_servers.insert(url.clone(), "Unknown Agent".to_string());
                 }
                 Err(ToolError::ExecutionFailed("execute_command(connect)".to_string(), format!("Failed to connect to agent at {}: {}", url, e)))
            }
        }
    }

    fn handle_disconnect_logic(&self) -> Result<String, ToolError> {
        // Cannot actually disconnect as state is not accessible
        Ok("⚠️ Cannot perform disconnect via tool. Use ':disconnect' in REPL.".to_string())
    }

    fn handle_new_session_logic(&self) -> Result<String, ToolError> {
        // Can generate an ID but cannot set it as current
        let session_id = format!("session-{}", Uuid::new_v4());
        Ok(format!("✅ Generated new session ID: {}. Use ':session new' in REPL to activate it.", session_id))
    }

    fn handle_show_session_logic(&self) -> Result<String, ToolError> {
        // Cannot access current session ID
         Ok("⚠️ Cannot show current session via tool. Use ':session show' in REPL.".to_string())
    }

    fn handle_show_card_logic(&self) -> Result<String, ToolError> {
        // Recreate card based on stored agent info
         let capabilities = AgentCapabilities {
            push_notifications: true, state_transition_history: true, streaming: true,
        };
        let card = AgentCard {
            name: self.agent_name.clone(),
            description: Some("A bidirectional A2A agent.".to_string()),
            version: AGENT_VERSION.to_string(),
            url: format!("http://{}:{}", self.bind_address, self.port),
            capabilities,
            authentication: None, default_input_modes: vec!["text".to_string()], default_output_modes: vec!["text".to_string()],
            documentation_url: None, provider: None, skills: vec![],
        };
        let mut output = String::from("\n📇 Agent Card (via tool):\n");
        output.push_str(&format!("  Name: {}\n", card.name));
        // ... (add other fields as needed) ...
        Ok(output)
    }

    // Add logic for other commands if needed (list_agents, remember_agent are separate tools)
    // Commands interacting with TaskService (history, tasks, task, artifacts, cancelTask) are omitted for now.
}


#[async_trait]
impl Tool for ExecuteCommandTool {
    fn name(&self) -> &str { "execute_command" }

    fn description(&self) -> &str {
        "Executes internal agent commands like connect, disconnect, list_servers, session new, card. \
         Note: May not fully update agent state for connect/disconnect/session. \
         Expects parameters: {\"command\": \"command_name\", \"args\": \"arguments_string\"}"
    }

    #[instrument(skip(self, params), name="tool_execute_command")]
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        debug!("Executing 'execute_command' tool.");
        tracing::trace!(?params, "Tool parameters received.");

        let command = params.get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams(self.name().to_string(), "Missing 'command' string parameter".to_string()))?
            .trim()
            .to_lowercase();

        let args = params.get("args")
            .and_then(|v| v.as_str())
            .unwrap_or("") // Default to empty string if args are missing or not a string
            .trim();

        debug!(%command, %args, "Parsed command and arguments.");

        let result_string = match command.as_str() {
            "connect" => self.handle_connect_logic(args).await,
            "disconnect" => self.handle_disconnect_logic(),
            "servers" => self.handle_list_servers_logic(),
            "session" if args == "new" => self.handle_new_session_logic(),
            "session" if args == "show" => self.handle_show_session_logic(),
            "card" => self.handle_show_card_logic(),
            // Add other command handlers here
            // Omit task-related commands for now
            "history" | "tasks" | "task" | "artifacts" | "cancelTask" => {
                 Err(ToolError::UnsupportedOperation("execute_command".to_string(), format!("Task-related command '{}' not supported via tool yet.", command)))
            }
            "remote" => {
                 // Requires LLM and potentially client state - too complex for now
                 Err(ToolError::UnsupportedOperation("execute_command".to_string(), "'remote' command not supported via tool.".to_string()))
            }
             "tool" => {
                 Err(ToolError::UnsupportedOperation("execute_command".to_string(), "Cannot call ':tool' from within execute_command tool.".to_string()))
            }
            _ => Err(ToolError::InvalidParams(self.name().to_string(), format!("Unknown internal command: '{}'", command))),
        };

        // Convert String result/error into JSON Value result/error
        match result_string {
            Ok(text) => Ok(json!(text)),
            Err(e) => Err(e),
        }
    }

    fn capabilities(&self) -> &[&'static str] { &["agent_control", "meta"] }
}


// --- End New Tools ---


/// Manages and executes available local tools using standard A2A types.
#[derive(Clone)]
pub struct ToolExecutor {
    /// Registered tools that can be executed
    pub tools: Arc<HashMap<String, Box<dyn Tool>>>,
    // Add registry for tools that need it
    agent_registry: Option<Arc<crate::server::agent_registry::AgentRegistry>>, // Use canonical type
    // REMOVED agent_directory field
    // Reference to known_servers map from BidirectionalAgent
    known_servers: Option<Arc<DashMap<String, String>>>,
    // Keep LLM client for tools that need it
    llm: Option<Arc<dyn LlmClient>>,
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
            agent_registry: None, // Initialize new fields
            // REMOVED agent_directory initialization
            known_servers: None,
            llm: None,
        }
    }

    /// Creates a new ToolExecutor with tools enabled via configuration.
    pub fn with_enabled_tools(
        enabled: &[String],
        llm: Option<Arc<dyn LlmClient>>, // Make LLM optional
        agent_registry: Option<Arc<crate::server::agent_registry::AgentRegistry>>, // Add registry parameter
        known_servers: Option<Arc<DashMap<String, String>>>, // Optional map of known servers from BidirectionalAgent
        // Need agent's own info for ExecuteCommandTool
        agent_id: &str,
        agent_name: &str,
        bind_address: &str,
        port: u16,
        // task_service: Option<Arc<crate::server::services::task_service::TaskService>>, // Omit for now
    ) -> Self {
        let mut map: HashMap<String, Box<dyn Tool>> = HashMap::new();

        // Always register the echo tool as a fallback
        map.insert("echo".into(), Box::new(EchoTool));
        tracing::debug!("Tool 'echo' registered.");

        for name in enabled {
            match name.as_str() {
                "llm" => {
                    if !map.contains_key("llm") {
                        // Check if llm Option is Some before unwrapping
                        if let Some(llm_client) = llm.clone() { // Clone the Option<Arc<...>>
                            map.insert("llm".into(), Box::new(LlmTool::new(llm_client))); // Pass the Arc directly
                            tracing::debug!("Tool 'llm' registered.");
                        } else {
                            tracing::warn!("Cannot register 'llm' tool: LLM client not provided.");
                        }
                    }
                }
                "summarize" => {
                     if !map.contains_key("summarize") {
                        // Check if llm Option is Some before unwrapping
                        if let Some(llm_client) = llm.clone() { // Clone the Option<Arc<...>>
                            map.insert("summarize".into(), Box::new(SummarizeTool::new(llm_client))); // Pass the Arc directly
                            tracing::debug!("Tool 'summarize' registered.");
                        } else {
                            tracing::warn!("Cannot register 'summarize' tool: LLM client not provided.");
                        }
                    }
                }
                "list_agents" => {
                    if !map.contains_key("list_agents") {
                        // Use agent_registry instead of agent_directory
                        if let Some(reg) = agent_registry.clone() {
                            map.insert("list_agents".into(), Box::new(ListAgentsTool::new(reg))); // Pass registry
                            tracing::debug!("Tool 'list_agents' registered.");
                        } else {
                            tracing::warn!("Cannot register 'list_agents' tool: agent_registry not provided.");
                        }
                    }
                }
                "remember_agent" => { // <-- Register the new tool
                    if !map.contains_key("remember_agent") {
                        if let Some(reg) = agent_registry.clone() {
                            // Pass both agent_registry and known_servers to the tool                            
                            map.insert("remember_agent".into(), Box::new(RememberAgentTool::new(reg, known_servers.clone())));
                            tracing::debug!("Tool 'remember_agent' registered.");
                        } else {
                            tracing::warn!("Cannot register 'remember_agent' tool: agent_registry not provided.");
                        }
                    }
                }
                 "execute_command" => { // <-- Register the new command tool
                    if !map.contains_key("execute_command") {
                        if let Some(reg) = agent_registry.clone() {
                            if let Some(ks) = known_servers.clone() {
                                // Pass registry, known_servers, and agent's own info
                                map.insert("execute_command".into(), Box::new(ExecuteCommandTool::new(
                                    reg,
                                    ks,
                                    agent_id.to_string(),
                                    agent_name.to_string(),
                                    bind_address.to_string(),
                                    port,
                                    llm.clone(), // Pass LLM if available
                                    // task_service.clone(), // Omit TaskService for now
                                )));
                                tracing::debug!("Tool 'execute_command' registered.");
                            } else {
                                tracing::warn!("Cannot register 'execute_command' tool: known_servers map not provided.");
                            }
                        } else {
                            tracing::warn!("Cannot register 'execute_command' tool: agent_registry not provided.");
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
        Self {
            tools: Arc::new(map),
            agent_registry, // Store the registry
            // REMOVED agent_directory storage
            known_servers, // Store the known_servers map
            llm, // Store the LLM client
        }
    }


    /// Executes a specific tool by name with the given JSON parameters.
    #[instrument(skip(self, params), fields(tool_name))] // Add tracing span
    pub async fn execute_tool(&self, tool_name: &str, params: Value) -> Result<Value, ToolError> {
        tracing::debug!("Attempting to execute tool."); // Add tracing
        tracing::trace!(?params, "Parameters for tool execution."); // Add tracing
        match self.tools.get(tool_name) {
            Some(tool) => {
                tracing::info!("Executing tool."); // Add tracing
                let result = tool.execute(params).await;
                match &result {
                    Ok(v) => {
                        tracing::info!("Tool execution successful."); // Add tracing
                        tracing::trace!(output = %v, "Tool output value."); // Add tracing
                    },
                    Err(e) => {
                        tracing::error!(error = %e, "Tool execution failed."); // Add tracing
                    }
                }
                result
            },
            None => {
                tracing::warn!("Tool not found in registered tools."); // Add tracing
                Err(ToolError::NotFound(tool_name.to_string()))
            },
        }
    }


    /// Executes a task locally using the specified tool and pre-extracted parameters.
    /// Updates the task state and artifacts based on the tool execution result.
    #[instrument(skip(self, task, params), fields(task_id = %task.id, tool_name))] // Add tracing span
    pub async fn execute_task_locally(&self, task: &mut Task, tool_name: &str, params: Value) -> Result<(), ServerError> {
        debug!("Executing task locally with pre-extracted parameters."); // Changed to debug
        tracing::trace!(?params, "Parameters received for tool execution."); // Add tracing

        // Check if the requested tool exists and is registered
        // If not, return a ToolError::NotFound which will be converted to ServerError
        if !self.tools.contains_key(tool_name) {
            tracing::warn!(requested_tool = %tool_name, "Requested tool not found or not registered in ToolExecutor.");
            // Add a warning message to the task history
            let warning_msg = format!("Tool '{}' not found or not enabled for this agent.", tool_name);
            task.history.get_or_insert_with(Vec::new).push(Message {
                role: Role::Agent,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: warning_msg.clone(),
                    metadata: None,
                })],
                metadata: None,
            });
            // Return an error instead of falling back to echo
            return Err(ToolError::NotFound(tool_name.to_string()).into());
        }
        
        // Execute the tool using the provided parameters
        debug!("Executing tool '{}'.", tool_name); // Changed to debug
        match self.execute_tool(tool_name, params).await {
            Ok(result_value) => {
                info!("Tool execution successful."); // Keep info for success
                tracing::trace!(result = %result_value, "Tool result value."); // Add tracing

                // Create result parts (Text and potentially Data)
                let mut result_parts: Vec<Part> = Vec::new();

                // Always add a text part with the string representation
                let result_text = result_value.to_string();
                result_parts.push(Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: result_text.clone(), // Clone for potential use in status message
                    metadata: None,
                }));
                tracing::debug!("Created TextPart for tool result."); // Add tracing

                // If the result is a JSON object or array, add a DataPart
                if result_value.is_object() || result_value.is_array() {
                     // Attempt to convert the Value directly into a Map<String, Value>
                     // This might fail if it's not an object, handle gracefully
                     let data_map = match result_value.as_object() {
                         Some(obj) => obj.clone(),
                         None => {
                             // If it's not an object (e.g., array or primitive), wrap it
                             let mut map = serde_json::Map::new();
                             map.insert("result".to_string(), result_value.clone());
                             map
                         }
                     };

                     result_parts.push(Part::DataPart(DataPart {
                         type_: "json".to_string(), // Assuming JSON data
                         data: data_map,
                         metadata: None,
                     }));
                     tracing::debug!("Created DataPart for tool result."); // Add tracing
                } else {
                    tracing::trace!("Tool result is not JSON object/array, skipping DataPart creation."); // Add tracing
                }


                // Create an artifact from the result parts
                let artifact_index = task.artifacts.as_ref().map_or(0, |a| a.len()) as i64;
                let artifact = Artifact {
                    parts: result_parts,
                    index: artifact_index,
                    name: Some(format!("{}_result", tool_name)),
                    description: Some(format!("Result from tool '{}'", tool_name)),
                    append: None,
                    last_chunk: Some(true), // Mark as last chunk for this artifact
                    metadata: None,
                };
                tracing::debug!(artifact_name = ?artifact.name, artifact_index, "Created result artifact."); // Add tracing

                // Add artifact to the task
                task.artifacts.get_or_insert_with(Vec::new).push(artifact);
                tracing::trace!("Added result artifact to task."); // Add tracing

                // Update Task Status to Completed
                task.status = TaskStatus {
                    state: TaskState::Completed,
                    timestamp: Some(Utc::now()),
                    // Provide a response message summarizing the result
                    message: Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            // Use the string representation of the result, cleaned up
                            text: result_text.trim_matches('"').to_string(),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
                info!("Updated task status to Completed."); // Keep info for state change
                tracing::trace!(?task.status, "Final task status."); // Add tracing
                Ok(())
            }
            Err(tool_error) => {
                tracing::error!(error = %tool_error, "Tool execution failed.");

                // Update Task Status to InputRequired
                task.status = TaskStatus {
                    state: TaskState::InputRequired, // <-- Set state
                    timestamp: Some(Utc::now()),
                    message: Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!(
                                "Tool execution failed: {}. How should I proceed? (e.g., 'retry', 'cancel', or provide different parameters)",
                                tool_error
                            ),
                            metadata: None,
                        })],
                        // Optional: Add metadata indicating this InputRequired is due to an error
                        metadata: Some(serde_json::json!({ "error_context": tool_error.to_string() }).as_object().unwrap().clone()),
                    }),
                };
                info!("Updated task status to InputRequired due to tool error.");
                tracing::trace!(?task.status, "Final task status after tool error.");

                // Return Ok because the task flow continues, waiting for input.
                // The error is captured in the task status.
                Ok(()) // <-- Change return
            }
        }
    }
    
    /// Process a follow-up message using a default tool (e.g., echo).
    /// This is a simplified approach; real follow-up might involve more complex logic or routing.
    #[instrument(skip(self, task, message), fields(task_id = %task.id))] // Add tracing span
    pub async fn process_follow_up(&self, task: &mut Task, message: Message) -> Result<(), ServerError> {
        debug!("Processing follow-up message for task."); // Changed to debug
        // Default to echo tool for simple follow-up processing
        let tool_name = "echo";
        tracing::debug!(%tool_name, "Using default tool for follow-up."); // Add tracing
 
        // Extract parameters from the follow-up message
        tracing::debug!("Creating simple parameters for follow-up message."); // Add tracing
        // Create simple {"text": ...} params for the echo tool
        let follow_up_text = message.parts.iter()
            .filter_map(|p| match p { Part::TextPart(tp) => Some(tp.text.as_str()), _ => None })
            .collect::<Vec<_>>().join("\n");
        let params = json!({"text": follow_up_text});
        tracing::trace!(?params, "Parameters extracted for follow-up tool execution."); // Add tracing
 
        // Execute the tool
        debug!("Executing follow-up tool '{}'.", tool_name); // Changed to debug
        match self.execute_tool(tool_name, params).await {
            Ok(result_value) => {
                info!("Follow-up tool execution successful."); // Keep info for success
                tracing::trace!(result = %result_value, "Follow-up tool result value."); // Add tracing

                // Create a text part for the result
                let result_text = result_value.to_string();
                let text_part = TextPart {
                    type_: "text".to_string(),
                    text: result_text.clone(), // Clone for status message
                    metadata: None,
                };
                tracing::debug!("Created TextPart for follow-up result."); // Add tracing

                // Create an artifact with the result
                let artifact_index = task.artifacts.as_ref().map_or(0, |a| a.len()) as i64;
                let artifact = Artifact {
                    parts: vec![Part::TextPart(text_part)],
                    index: artifact_index,
                    name: Some("follow_up_result".to_string()),
                    description: Some("Result from follow-up message processing".to_string()),
                    append: None,
                    last_chunk: Some(true),
                    metadata: None,
                };
                tracing::debug!(artifact_name = ?artifact.name, artifact_index, "Created follow-up result artifact."); // Add tracing

                // Add artifact to the task
                task.artifacts.get_or_insert_with(Vec::new).push(artifact);
                tracing::trace!("Added follow-up result artifact to task."); // Add tracing

                // Update task status - typically Completed after follow-up, but could be InputRequired again
                // For this simple echo example, we'll mark it Completed.
                task.status = TaskStatus {
                    state: TaskState::Completed, // Assume completed after echo
                    timestamp: Some(Utc::now()),
                    message: Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            // Include the tool result in the response message
                            text: result_text.trim_matches('"').to_string(),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
                info!("Updated task status to Completed after follow-up."); // Keep info for state change
                tracing::trace!(?task.status, "Final task status after follow-up."); // Add tracing
                Ok(())
            }
            Err(tool_error) => {
                tracing::error!(error = %tool_error, "Follow-up tool execution failed."); // Add tracing
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
                info!("Updated task status to Failed due to follow-up tool error."); // Keep info for state change
                tracing::trace!(?task.status, "Final task status after follow-up."); // Add tracing
                Err(tool_error.into())
            }
        }
    }
    
}
