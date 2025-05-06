//! Executes local tools with standard A2A protocol-compliant types and artifacts.

use crate::bidirectional::llm_client::LlmClient; // <-- Update import path
use crate::client::A2aClient; // <-- Import A2aClient
use crate::server::error::ServerError;
use crate::types::{
    AgentCapabilities, // <-- Import AgentCapabilities
    AgentCard,
    Artifact,
    DataPart,
    Message,
    Part,
    Role,
    Task,
    TaskState,
    TaskStatus,
    TextPart,
};
use anyhow; // Import anyhow for error handling in new tools
use dashmap::DashMap; // For known_servers sync
use uuid::Uuid; // <-- Import Uuid

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn}; // <-- Import warn macro

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

    #[error("Operation '{1}' is not supported by tool '{0}'")] // <-- Add new variant
    UnsupportedOperation(String, String),
}

// Implement conversion from std::io::Error
impl From<std::io::Error> for ToolError {
    fn from(e: std::io::Error) -> Self {
        // Corrected parameter type
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
            ToolError::NotFound(tool) => {
                ServerError::UnsupportedOperation(format!("Tool '{}' not found", tool))
            }
            ToolError::InvalidParams(tool, msg) => ServerError::InvalidParameters(format!(
                "Invalid parameters for tool '{}': {}",
                tool, msg
            )),
            ToolError::ExecutionFailed(tool, msg) => ServerError::ServerTaskExecutionFailed(
                format!("Tool '{}' execution failed: {}", tool, msg),
            ),
            ToolError::ConfigError(tool, msg) => {
                ServerError::ConfigError(format!("Tool '{}' configuration error: {}", tool, msg))
            }
            ToolError::IoError(msg) => ServerError::ServerTaskExecutionFailed(format!(
                "I/O error during tool execution: {}",
                msg
            )),
            ToolError::ExternalError(msg) => ServerError::ServerTaskExecutionFailed(format!(
                "External error during tool execution: {}",
                msg
            )),
            ToolError::UnsupportedOperation(tool, op) =>
            // <-- Handle new variant
            {
                ServerError::UnsupportedOperation(format!(
                    "Operation '{}' not supported by tool '{}'",
                    op, tool
                ))
            }
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
    fn name(&self) -> &str {
        "echo"
    }

    fn description(&self) -> &str {
        "Echoes back the input text"
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        if let Some(text) = params.get("text").and_then(|v| v.as_str()) {
            Ok(json!(format!("Echo: {}", text)))
        } else {
            Err(ToolError::InvalidParams(
                "echo".to_string(),
                "Missing 'text' parameter".to_string(),
            ))
        }
    }

    fn capabilities(&self) -> &[&'static str] {
        &["echo", "text_manipulation"]
    }
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
    fn name(&self) -> &str {
        "list_agents"
    }

    fn description(&self) -> &str {
        "Lists all known agents and their capabilities"
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        // Get format parameter if provided (default to "detailed")
        let format = params
            .get("format")
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
            }
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

    fn capabilities(&self) -> &[&'static str] {
        &["agent_directory", "agent_discovery"]
    }
}

/// Tool that directly uses the LLM based on input text
pub struct LlmTool {
    llm: Arc<dyn LlmClient>,
}
impl LlmTool {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl Tool for LlmTool {
    fn name(&self) -> &str {
        "llm"
    }
    fn description(&self) -> &str {
        "Ask the LLM directly using the provided text as a prompt"
    }
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let prompt = params
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if prompt.is_empty() {
            return Err(ToolError::InvalidParams(
                "llm".to_string(),
                "'text' parameter cannot be empty".to_string(),
            ));
        }
        // Use ? to propagate anyhow::Error, which will be converted by From<anyhow::Error> for ToolError
        let result = self.llm.complete(prompt, None).await?;
        Ok(json!(result))
    }
    fn capabilities(&self) -> &[&'static str] {
        &["llm", "chat", "general_purpose"]
    }
}

/// Tool that asks the LLM to summarize the input text
pub struct SummarizeTool {
    llm: Arc<dyn LlmClient>,
}
impl SummarizeTool {
    pub fn new(llm: Arc<dyn LlmClient>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl Tool for SummarizeTool {
    fn name(&self) -> &str {
        "summarize"
    }
    fn description(&self) -> &str {
        "Summarize the received text using the LLM"
    }
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        let text = params
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if text.is_empty() {
            return Err(ToolError::InvalidParams(
                "summarize".to_string(),
                "'text' parameter cannot be empty".to_string(),
            ));
        }
        let prompt = format!("Briefly summarize the following text:\n\n{}", text);
        // Use ? to propagate anyhow::Error
        let result = self.llm.complete(&prompt, None).await?;
        Ok(json!(result))
    }
    fn capabilities(&self) -> &[&'static str] {
        &["summarize", "text_manipulation"]
    }
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
        known_servers: Option<Arc<DashMap<String, String>>>,
    ) -> Self {
        Self {
            agent_registry,
            known_servers,
        }
    }
}

#[async_trait]
impl Tool for RememberAgentTool {
    fn name(&self) -> &str {
        "remember_agent"
    }

