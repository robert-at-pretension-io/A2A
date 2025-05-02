//! LLM-powered routing and task decomposition with A2A protocol support
//!
//! This module provides the necessary components for routing tasks using
//! LLM-based decision making, with proper support for multi-turn conversations
//! through the A2A protocol's InputRequired state.

// LLM client will be implemented later
// pub mod claude_client;
// pub use self::claude_client::{LlmClient, LlmClientConfig};

/// Agent for synthesizing results from subtasks 
pub struct SynthesisAgent {
    /// LLM client for synthesis will be implemented later
    llm_client: Option<String>,
}

/// Create a synthesis agent with an API key
pub fn create_synthesis_agent(api_key: Option<String>) -> Result<Arc<SynthesisAgent>, AgentError> {
    // Placeholder implementation - will be fully implemented later
    Ok(Arc::new(SynthesisAgent { llm_client: api_key }))
}

use crate::bidirectional_agent::{
    agent_registry::AgentRegistry,
    error::AgentError,
    task_router::{RoutingDecision, SubtaskDefinition},
    tool_executor::ToolExecutor,
    types::{create_text_message, extract_tool_call, get_metadata_ext, set_metadata_ext},
    llm_core::{LlmClient, LlmClientConfig, LlmMessage},
};
use crate::types::{Message, Part, Role, TaskSendParams, TaskState, TextPart};
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Namespace for LLM routing metadata
const LLM_ROUTING_NAMESPACE: &str = "a2a.bidirectional.llm_routing";

/// Configuration for LLM-powered routing.
#[derive(Debug, Clone)]
pub struct LlmRoutingConfig {
    /// Max tokens to generate for routing decisions
    pub max_tokens: u32,
    
    /// Temperature for routing decisions (0.0 - 1.0)
    pub temperature: f32,
    
    /// Model to use for routing (e.g., "claude-3-haiku-20240307")
    pub model: String,
    
    /// Request timeout in seconds
    pub timeout_seconds: u64,
    
    /// Whether to enable multi-turn conversations
    pub enable_multi_turn: bool,
    
    /// Whether to enable task decomposition
    pub enable_decomposition: bool,
}

impl Default for LlmRoutingConfig {
    fn default() -> Self {
        Self {
            max_tokens: 2048,
            temperature: 0.1,
            model: "claude-3-haiku-20240307".to_string(),
            timeout_seconds: 30,
            enable_multi_turn: true,
            enable_decomposition: true,
        }
    }
}

/// Interface for routing agents that can make decisions on task handling
#[async_trait]
pub trait RoutingAgentTrait: Send + Sync {
    /// Makes a routing decision based on the task
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError>;
    
    /// Processes a follow-up message for multi-turn conversations
    async fn process_follow_up(&self, task_id: &str, message: &Message) -> Result<RoutingDecision, AgentError>;
    
    /// Checks if a task should be decomposed into subtasks
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, AgentError>;
    
    /// Decomposes a task into subtasks
    async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, AgentError>;
}

/// Implementation of RoutingAgentTrait using LLM for decision making
pub struct RoutingAgent {
    /// Agent registry for looking up available agents
    agent_registry: Arc<AgentRegistry>,
    
    /// Tool executor for local execution
    tool_executor: Arc<ToolExecutor>,
    
    /// LLM client for making routing decisions - we'll implement this properly later
    llm_client: Option<String>,
    
    /// Configuration for LLM routing
    config: LlmRoutingConfig,
    
    /// Cache for conversation context
    conversation_context: DashMap<String, Vec<ContextMessage>>,
}

/// Structure for storing conversation context
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContextMessage {
    /// The role of the message sender
    role: String,
    
    /// The content of the message
    content: String,
}

/// Keys used for metadata storage
struct MetadataKeys;

impl MetadataKeys {
    /// Key for storing conversation context ID
    const CONVERSATION_ID: &'static str = "conversation_id";
    
    /// Key for storing conversation state
    const CONVERSATION_STATE: &'static str = "conversation_state";
    
    /// Key for storing routing stage
    const ROUTING_STAGE: &'static str = "routing_stage";
}