    fn description(&self) -> &str {
        "Discovers and stores/updates another agent's information in the registry using its base URL, and automatically connects to it."
    }

    #[instrument(skip(self, params), name = "tool_remember_agent")]
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        debug!("Executing 'remember_agent' tool."); // Changed to debug
        tracing::trace!(?params, "Tool parameters received.");

        // Extract the agent_base_url parameter
        tracing::debug!("Extracting 'agent_base_url' parameter.");
        let agent_url = params
            .get("agent_base_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                tracing::warn!("Missing or invalid 'agent_base_url' parameter.");
                ToolError::InvalidParams(
                    self.name().to_string(),
                    "Missing or invalid string parameter: 'agent_base_url'".to_string(),
                )
            })?
            .trim(); // Trim whitespace

        if agent_url.is_empty() {
            tracing::warn!("'agent_base_url' parameter is empty.");
            return Err(ToolError::InvalidParams(
                self.name().to_string(),
                "'agent_base_url' cannot be empty".to_string(),
            ));
        }
        // Basic URL validation (can be enhanced)
        if !agent_url.starts_with("http://") && !agent_url.starts_with("https://") {
            tracing::warn!(%agent_url, "Invalid URL format for 'agent_base_url'.");
            return Err(ToolError::InvalidParams(
                self.name().to_string(),
                "'agent_base_url' must start with http:// or https://".to_string(),
            ));
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
            known_servers.insert(agent_url.to_string(), agent_name.clone());
        }

        // Simulate a connection to provide connection feedback
        debug!(%agent_url, %agent_name, "Auto-connecting to agent after remembering it");
        let client = A2aClient::new(agent_url);

        // Try to connect and get more detailed connection info
        let connection_result = match client.get_agent_card().await {
            Ok(card) => {
                let connection_message =
                    format!("✅ Successfully connected to agent: {}", card.name);
                debug!(%agent_url, agent_name = %card.name, "Successfully connected to remembered agent");
                connection_message
            }
            Err(e) => {
                warn!(error = %e, %agent_url, "Connection attempt after remembering agent failed");
                format!("⚠️ Agent remembered but connection failed: {}", e)
            }
        };

        // Create a comprehensive response that includes both remembering and connection
        let response_text = format!(
            "🔄 Agent Operations Complete:\n\n✅ Successfully discovered and remembered agent from URL '{}'.\n\n{}",
            agent_url, connection_result
        );

        info!(%response_text); // Keep info for successful remembering
        Ok(json!(response_text)) // Return comprehensive text confirmation
    }

    fn capabilities(&self) -> &[&'static str] {
        &["agent_registry", "agent_discovery", "agent_management"]
    }
}

/// Tool that handles tasks requiring human input
/// This is used when a task in InputRequired state is returned from
/// agent B back to agent A, and the LLM decides human input is needed
pub struct HumanInputTool;

#[async_trait]
impl Tool for HumanInputTool {
    fn name(&self) -> &str {
        "human_input"
    }