impl RoutingAgent {
    /// Creates a new routing agent with LLM capabilities.
    pub fn new(
        agent_registry: Arc<AgentRegistry>,
        tool_executor: Arc<ToolExecutor>,
        config: LlmRoutingConfig,
        api_key: Option<String>,
    ) -> Result<Self, AgentError> {
        // Initialize LLM client if API key is provided
        let llm_client = match api_key {
            Some(key) => {
                let client_config = LlmClientConfig {
                    api_key: key,
                    model: config.model.clone(),
                    max_tokens: config.max_tokens,
                    temperature: config.temperature,
                    timeout_seconds: config.timeout_seconds,
                };
                
                match LlmClient::new(client_config) {
                    Ok(client) => Some(client),
                    Err(e) => {
                        warn!("Failed to initialize LLM client: {}", e);
                        None
                    }
                }
            },
            None => None,
        };
        
        Ok(Self {
            agent_registry,
            tool_executor,
            llm_client,
            config,
            conversation_context: DashMap::new(),
        })
    }
    
    /// Initialize a new conversation context
    fn initialize_context(&self, task_id: &str, initial_prompt: &str) -> String {
        let conversation_id = format!("conv-{}", Uuid::new_v4());
        
        let system_message = ContextMessage {
            role: "system".to_string(),
            content: "You are a helpful AI assistant that makes routing decisions for an agent-to-agent system.".to_string(),
        };
        
        let user_message = ContextMessage {
            role: "user".to_string(),
            content: initial_prompt.to_string(),
        };
        
        self.conversation_context.insert(
            conversation_id.clone(),
            vec![system_message, user_message],
        );
        
        conversation_id
    }
    
    /// Add a message to an existing conversation context
    fn add_to_context(&self, conversation_id: &str, role: &str, content: &str) -> Result<(), AgentError> {
        let mut context = self.conversation_context
            .get_mut(conversation_id)
            .ok_or_else(|| AgentError::RoutingError(format!(
                "Conversation context not found: {}", conversation_id
            )))?;
        
        context.push(ContextMessage {
            role: role.to_string(),
            content: content.to_string(),
        });
        
        Ok(())
    }
    
    /// Get the conversation context for a task
    fn get_context(&self, conversation_id: &str) -> Result<Vec<ContextMessage>, AgentError> {
        self.conversation_context
            .get(conversation_id)
            .map(|ctx| ctx.clone())
            .ok_or_else(|| AgentError::RoutingError(format!(
                "Conversation context not found: {}", conversation_id
            )))
    }
    
    /// Extract text from a message
    fn extract_text_from_message(&self, message: &Message) -> String {
        let mut text = String::new();
        
        for part in &message.parts {
            if let Part::TextPart(text_part) = part {
                if !text.is_empty() {
                    text.push_str("\n\n");
                }
                text.push_str(&text_part.text);
            }
        }
        
        text
    }
    