    fn description(&self) -> &str {
        "Handles tasks that require human input through the REPL interface. Used for returning tasks in InputRequired state that need human expertise."
    }

    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        // Extract text and prompt from parameters
        let text = params
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let prompt = params
            .get("prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("Additional input required.");

        // Generate a message informing the user about the required input
        let human_prompt = format!(
            "🔄 This task requires human input.\n\n\
            Original follow-up message: {}\n\n\
            Reason input is needed: {}\n\n\
            Please provide the required information through the REPL interface.",
            text, prompt
        );

        // Return the message as result - this will get displayed to the user
        Ok(json!(human_prompt))
    }

    fn capabilities(&self) -> &[&'static str] {
        &["human_interaction", "input_required"]
    }
}

/// Tool to list known servers
pub struct ListServersTool {
    known_servers: Arc<DashMap<String, String>>,
}

impl ListServersTool {
    pub fn new(known_servers: Arc<DashMap<String, String>>) -> Self {
        Self { known_servers }
    }
}

#[async_trait]
impl Tool for ListServersTool {
    fn name(&self) -> &str {
        "list_servers"
    }

    fn description(&self) -> &str {
        "Lists all known remote servers this agent can connect to."
    }

    async fn execute(&self, _params: Value) -> Result<Value, ToolError> {
        if self.known_servers.is_empty() {
            Ok(json!("No known servers found."))
        } else {
            let mut server_list: Vec<HashMap<String, String>> = self
                .known_servers
                .iter()
                .map(|entry| {
                    let mut map = HashMap::new();
                    map.insert("name".to_string(), entry.value().clone());
                    map.insert("url".to_string(), entry.key().clone());
                    map
                })
                .collect();
            server_list.sort_by(|a, b| a.get("name").unwrap_or(&String::new()).cmp(b.get("name").unwrap_or(&String::new())));
            Ok(json!({
                "count": server_list.len(),
                "servers": server_list
            }))
        }
    }

    fn capabilities(&self) -> &[&'static str] {
        &["agent_discovery", "meta"]
    }
}

/// Tool to show the agent's own card
pub struct ShowCardTool {
    agent_name: String,
    agent_id: String,
    agent_version: String,
    bind_address: String,
    port: u16,
}

impl ShowCardTool {
    pub fn new(
        agent_name: String,
        agent_id: String,
        agent_version: String,
        bind_address: String,
        port: u16,
    ) -> Self {
        Self {
            agent_name,
            agent_id,
            agent_version,
            bind_address,
            port,
        }
    }
}

#[async_trait]
impl Tool for ShowCardTool {
    fn name(&self) -> &str {
        "show_agent_card"
    }

    fn description(&self) -> &str {
        "Displays the current agent's own information card."
    }

    async fn execute(&self, _params: Value) -> Result<Value, ToolError> {
        let capabilities = AgentCapabilities {
            push_notifications: true,
            state_transition_history: true,
            streaming: true,
        };
        let card = AgentCard {
            name: self.agent_name.clone(),
            description: Some(format!("Bidirectional A2A Agent (ID: {})", self.agent_id)),
            url: format!("http://{}:{}", self.bind_address, self.port),
            version: self.agent_version.clone(),
            capabilities,
            authentication: None,
            default_input_modes: vec!["text".to_string()],
            default_output_modes: vec!["text".to_string()],
            documentation_url: None,
            provider: None,
            skills: vec![], // Skills could be dynamically added if ToolExecutor knows them
        };
        serde_json::to_value(card).map_err(|e| ToolError::ExecutionFailed("show_agent_card".to_string(), e.to_string()))
    }

    fn capabilities(&self) -> &[&'static str] {
        &["meta", "self_info"]
    }
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
    agent_version: String, // <-- Add agent_version field
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
        agent_version: String, // <-- Add agent_version parameter
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
            agent_version, // <-- Store agent_version
            bind_address,
            port,
            llm,
            // task_service,
        }
    }

    // --- Helper methods mirroring BidirectionalAgent handlers (simplified) ---
    // These helpers perform the logic but return String results instead of modifying agent state.
    // Most of these will be removed as their functionality moves to AgentActions or dedicated Tools.

    // Add logic for other commands if needed (list_agents, remember_agent are separate tools)
    // Commands interacting with TaskService (history, tasks, task, artifacts, cancelTask) are omitted for now.
}

#[async_trait]
impl Tool for ExecuteCommandTool {
    fn name(&self) -> &str {
        "execute_command" // This tool might become very minimal or be removed.
    }

    fn description(&self) -> &str {
        "Executes specific, limited internal agent commands. Most internal operations are now AgentActions or dedicated Tools. \
         Expects parameters: {\"command\": \"command_name\", \"args\": \"arguments_string\"}"
    }

    #[instrument(skip(self, params), name = "tool_execute_command")]
    async fn execute(&self, params: Value) -> Result<Value, ToolError> {
        debug!("Executing 'execute_command' tool.");
        tracing::trace!(?params, "Tool parameters received.");

        let command = params
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidParams(
                    self.name().to_string(),
                    "Missing 'command' string parameter".to_string(),
                )
            })?
            .trim()
            .to_lowercase();

        let _args = params // args might still be useful for some commands
            .get("args")
            .and_then(|v| v.as_str())
            .unwrap_or("") 
            .trim();

        debug!(%command, %_args, "Parsed command and arguments.");

        // Most command logic is removed. Only keep what's truly an "execute_command" type action.
        // Or, this tool could be deprecated if all functionality is moved.
        // For now, let's make it return an error for most things.
        let result_string = match command.as_str() {
            // Example: Keep a very specific command if needed, otherwise error.
            // "some_specific_meta_command" => Ok("Specific meta command executed.".to_string()),
            "connect" | "disconnect" | "servers" | "session" | "card" |
            "history" | "tasks" | "task" | "artifacts" | "cancelTask" | "remote" | "tool" => {
                Err(ToolError::UnsupportedOperation(
                    self.name().to_string(),
                    format!(
                        "Command '{}' is now handled by a dedicated AgentAction or Tool, not 'execute_command'.",
                        command
                    ),
                ))
            }
            _ => Err(ToolError::InvalidParams(
                self.name().to_string(),
                format!("Unknown or unsupported internal command for 'execute_command' tool: '{}'", command),
            )),
        };

        // Convert String result/error into JSON Value result/error
        match result_string {
            Ok(text) => Ok(json!(text)),
            Err(e) => Err(e),
        }
    }

    fn capabilities(&self) -> &[&'static str] {
        &["meta"] // Capabilities might change based on what's left.
    }
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