    /// Decide how to route a task
    async fn decide_routing(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError> {
        debug!("Making routing decision for task '{}'", params.id);
        
        // Extract task text
        let task_text = self.extract_text_from_message(&params.message);
        if task_text.is_empty() {
            warn!("No text found in task params for '{}', using fallback routing", params.id);
            return self.fallback_routing();
        }
        
        // Look for tool calls in the message
        let tool_calls = self.extract_tool_calls(&params.message);
        if !tool_calls.is_empty() {
            // If there are explicit tool calls, use them directly
            debug!("Found explicit tool calls in task '{}': {:?}", params.id, tool_calls);
            return Ok(RoutingDecision::Local {
                tool_names: tool_calls.into_iter().map(|(name, _)| name).collect(),
            });
        }
        
        // Get available tools and agents
        let tools: Vec<String> = self.tool_executor.available_tools().clone();
        let available_agents = self.get_available_agents().await;
        
        // Create LLM prompt for routing
        let prompt = self.create_routing_prompt(&task_text, &tools, &available_agents);
        
        // If LLM client is not available, use fallback routing
        if self.llm_client.is_none() {
            warn!("No LLM client available for task '{}', using fallback routing", params.id);
            return self.fallback_routing();
        }
        
        // Use LLM to decide routing
        match self.llm_routing_decision(&prompt).await {
            Ok(decision) => {
                debug!("LLM routing decision for task '{}': {:?}", params.id, decision);
                
                // Store conversation context for multi-turn support
                let conversation_id = self.initialize_context(&params.id, &prompt);
                
                // Add the LLM's response to the conversation context
                let response_text = match &decision {
                    RoutingDecision::Local { tool_names } => {
                        format!("I'll handle this task locally using these tools: {:?}", tool_names)
                    },
                    RoutingDecision::Remote { agent_id } => {
                        format!("I'll delegate this task to agent '{}'", agent_id)
                    },
                    RoutingDecision::Decompose { subtasks } => {
                        format!("I'll break this task down into {} subtasks", subtasks.len())
                    },
                    RoutingDecision::Reject { reason } => {
                        format!("I'll reject this task: {}", reason)
                    },
                };
                
                self.add_to_context(&conversation_id, "assistant", &response_text)?;
                
                // Store conversation ID in task metadata if params has metadata
                if let Some(ref mut metadata) = params.metadata.clone() {
                    let _ = set_metadata_ext(metadata, LLM_ROUTING_NAMESPACE, MetadataKeys::CONVERSATION_ID, conversation_id);
                }
                
                Ok(decision)
            },
            Err(e) => {
                warn!("LLM routing failed for task '{}': {}, using fallback routing", params.id, e);
                self.fallback_routing()
            }
        }
    }
    
    /// Creates a prompt for LLM routing
    fn create_routing_prompt(&self, task_text: &str, tools: &[String], agents: &[(String, String)]) -> String {
        // Construct tool descriptions
        let mut tools_description = String::new();
        if !tools.is_empty() {
            tools_description.push_str("Available tools:\n");
            for tool in tools {
                let tool_description = match tool.as_str() {
                    "directory" => "Directory tool: Query and manage the agent directory",
                    "echo" => "Echo tool: Simple tool that returns the input text",
                    "http" => "HTTP tool: Make HTTP requests to external services",
                    "shell" => "Shell tool: Execute shell commands (careful with this one)",
                    _ => "Custom tool with unknown capabilities",
                };
                tools_description.push_str(&format!("- {}: {}\n", tool, tool_description));
            }
        } else {
            tools_description.push_str("No local tools are available.\n");
        }
        
        // Construct agent descriptions
        let mut agents_description = String::new();
        if !agents.is_empty() {
            agents_description.push_str("Available agents for delegation:\n");
            for (agent_id, agent_desc) in agents {
                agents_description.push_str(&format!("- {}\n", agent_desc));
            }
        } else {
            agents_description.push_str("No remote agents are available for delegation.\n");
        }
        
        // Determine if multi-turn is enabled
        let multi_turn_option = if self.config.enable_multi_turn {
            "3. MULTI_TURN: Break the task into a multi-turn conversation where you ask for more information\n"
        } else {
            ""
        };
        
        // Create routing prompt with improved formatting
        format!(
            "# Task Routing Decision\n\n\
            You are a task router for a bidirectional A2A agent system. Your job is to determine the best way to handle a task.\n\n\
            ## Task Description\n{}\n\n\
            ## Routing Options\n\
            1. LOCAL: Handle the task locally using one or more tools\n\
            {}\n\
            2. REMOTE: Delegate the task to another agent\n\
            {}\n\
            {}\
            4. REJECT: Reject the task if it's inappropriate or impossible to handle\n\n\
            ## Instructions\n\
            Analyze the task and decide how to handle it based on the available options.\n\
            - Choose LOCAL if the task can be solved with the available tools\n\
            - Choose REMOTE if another agent would be better suited for the task\n\
            - Choose MULTI_TURN if you need more information from the user to proceed\n\
            - Choose REJECT only if the task is invalid, harmful, or impossible to complete\n\n\
            ## Response Format Requirements\n\
            Return ONLY a JSON object with no additional text, explanations, or decorations.\n\
            The JSON MUST follow the structure corresponding to your decision:\n\
            \n\
            For LOCAL decision:\n\
            {{\n  \
                \"decision_type\": \"LOCAL\",\n  \
                \"reason\": \"Brief explanation of your decision\",\n  \
                \"tool_names\": [\"tool1\", \"tool2\"]\n\
            }}\n\
            \n\
            For REMOTE decision:\n\
            {{\n  \
                \"decision_type\": \"REMOTE\",\n  \
                \"reason\": \"Brief explanation of your decision\",\n  \
                \"agent_id\": \"agent_id\"\n\
            }}\n\
            \n\
            For MULTI_TURN decision:\n\
            {{\n  \
                \"decision_type\": \"MULTI_TURN\",\n  \
                \"reason\": \"Brief explanation of why you need more information\",\n  \
                \"prompt\": \"Specific question to ask the user for more information\"\n\
            }}\n\
            \n\
            For REJECT decision:\n\
            {{\n  \
                \"decision_type\": \"REJECT\",\n  \
                \"reason\": \"Detailed explanation of why the task cannot be handled\"\n\
            }}\n\
            \n\n\
            DO NOT include any other text, markdown formatting, or explanations outside the JSON.\n\
            DO NOT use non-existent tools or agents - only use the ones listed above.\n\
            If LOCAL decision, you MUST include at least one valid tool from the available tools list.\n\
            If REMOTE decision, you MUST include a valid agent_id from the available agents list.",
            task_text,
            tools_description,
            agents_description,
            multi_turn_option
        )
    }
    
    /// Process a follow-up message for a task
    async fn process_follow_up_internal(&self, task_id: &str, message: &Message, conversation_id: &str) -> Result<RoutingDecision, AgentError> {
        debug!("Processing follow-up for task '{}' (conversation '{}')", task_id, conversation_id);
        
        // Extract text from the follow-up message
        let message_text = self.extract_text_from_message(message);
        if message_text.is_empty() {
            warn!("Empty follow-up message for task '{}', using local execution", task_id);
            return Ok(RoutingDecision::Local {
                tool_names: vec!["echo".to_string()],
            });
        }
        
        // Check for tool calls in the follow-up message
        let tool_calls = self.extract_tool_calls(message);
        if !tool_calls.is_empty() {
            // If there are explicit tool calls, use them directly
            debug!("Found tool calls in follow-up for task '{}': {:?}", task_id, tool_calls);
            return Ok(RoutingDecision::Local {
                tool_names: tool_calls.into_iter().map(|(name, _)| name).collect(),
            });
        }
        
        // Get conversation context
        let mut context = self.get_context(conversation_id)?;
        
        // Add user follow-up to context
        context.push(ContextMessage {
            role: "user".to_string(),
            content: message_text.clone(),
        });
        
        // Update the context in storage
        self.conversation_context.insert(conversation_id.to_string(), context.clone());
        
        // If LLM client is not available, use fallback local execution
        if self.llm_client.is_none() {
            warn!("No LLM client available for follow-up on task '{}', using local execution", task_id);
            return Ok(RoutingDecision::Local {
                tool_names: vec!["echo".to_string()],
            });
        }
        
        // Get available tools and agents
        let tools: Vec<String> = self.tool_executor.available_tools().clone();
        let available_agents = self.get_available_agents().await;
        
        // Create prompt for follow-up routing
        let prompt = self.create_follow_up_prompt(&message_text, &tools, &available_agents);
        
        // Use LLM to decide routing for the follow-up
        match self.llm_routing_decision(&prompt).await {
            Ok(decision) => {
                debug!("LLM routing decision for follow-up on task '{}': {:?}", task_id, decision);
                
                // Add the LLM's response to the conversation context
                let response_text = match &decision {
                    RoutingDecision::Local { tool_names } => {
                        format!("I'll handle this follow-up with these tools: {:?}", tool_names)
                    },
                    RoutingDecision::Remote { agent_id } => {
                        format!("I'll delegate this follow-up to agent '{}'", agent_id)
                    },
                    RoutingDecision::Decompose { subtasks } => {
                        format!("I'll break this follow-up into {} subtasks", subtasks.len())
                    },
                    RoutingDecision::Reject { reason } => {
                        format!("I'll reject this follow-up: {}", reason)
                    },
                };
                
                self.add_to_context(conversation_id, "assistant", &response_text)?;
                
                Ok(decision)
            },
            Err(e) => {
                warn!("LLM routing failed for follow-up on task '{}': {}, using local execution", task_id, e);
                Ok(RoutingDecision::Local {
                    tool_names: vec!["echo".to_string()],
                })
            }
        }
    }
    
    /// Creates a prompt for LLM follow-up routing
    fn create_follow_up_prompt(&self, follow_up_text: &str, tools: &[String], agents: &[(String, String)]) -> String {
        // Construct tool descriptions
        let mut tools_description = String::new();
        if !tools.is_empty() {
            tools_description.push_str("Available tools:\n");
            for tool in tools {
                let tool_description = match tool.as_str() {
                    "directory" => "Directory tool: Query and manage the agent directory",
                    "echo" => "Echo tool: Simple tool that returns the input text",
                    "http" => "HTTP tool: Make HTTP requests to external services",
                    "shell" => "Shell tool: Execute shell commands (careful with this one)",
                    _ => "Custom tool with unknown capabilities",
                };
                tools_description.push_str(&format!("- {}: {}\n", tool, tool_description));
            }
        } else {
            tools_description.push_str("No local tools are available.\n");
        }
        
        // Construct agent descriptions
        let mut agents_description = String::new();
        if !agents.is_empty() {
            agents_description.push_str("Available agents for delegation:\n");
            for (agent_id, agent_desc) in agents {
                agents_description.push_str(&format!("- {}\n", agent_desc));
            }
        } else {
            agents_description.push_str("No remote agents are available for delegation.\n");
        }
        
        // Determine if multi-turn is enabled
        let multi_turn_option = if self.config.enable_multi_turn {
            "3. MULTI_TURN: Continue the multi-turn conversation by asking for additional information\n"
        } else {
            ""
        };
        
        // Create follow-up prompt
        format!(
            "# Follow-up Message Routing Decision\n\n\
            You are processing a follow-up message in a multi-turn conversation. Decide how to handle it.\n\n\
            ## Follow-up Message\n{}\n\n\
            ## Routing Options\n\
            1. LOCAL: Handle the follow-up locally using one or more tools\n\
            {}\n\
            2. REMOTE: Delegate the follow-up to another agent\n\
            {}\n\
            {}\
            4. REJECT: Reject the follow-up if it's inappropriate or impossible to handle\n\n\
            ## Instructions\n\
            Analyze the follow-up message and decide how to handle it based on the available options.\n\
            - Choose LOCAL if the message can be processed with the available tools\n\
            - Choose REMOTE if another agent would be better suited to handle the follow-up\n\
            - Choose MULTI_TURN if you still need more information from the user to proceed\n\
            - Choose REJECT only if the follow-up is invalid, harmful, or impossible to handle\n\n\
            ## Response Format Requirements\n\
            Return ONLY a JSON object with no additional text, explanations, or decorations.\n\
            The JSON MUST follow the structure corresponding to your decision (as in the initial routing prompt).\n\
            DO NOT include any other text, markdown formatting, or explanations outside the JSON.",
            follow_up_text,
            tools_description,
            agents_description,
            multi_turn_option
        )
    }
    
    /// Use LLM to make a routing decision
    async fn llm_routing_decision(&self, prompt: &str) -> Result<RoutingDecision, AgentError> {
        // Get LLM client
        let llm_client = self.llm_client.as_ref().ok_or_else(|| {
            AgentError::ConfigError("LLM client not available".to_string())
        })?;
        
        // Define response structure for LOCAL decision
        #[derive(Deserialize)]
        struct LocalRouting {
            decision_type: String,
            reason: String,
            tool_names: Vec<String>,
        }
        
        // Define response structure for REMOTE decision
        #[derive(Deserialize)]
        struct RemoteRouting {
            decision_type: String,
            reason: String,
            agent_id: String,
        }
        
        // Define response structure for MULTI_TURN decision
        #[derive(Deserialize)]
        struct MultiTurnRouting {
            decision_type: String,
            reason: String,
            prompt: String,
        }
        
        // Define response structure for REJECT decision
        #[derive(Deserialize)]
        struct RejectRouting {
            decision_type: String,
            reason: String,
        }
        
        // Send prompt to LLM and get response
        let response = llm_client.complete(prompt).await.map_err(|e| {
            AgentError::RoutingError(format!("LLM routing request failed: {}", e))
        })?;
        
        // Parse the response based on decision type
        if response.contains("\"decision_type\"") {
            if response.contains("\"decision_type\": \"LOCAL\"") || response.contains("\"decision_type\":\"LOCAL\"") {
                // Parse LOCAL decision
                match serde_json::from_str::<LocalRouting>(&response) {
                    Ok(routing) => {
                        // Validate tool names
                        let available_tools = self.tool_executor.available_tools();
                        let mut valid_tools = Vec::new();
                        
                        for tool in &routing.tool_names {
                            if available_tools.contains(tool) {
                                valid_tools.push(tool.clone());
                            }
                        }
                        
                        // If no valid tools, default to echo
                        if valid_tools.is_empty() {
                            valid_tools.push("echo".to_string());
                        }
                        
                        Ok(RoutingDecision::Local { tool_names: valid_tools })
                    },
                    Err(e) => Err(AgentError::RoutingError(format!("Failed to parse LOCAL routing: {}", e)))
                }
            } else if response.contains("\"decision_type\": \"REMOTE\"") || response.contains("\"decision_type\":\"REMOTE\"") {
                // Parse REMOTE decision
                match serde_json::from_str::<RemoteRouting>(&response) {
                    Ok(routing) => {
                        // Validate agent ID
                        if self.agent_registry.exists(&routing.agent_id).await {
                            Ok(RoutingDecision::Remote { agent_id: routing.agent_id })
                        } else {
                            // Default to first available agent if any
                            let available_agents = self.get_available_agents().await;
                            if !available_agents.is_empty() {
                                Ok(RoutingDecision::Remote { agent_id: available_agents[0].0.clone() })
                            } else {
                                // Fallback to local execution if no agents available
                                Ok(RoutingDecision::Local { tool_names: vec!["echo".to_string()] })
                            }
                        }
                    },
                    Err(e) => Err(AgentError::RoutingError(format!("Failed to parse REMOTE routing: {}", e)))
                }
            } else if response.contains("\"decision_type\": \"MULTI_TURN\"") || response.contains("\"decision_type\":\"MULTI_TURN\"") {
                // Parse MULTI_TURN decision
                match serde_json::from_str::<MultiTurnRouting>(&response) {
                    Ok(routing) => {
                        // Create a decomposition with a single subtask (for the follow-up)
                        let subtasks = vec![format!("{}", routing.prompt)];
                        Ok(RoutingDecision::Decompose { subtasks })
                    },
                    Err(e) => Err(AgentError::RoutingError(format!("Failed to parse MULTI_TURN routing: {}", e)))
                }
            } else if response.contains("\"decision_type\": \"REJECT\"") || response.contains("\"decision_type\":\"REJECT\"") {
                // Parse REJECT decision
                match serde_json::from_str::<RejectRouting>(&response) {
                    Ok(routing) => Ok(RoutingDecision::Reject { reason: routing.reason }),
                    Err(e) => Err(AgentError::RoutingError(format!("Failed to parse REJECT routing: {}", e)))
                }
            } else {
                Err(AgentError::RoutingError(format!("Unknown decision type in LLM response: {}", response)))
            }
        } else {
            Err(AgentError::RoutingError(format!("Invalid LLM response format: {}", response)))
        }
    }
    
    /// Extract tool calls from a message
    fn extract_tool_calls(&self, message: &Message) -> Vec<(String, Value)> {
        let mut tool_calls = Vec::new();
        
        for part in &message.parts {
            if let Some((name, params)) = extract_tool_call(part) {
                tool_calls.push((name, params));
            }
        }
        
        tool_calls
    }
    
    /// Fallback routing when LLM is not available
    fn fallback_routing(&self) -> Result<RoutingDecision, AgentError> {
        debug!("Using fallback routing strategy");
        
        // 1. Check if any tools are available
        let tools: Vec<String> = self.tool_executor.available_tools().clone();
        if !tools.is_empty() {
            // If the directory tool is available, prefer it
            if tools.contains(&"directory".to_string()) {
                return Ok(RoutingDecision::Local { tool_names: vec!["directory".to_string()] });
            }
            // Otherwise use the first available tool
            return Ok(RoutingDecision::Local { tool_names: vec![tools[0].clone()] });
        }
        
        // 2. Check for available agents
        let agents = self.agent_registry.get_active_agents();
        let agent_ids: Vec<String> = agents.into_iter().map(|a| a.id).collect();
        
        if !agent_ids.is_empty() {
            return Ok(RoutingDecision::Remote { agent_id: agent_ids[0].clone() });
        }
        
        // 3. If no tools or agents, fall back to echo tool
        Ok(RoutingDecision::Local { tool_names: vec!["echo".to_string()] })
    }
    
    /// Get a list of available agents with descriptions
    async fn get_available_agents(&self) -> Vec<(String, String)> {
        let mut available_agents = Vec::new();
        let active_agents = self.agent_registry.get_active_agents();
        
        for agent in active_agents {
            // Extract agent capabilities for prompt
            let agent_desc = format!("Agent ID: {}\nName: {}\nDescription: {}\nCapabilities: {:?}\n",
                agent.id,
                agent.name,
                agent.description.unwrap_or_else(|| "None".to_string()),
                agent.capabilities
            );
            
            available_agents.push((agent.id, agent_desc));
        }
        
        available_agents
    }
    
    /// Get the conversation ID from task metadata
    fn get_conversation_id(&self, params: &TaskSendParams) -> Option<String> {
        params.metadata.as_ref().and_then(|metadata| {
            get_metadata_ext::<String>(metadata, LLM_ROUTING_NAMESPACE, MetadataKeys::CONVERSATION_ID)
        })
    }
    
    async fn execute_decompose(&self, params: &TaskSendParams) -> Result<bool, AgentError> {
        // This is a placeholder implementation
        // In a real implementation, this would use LLM to determine if task decomposition is appropriate
        Ok(false)
    }
    
    async fn execute_task_decomposition(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, AgentError> {
        // This is a placeholder implementation
        let subtask_id = format!("subtask-{}", Uuid::new_v4());
        
        let subtask = SubtaskDefinition {
            id: subtask_id,
            input_message: "Placeholder subtask - decomposition not fully implemented".to_string(),
            metadata: Some(serde_json::Map::new()),
        };
        
        Ok(vec![subtask])
    }
}

#[async_trait]
impl RoutingAgentTrait for RoutingAgent {
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError> {
        self.decide_routing(params).await
    }
    
    async fn process_follow_up(&self, task_id: &str, message: &Message) -> Result<RoutingDecision, AgentError> {
        // Try to get conversation ID from the task storage
        // In a real implementation, we would retrieve the task from the repository
        // and extract the conversation ID from its metadata
        let conversation_id = format!("conv-{}", task_id);
        
        // Process the follow-up message
        self.process_follow_up_internal(task_id, message, &conversation_id).await
    }
    
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, AgentError> {
        self.execute_decompose(params).await
    }
    
    async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, AgentError> {
        self.execute_task_decomposition(params).await
    }
}

/// Create a new routing agent with standard settings
pub fn create_routing_agent(
    agent_registry: Arc<AgentRegistry>,
    tool_executor: Arc<ToolExecutor>,
) -> Result<Arc<dyn RoutingAgentTrait>, AgentError> {
    // Try to get API key from environment
    let api_key = std::env::var("ANTHROPIC_API_KEY").ok();
    
    // Create default config
    let config = LlmRoutingConfig::default();
    
    // Create routing agent
    let agent = RoutingAgent::new(agent_registry, tool_executor, config, api_key)?;
    
    Ok(Arc::new(agent))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bidirectional_agent::task_router::RoutingDecision;
    use crate::bidirectional_agent::agent_registry::AgentRegistry;
    use crate::bidirectional_agent::tool_executor::ToolExecutor;
    use crate::bidirectional_agent::types::create_tool_call_part;
    use crate::bidirectional_agent::agent_directory::AgentDirectory;
    use crate::bidirectional_agent::config::DirectoryConfig;
    use serde_json::json;
    use tempfile::tempdir;
    
    // Helper to create a test message
    fn create_test_message(text: &str) -> Message {
        Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: text.to_string(),
                metadata: None,
            })],
            metadata: None,
        }
    }
    
    // Helper to create a test message with a tool call
    fn create_tool_call_message(tool_name: &str, params: Value) -> Message {
        let tool_call_part = create_tool_call_part(tool_name, params, None);
        
        Message {
            role: Role::User,
            parts: vec![Part::DataPart(tool_call_part)],
            metadata: None,
        }
    }
    
    // Helper to create test params
    fn create_test_params(id: &str, message: Message) -> TaskSendParams {
        TaskSendParams {
            id: id.to_string(),
            message,
            history_length: None,
            metadata: None,
            push_notification: None,
            session_id: None,
        }
    }
    
    // Setup test components
    async fn setup_test_components() -> (Arc<AgentRegistry>, Arc<ToolExecutor>) {
        // Create a temporary directory for the test
        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let db_path = temp_dir.path().join("test_directory.db");
        
        // Create directory config
        let config = DirectoryConfig {
            db_path: db_path.to_string_lossy().to_string(),
            ..Default::default()
        };
        
        // Create agent directory
        let directory = Arc::new(AgentDirectory::new(&config).await.expect("Failed to create agent directory"));
        
        // Create agent registry and tool executor
        let registry = Arc::new(AgentRegistry::new(directory.clone()));
        let executor = Arc::new(ToolExecutor::new(directory));
        
        (registry, executor)
    }
    
    #[tokio::test]
    async fn test_extract_tool_calls() {
        // Create test components
        let (registry, executor) = setup_test_components().await;
        
        // Create routing agent without LLM
        let config = LlmRoutingConfig::default();
        let agent = RoutingAgent::new(registry, executor, config, None).unwrap();
        
        // Create a message with a tool call
        let params = json!({
            "url": "https://example.com",
            "method": "GET"
        });
        
        let message = create_tool_call_message("http", params.clone());
        
        // Extract tool calls
        let tool_calls = agent.extract_tool_calls(&message);
        
        // Verify result
        assert_eq!(tool_calls.len(), 1, "Should extract one tool call");
        assert_eq!(tool_calls[0].0, "http", "Tool name should be 'http'");
        assert_eq!(tool_calls[0].1, params, "Tool params should match");
    }
    
    #[tokio::test]
    async fn test_fallback_routing() {
        // Create test components
        let (registry, executor) = setup_test_components().await;
        
        // Create routing agent without LLM
        let config = LlmRoutingConfig::default();
        let agent = RoutingAgent::new(registry, executor, config, None).unwrap();
        
        // Test fallback routing
        let result = agent.fallback_routing().unwrap();
        
        // Verify result (should be LOCAL with echo tool)
        match result {
            RoutingDecision::Local { tool_names } => {
                assert_eq!(tool_names.len(), 1, "Should have one tool");
                assert_eq!(tool_names[0], "echo", "Tool should be 'echo'");
            },
            _ => panic!("Expected LOCAL routing decision"),
        }
    }
    
    #[tokio::test]
    async fn test_decide_with_tool_call() {
        // Create test components
        let (registry, executor) = setup_test_components().await;
        
        // Create routing agent without LLM
        let config = LlmRoutingConfig::default();
        let agent = RoutingAgent::new(registry, executor, config, None).unwrap();
        
        // Create a message with a tool call
        let params = json!({
            "query": "list active agents"
        });
        
        let message = create_tool_call_message("directory", params);
        let task_params = create_test_params("test-task", message);
        
        // Test decide method
        let result = agent.decide(&task_params).await.unwrap();
        
        // Verify result (should be LOCAL with directory tool)
        match result {
            RoutingDecision::Local { tool_names } => {
                assert_eq!(tool_names.len(), 1, "Should have one tool");
                assert_eq!(tool_names[0], "directory", "Tool should be 'directory'");
            },
            _ => panic!("Expected LOCAL routing decision"),
        }
    }
    
    #[tokio::test]
    async fn test_decide_with_text() {
        // Create test components
        let (registry, executor) = setup_test_components().await;
        
        // Create routing agent without LLM
        let config = LlmRoutingConfig::default();
        let agent = RoutingAgent::new(registry, executor, config, None).unwrap();
        
        // Create a message with text
        let message = create_test_message("Test message");
        let task_params = create_test_params("test-task", message);
        
        // Test decide method
        let result = agent.decide(&task_params).await.unwrap();
        
        // Verify result (should be fallback routing)
        match result {
            RoutingDecision::Local { tool_names } => {
                assert_eq!(tool_names.len(), 1, "Should have one tool");
                assert_eq!(tool_names[0], "echo", "Tool should be 'echo'");
            },
            _ => panic!("Expected LOCAL routing decision"),
        }
    }
    
    #[tokio::test]
    async fn test_process_follow_up() {
        // Create test components
        let (registry, executor) = setup_test_components().await;
        
        // Create routing agent without LLM
        let config = LlmRoutingConfig::default();
        let agent = RoutingAgent::new(registry, executor, config, None).unwrap();
        
        // Create a message with a tool call
        let params = json!({
            "query": "get agent details"
        });
        
        let message = create_tool_call_message("directory", params);
        
        // Test process_follow_up method
        let result = agent.process_follow_up("test-task", &message).await.unwrap();
        
        // Verify result (should be LOCAL with directory tool)
        match result {
            RoutingDecision::Local { tool_names } => {
                assert_eq!(tool_names.len(), 1, "Should have one tool");
                assert_eq!(tool_names[0], "directory", "Tool should be 'directory'");
            },
            _ => panic!("Expected LOCAL routing decision"),
        }
    }
}