impl Default for ToolExecutor {
    fn default() -> Self {
        Self::new()
    }
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
        tool_list: &[String],
        is_exclusion_list: bool,
        llm: Option<Arc<dyn LlmClient>>, // Make LLM optional
        agent_registry: Option<Arc<crate::server::agent_registry::AgentRegistry>>, // Add registry parameter
        known_servers: Option<Arc<DashMap<String, String>>>, // Optional map of known servers from BidirectionalAgent
        // Need agent's own info for ExecuteCommandTool
        agent_id: &str,
        agent_name: &str,
        agent_version: &str, // <-- Add agent_version parameter
        bind_address: &str,
        port: u16,
        // task_service: Option<Arc<crate::server::services::task_service::TaskService>>, // Omit for now
    ) -> Self {
        let mut map: HashMap<String, Box<dyn Tool>> = HashMap::new();

        // Always register the echo tool as a fallback
        map.insert("echo".into(), Box::new(EchoTool));
        tracing::debug!("Tool 'echo' registered.");

        // Always register the human_input tool as it's essential for InputRequired handling
        map.insert("human_input".into(), Box::new(HumanInputTool));
        tracing::debug!("Tool 'human_input' registered.");

        // Define a function to determine if a tool should be enabled
        let should_enable = |name: &str| -> bool {
            if is_exclusion_list {
                // If it's an exclusion list, enable the tool if it's NOT in the list
                !tool_list.iter().any(|x| x == name)
            } else {
                // If it's an inclusion list, enable the tool if it IS in the list, or if the list is empty (enable all)
                tool_list.iter().any(|x| x == name) || tool_list.is_empty()
            }
        };
        
        // Register LLM tool if it should be enabled
        if should_enable("llm") {
            if !map.contains_key("llm") {
                // Check if llm Option is Some before unwrapping
                if let Some(llm_client) = llm.clone() {
                    // Clone the Option<Arc<...>>
                    map.insert("llm".into(), Box::new(LlmTool::new(llm_client))); // Pass the Arc directly
                    tracing::debug!("Tool 'llm' registered.");
                } else {
                    tracing::warn!("Cannot register 'llm' tool: LLM client not provided.");
                }
            }
        }
        
        // Register summarize tool if it should be enabled
        if should_enable("summarize") {
            if !map.contains_key("summarize") {
                // Check if llm Option is Some before unwrapping
                if let Some(llm_client) = llm.clone() {
                    // Clone the Option<Arc<...>>
                    map.insert(
                        "summarize".into(),
                        Box::new(SummarizeTool::new(llm_client)),
                    ); // Pass the Arc directly
                    tracing::debug!("Tool 'summarize' registered.");
                } else {
                    tracing::warn!(
                        "Cannot register 'summarize' tool: LLM client not provided."
                    );
                }
            }
        }
        
        // Register list_agents tool if it should be enabled
        if should_enable("list_agents") {
            if !map.contains_key("list_agents") {
                // Use agent_registry instead of agent_directory
                if let Some(reg) = agent_registry.clone() {
                    map.insert("list_agents".into(), Box::new(ListAgentsTool::new(reg))); // Pass registry
                    tracing::debug!("Tool 'list_agents' registered.");
                } else {
                    tracing::warn!(
                        "Cannot register 'list_agents' tool: agent_registry not provided."
                    );
                }
            }
        }
        
        // Register remember_agent tool if it should be enabled
        if should_enable("remember_agent") {
            // <-- Register the new tool
            if !map.contains_key("remember_agent") {
                if let Some(reg) = agent_registry.clone() {
                    // Pass both agent_registry and known_servers to the tool
                    map.insert(
                        "remember_agent".into(),
                        Box::new(RememberAgentTool::new(reg, known_servers.clone())),
                    );
                    tracing::debug!("Tool 'remember_agent' registered.");
                } else {
                    tracing::warn!("Cannot register 'remember_agent' tool: agent_registry not provided.");
                }
            }
        }
        
        // Register execute_command tool if it should be enabled
        if should_enable("execute_command") {
            // <-- Register the new command tool
            if !map.contains_key("execute_command") {
                if let Some(reg) = agent_registry.clone() {
                    if let Some(ks) = known_servers.clone() {
                        // Pass registry, known_servers, and agent's own info
                        map.insert(
                            "execute_command".into(),
                            Box::new(ExecuteCommandTool::new(
                                reg,
                                ks,
                                agent_id.to_string(),
                                agent_name.to_string(),
                                agent_version.to_string(), 
                                bind_address.to_string(),
                                port,
                                llm.clone(), 
                            )),
                        );
                        tracing::debug!("Tool 'execute_command' registered.");
                    } else {
                        tracing::warn!("Cannot register 'execute_command' tool: known_servers map not provided.");
                    }
                } else {
                    tracing::warn!("Cannot register 'execute_command' tool: agent_registry not provided.");
                }
            }
        }

        // Register ListServersTool if enabled
        if should_enable("list_servers") {
            if !map.contains_key("list_servers") {
                if let Some(ks) = known_servers.clone() {
                    map.insert("list_servers".into(), Box::new(ListServersTool::new(ks)));
                    tracing::debug!("Tool 'list_servers' registered.");
                } else {
                     tracing::warn!("Cannot register 'list_servers' tool: known_servers map not provided.");
                }
            }
        }

        // Register ShowCardTool if enabled
        if should_enable("show_agent_card") {
            if !map.contains_key("show_agent_card") {
                map.insert("show_agent_card".into(), Box::new(ShowCardTool::new(
                    agent_name.to_string(),
                    agent_id.to_string(),
                    agent_version.to_string(),
                    bind_address.to_string(),
                    port,
                )));
                tracing::debug!("Tool 'show_agent_card' registered.");
            }
        }
        
        // Log any unknown tools in the configuration list if not an exclusion list
        if !is_exclusion_list {
            for name in tool_list {
                match name.as_str() {
                    "echo" | "human_input" | "llm" | "summarize" | "list_agents" | "remember_agent" | "execute_command" | "list_servers" | "show_agent_card" => {
                        // Known tool, already handled
                    },
                    unknown => {
                        tracing::warn!("Unknown tool '{}' in tool configuration list. Ignoring.", unknown);
                    }
                }
            }
        }

        tracing::debug!("ToolExecutor initialized with tools: {:?}", map.keys());
        Self {
            tools: Arc::new(map),
            agent_registry, // Store the registry
            // REMOVED agent_directory storage
            known_servers, // Store the known_servers map
            llm,           // Store the LLM client
        }
    }

    /// Executes a specific tool by name with the given JSON parameters.
    #[instrument(skip(self, params), fields(tool_name))] // Add tracing span
    pub async fn execute_tool(&self, tool_name: &str, params: Value) -> Result<Value, ToolError> {
        tracing::debug!("Attempting to execute tool."); // Add tracing
        tracing::trace!(?params, "Parameters for tool execution."); // Add tracing
        match self.tools.get(tool_name) {
            Some(tool) => {
                tracing::debug!("Executing tool."); // Changed to debug
                let result = tool.execute(params).await;
                match &result {
                    Ok(v) => {
                        tracing::debug!("Tool execution successful."); // Changed to debug
                        tracing::trace!(output = %v, "Tool output value."); // Add tracing
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Tool execution failed."); // Add tracing
                    }
                }
                result
            }
            None => {
                tracing::warn!("Tool not found in registered tools."); // Add tracing
                Err(ToolError::NotFound(tool_name.to_string()))
            }
        }
    }

    /// Executes a task locally using the specified tool and pre-extracted parameters.
    /// Updates the task state and artifacts based on the tool execution result.
    #[instrument(skip(self, task, params), fields(task_id = %task.id, tool_name))] // Add tracing span
    pub async fn execute_task_locally(
        &self,
        task: &mut Task,
        tool_name: &str,
        params: Value,
    ) -> Result<(), ServerError> {
        debug!("Executing task locally with pre-extracted parameters."); // Changed to debug
        tracing::trace!(?params, "Parameters received for tool execution."); // Add tracing

        // Check if the requested tool exists and is registered
        // If not, return a ToolError::NotFound which will be converted to ServerError
        if !self.tools.contains_key(tool_name) {
            tracing::warn!(requested_tool = %tool_name, "Requested tool not found or not registered in ToolExecutor.");
            // Add a warning message to the task history
            let warning_msg = format!(
                "Tool '{}' not found or not enabled for this agent.",
                tool_name
            );
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
                debug!("Tool execution successful."); // Changed to debug
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
                    tracing::trace!(
                        "Tool result is not JSON object/array, skipping DataPart creation."
                    ); // Add tracing
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
                debug!("Updated task status to Completed."); // Changed to debug
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
                debug!("Updated task status to InputRequired due to tool error.");
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
    pub async fn process_follow_up(
        &self,
        task: &mut Task,
        message: Message,
    ) -> Result<(), ServerError> {
        debug!("Processing follow-up message for task."); // Changed to debug
                                                          // Default to echo tool for simple follow-up processing
        let tool_name = "echo";
        tracing::debug!(%tool_name, "Using default tool for follow-up."); // Add tracing

        // Extract parameters from the follow-up message
        tracing::debug!("Creating simple parameters for follow-up message."); // Add tracing
                                                                              // Create simple {"text": ...} params for the echo tool
        let follow_up_text = message
            .parts
            .iter()
            .filter_map(|p| match p {
                Part::TextPart(tp) => Some(tp.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");
        let params = json!({"text": follow_up_text});
        tracing::trace!(
            ?params,
            "Parameters extracted for follow-up tool execution."
        ); // Add tracing

        // Execute the tool
        debug!("Executing follow-up tool '{}'.", tool_name); // Changed to debug
        match self.execute_tool(tool_name, params).await {
            Ok(result_value) => {
                debug!("Follow-up tool execution successful."); // Changed to debug
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
                debug!("Updated task status to Completed after follow-up."); // Changed to debug
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
                debug!("Updated task status to Failed due to follow-up tool error."); // Changed to debug
                tracing::trace!(?task.status, "Final task status after follow-up."); // Add tracing
                Err(tool_error.into())
            }
        }
    }
}
