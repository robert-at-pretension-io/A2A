//! Refactored LLM-powered task routing implementation.
//!
//! This module provides a specialized TaskRouter that uses LLMs for
//! making sophisticated routing decisions, using the new LlmClient trait
//! and template system.

#[cfg(feature = "bidir-local-exec")]
use crate::bidirectional_agent::llm_routing::RoutingAgentTrait;
use crate::bidirectional_agent::agent_directory::AgentDirectory;

use crate::bidirectional_agent::{
    agent_registry::AgentRegistry,
    task_router::{RoutingDecision, SubtaskDefinition, LlmTaskRouterTrait},
    tool_executor::ToolExecutor,
    error::AgentError,
    llm_core::{LlmClient, TemplateManager},
};
use crate::types::{TaskSendParams, Message};
use std::sync::Arc;
use std::collections::HashMap;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for LLM-powered routing
#[derive(Debug, Clone)]
pub struct LlmRoutingConfig {
    /// Model to use for routing (e.g., "claude-3-haiku-20240307")
    pub model: String,
    
    /// Temperature for routing decisions (0.0 - 1.0)
    pub temperature: f32,
    
    /// Max tokens to generate for routing decisions
    pub max_tokens: u32,
}

impl Default for LlmRoutingConfig {
    fn default() -> Self {
        Self {
            model: "claude-3-haiku-20240307".to_string(),
            temperature: 0.1,
            max_tokens: 2048,
        }
    }
}

/// Response structure for routing decisions
#[derive(Deserialize)]
struct RoutingResponse {
    decision_type: String,
    reason: String,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    agent_id: String,
}

/// Response structure for decomposition check
#[derive(Deserialize)]
struct DecompositionResponse {
    should_decompose: bool,
    reasoning: String,
}

/// Response structure for task decomposition
#[derive(Deserialize)]
struct DecompositionResult {
    subtasks: Vec<Subtask>,
}

#[derive(Deserialize)]
struct Subtask {
    description: String,
}

/// Enhanced TaskRouter that uses LLM-based decision making
pub struct RefactoredLlmTaskRouter {
    /// Agent registry for delegation
    agent_registry: Arc<AgentRegistry>,
    
    /// Tool executor for local execution
    tool_executor: Arc<ToolExecutor>,
    
    /// LLM client for making routing decisions
    llm_client: Arc<dyn LlmClient>,
    
    /// Template manager for prompt templates
    template_manager: TemplateManager,
    
    /// Configuration for LLM routing
    config: LlmRoutingConfig,
}

impl RefactoredLlmTaskRouter {
    /// Creates a new LLM-powered task router.
    pub fn new(
        agent_registry: Arc<AgentRegistry>,
        tool_executor: Arc<ToolExecutor>,
        llm_client: Arc<dyn LlmClient>,
        config: Option<LlmRoutingConfig>,
    ) -> Self {
        Self {
            agent_registry,
            tool_executor,
            llm_client,
            template_manager: TemplateManager::with_default_dir(),
            config: config.unwrap_or_default(),
        }
    }
    
    /// Makes a routing decision based on the task.
    pub async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError> {
        println!("🧠 Refactored LlmTaskRouter analyzing task '{}'...", params.id);
        
        // First check for explicit routing hints in metadata
        if let Some(metadata) = &params.metadata {
            if let Some(route_hint) = metadata.get("_route_to").and_then(|v| v.as_str()) {
                println!("  Found explicit routing hint: {}", route_hint);
                return self.handle_routing_hint(route_hint);
            }
        }
        
        // No explicit hint, use LLM for intelligent routing
        self.llm_routing(params).await
    }
    
    /// Handles explicit routing hints from metadata.
    fn handle_routing_hint(&self, hint: &str) -> Result<RoutingDecision, AgentError> {
        match hint {
            "local" => {
                println!("  Routing hint: Execute locally.");
                Ok(RoutingDecision::Local { tool_names: vec!["echo".to_string()] })
            },
            "reject" => {
                println!("  Routing hint: Reject task.");
                Ok(RoutingDecision::Reject { 
                    reason: "Task explicitly rejected by routing hint".to_string() 
                })
            },
            _ => {
                // Check if it's an agent ID
                if self.agent_registry.get(hint).is_some() {
                    println!("  Routing hint: Delegate to '{}'.", hint);
                    Ok(RoutingDecision::Remote { agent_id: hint.to_string() })
                } else {
                    println!("  ⚠️ Unknown routing hint: '{}'", hint);
                    // Fall through to LLM routing
                    Err(AgentError::RoutingError(
                        format!("Unknown routing hint: {}", hint)
                    ))
                }
            }
        }
    }
    
    /// Checks if a task should be decomposed into subtasks.
    #[cfg(feature = "bidir-delegate")]
    pub async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, AgentError> {
        println!("🔍 Checking if task '{}' should be decomposed", params.id);
        
        // Extract task text
        let task_text = self.extract_text_from_params(params);
        if task_text.is_empty() {
            println!("⚠️ No text found in task params, assuming no decomposition needed");
            return Ok(false);
        }
        
        // Use LLM to determine if decomposition is needed
        match self.llm_should_decompose(&task_text).await {
            Ok(should_decompose) => {
                println!("✅ LLM decision on decomposition: {}", should_decompose);
                Ok(should_decompose)
            },
            Err(e) => {
                println!("⚠️ LLM decomposition check failed: {}, using heuristic", e);
                self.heuristic_should_decompose(&task_text)
            }
        }
    }
    
    /// Decomposes a complex task into subtasks.
    #[cfg(feature = "bidir-delegate")]
    pub async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, AgentError> {
        println!("🧩 Decomposing task '{}' into subtasks", params.id);
        
        // Extract task text
        let task_text = self.extract_text_from_params(params);
        if task_text.is_empty() {
            println!("⚠️ No text found in task params, returning default subtasks");
            return Ok(self.default_subtasks());
        }
        
        // Try to use LLM for task decomposition
        match self.llm_decompose_task(&task_text).await {
            Ok(subtasks) => {
                println!("✅ LLM decomposed task into {} subtasks", subtasks.len());
                Ok(subtasks)
            },
            Err(e) => {
                println!("⚠️ LLM decomposition failed: {}, using default subtasks", e);
                Ok(self.default_subtasks())
            }
        }
    }
    
    /// Simple heuristic to determine if a task should be decomposed
    #[cfg(feature = "bidir-delegate")]
    fn heuristic_should_decompose(&self, task_text: &str) -> Result<bool, AgentError> {
        // Count sentences as a simple heuristic for complexity
        let sentence_count = task_text.split(['.', '!', '?'])
            .filter(|s| !s.trim().is_empty())
            .count();
            
        // Count commas for subtasks hints
        let comma_count = task_text.matches(',').count();
        
        // Look for keywords that suggest multiple steps
        let has_step_keywords = ["step", "first", "then", "finally", "and", "or", "next"]
            .iter()
            .any(|&keyword| task_text.to_lowercase().contains(keyword));
            
        // Decision based on heuristics
        let should_decompose = sentence_count > 2 || comma_count > 3 || has_step_keywords;
        
        println!("📊 Heuristic analysis: sentences={}, commas={}, step_keywords={}, decompose={}",
            sentence_count, comma_count, has_step_keywords, should_decompose);
            
        Ok(should_decompose)
    }
    
    /// Generate default subtasks when LLM is not available
    #[cfg(feature = "bidir-delegate")]
    fn default_subtasks(&self) -> Vec<SubtaskDefinition> {
        vec![
            SubtaskDefinition {
                id: format!("subtask-{}", Uuid::new_v4()),
                input_message: "Research and gather information related to the task".to_string(),
                metadata: Some(serde_json::Map::new()),
            },
            SubtaskDefinition {
                id: format!("subtask-{}", Uuid::new_v4()),
                input_message: "Process information and provide results".to_string(),
                metadata: Some(serde_json::Map::new()),
            }
        ]
    }
    
    /// Use LLM to make a routing decision
    async fn llm_routing(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError> {
        // Extract task text
        let task_text = self.extract_text_from_params(params);
        if task_text.is_empty() {
            println!("⚠️ No text found in task params, using fallback routing");
            return self.fallback_routing();
        }
        
        // Get available tools and agents
        let tools: Vec<String> = self.tool_executor.tools.keys().cloned().collect();
        let mut available_agents = Vec::new();
        for agent_pair in self.agent_registry.agents.iter() {
            let agent_id = agent_pair.key();
            let agent_info = agent_pair.value();
            
            // Extract agent capabilities for prompt
            let agent_desc = format!("Agent ID: {}\nName: {}\nDescription: {}\nCapabilities: {:?}\n",
                agent_id,
                agent_info.card.name,
                agent_info.card.description.as_deref().unwrap_or("None"),
                agent_info.card.capabilities
            );
            
            available_agents.push((agent_id.to_string(), agent_desc));
        }
        
        // Construct tool descriptions
        let mut tools_description = String::new();
        if !tools.is_empty() {
            tools_description.push_str("Available tools:\n");
            for tool in &tools {
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
        if !available_agents.is_empty() {
            agents_description.push_str("Available agents for delegation:\n");
            for (agent_id, agent_desc) in &available_agents {
                agents_description.push_str(&format!("- {}\n", agent_desc));
            }
        } else {
            agents_description.push_str("No remote agents are available for delegation.\n");
        }
        
        // Create variables for template
        let mut variables = HashMap::new();
        variables.insert("task_description".to_string(), task_text);
        variables.insert("available_tools".to_string(), tools_description);
        variables.insert("available_agents".to_string(), agents_description);
        
        // Render template
        let prompt = match self.template_manager.render("routing_decision", &variables) {
            Ok(prompt) => prompt,
            Err(e) => {
                println!("⚠️ Failed to render routing template: {}", e);
                return self.fallback_routing();
            }
        };
        
        // Call LLM with the prompt
        let response_result = self.llm_client.complete_json::<RoutingResponse>(&prompt).await;
        
        // Parse response
        let response = match response_result {
            Ok(r) => r,
            Err(e) => {
                println!("⚠️ LLM routing failed: {}, using fallback routing", e);
                return self.fallback_routing();
            }
        };
        
        // Log the reason for the decision
        println!("💭 Routing reason: {}", response.reason);
        
        // Convert to routing decision
        match response.decision_type.to_uppercase().as_str() {
            "LOCAL" => {
                if response.tools.is_empty() {
                    // If no tools specified but we need local execution, default to directory tool if available
                    if tools.contains(&"directory".to_string()) {
                        println!("⚠️ LLM returned LOCAL decision with no tools, defaulting to directory tool");
                        return Ok(RoutingDecision::Local { tool_names: vec!["directory".to_string()] });
                    }
                    // Otherwise use the first available tool
                    if !tools.is_empty() {
                        println!("⚠️ LLM returned LOCAL decision with no tools, defaulting to first available tool");
                        return Ok(RoutingDecision::Local { tool_names: vec![tools[0].clone()] });
                    }
                    return Err(AgentError::RoutingError("LLM returned LOCAL decision but no tools available".to_string()));
                }
                
                // Validate that all tools exist
                let mut valid_tools = Vec::new();
                for tool in &response.tools {
                    if tools.contains(tool) {
                        valid_tools.push(tool.clone());
                    } else {
                        println!("⚠️ LLM suggested non-existent tool: {}, ignoring it", tool);
                    }
                }
                
                if valid_tools.is_empty() {
                    // If no valid tools were found, default to directory tool if available
                    if tools.contains(&"directory".to_string()) {
                        println!("⚠️ No valid tools suggested by LLM, defaulting to directory tool");
                        valid_tools.push("directory".to_string());
                    } else if !tools.is_empty() {
                        println!("⚠️ No valid tools suggested by LLM, defaulting to first available tool");
                        valid_tools.push(tools[0].clone());
                    } else {
                        return Err(AgentError::RoutingError("No valid tools available".to_string()));
                    }
                }
                
                Ok(RoutingDecision::Local { tool_names: valid_tools })
            },
            "REMOTE" => {
                if response.agent_id.is_empty() {
                    // If no agent specified but we have agents, default to first agent
                    if !available_agents.is_empty() {
                        println!("⚠️ LLM returned REMOTE decision with no agent_id, defaulting to first available agent");
                        return Ok(RoutingDecision::Remote { agent_id: available_agents[0].0.clone() });
                    }
                    return Err(AgentError::RoutingError("LLM returned REMOTE decision but no agents available".to_string()));
                }
                
                // Validate that the agent exists
                let agent_exists = available_agents.iter().any(|(id, _)| id == &response.agent_id);
                if !agent_exists {
                    // If the specified agent doesn't exist, default to first agent if available
                    if !available_agents.is_empty() {
                        println!("⚠️ LLM suggested non-existent agent: {}, defaulting to first available agent", response.agent_id);
                        return Ok(RoutingDecision::Remote { agent_id: available_agents[0].0.clone() });
                    }
                    return Err(AgentError::RoutingError("No valid agents available".to_string()));
                }
                
                Ok(RoutingDecision::Remote { agent_id: response.agent_id })
            },
            "REJECT" => {
                // Handle explicit rejection
                Ok(RoutingDecision::Reject { reason: response.reason })
            },
            invalid_type => {
                // If invalid decision type, default to LOCAL with directory tool if available
                println!("⚠️ Invalid decision_type from LLM: {}, attempting recovery", invalid_type);
                
                // Try to recover - if tools available, use local routing
                if !tools.is_empty() {
                    if tools.contains(&"directory".to_string()) {
                        println!("  ↳ Recovering with LOCAL routing to directory tool");
                        return Ok(RoutingDecision::Local { tool_names: vec!["directory".to_string()] });
                    }
                    println!("  ↳ Recovering with LOCAL routing to first available tool");
                    return Ok(RoutingDecision::Local { tool_names: vec![tools[0].clone()] });
                }
                
                // If no tools but agents available, use remote routing
                if !available_agents.is_empty() {
                    println!("  ↳ Recovering with REMOTE routing to first available agent");
                    return Ok(RoutingDecision::Remote { agent_id: available_agents[0].0.clone() });
                }
                
                // No recovery possible
                Err(AgentError::RoutingError(format!("Invalid decision_type from LLM: {} and no recovery options", invalid_type)))
            }
        }
    }
    
    /// Fallback routing when LLM is not available or fails
    fn fallback_routing(&self) -> Result<RoutingDecision, AgentError> {
        // 1. Check if any tools are available
        let tools: Vec<String> = self.tool_executor.tools.keys().cloned().collect();
        if !tools.is_empty() {
            // If the directory tool is available, prefer it
            if tools.contains(&"directory".to_string()) {
                return Ok(RoutingDecision::Local { tool_names: vec!["directory".to_string()] });
            }
            // Otherwise use the first available tool
            return Ok(RoutingDecision::Local { tool_names: vec![tools[0].clone()] });
        }
        
        // 2. Check if any agents are available for delegation
        let mut available_agents = Vec::new();
        for agent_pair in self.agent_registry.agents.iter() {
            let agent_id = agent_pair.key();
            available_agents.push(agent_id.to_string());
        }
        
        if !available_agents.is_empty() {
            return Ok(RoutingDecision::Remote { agent_id: available_agents[0].clone() });
        }
        
        // 3. Fallback to echo tool as last resort
        Ok(RoutingDecision::Local { tool_names: vec!["echo".to_string()] })
    }
    
    /// Extract text from task parameters
    fn extract_text_from_params(&self, params: &TaskSendParams) -> String {
        let message = &params.message;
        let mut text = String::new();
        
        for part in &message.parts {
            if let crate::types::Part::TextPart(text_part) = part {
                if !text.is_empty() {
                    text.push_str("\n\n");
                }
                text.push_str(&text_part.text);
            }
        }
        
        text
    }
    
    /// Use LLM to determine if a task should be decomposed
    #[cfg(feature = "bidir-delegate")]
    async fn llm_should_decompose(&self, task_text: &str) -> Result<bool, AgentError> {
        // Create variables for template
        let mut variables = HashMap::new();
        variables.insert("task_description".to_string(), task_text.to_string());
        
        // Create custom prompt since we don't have a template yet
        let prompt = format!(
            "# Task Decomposition Analysis\n\n\
            You are an AI assistant that helps determine if tasks should be broken down into subtasks.\n\n\
            ## Task Description\n{}\n\n\
            ## Instructions\n\
            Analyze the complexity of this task:\n\
            - Does it involve multiple distinct steps or actions?\n\
            - Would it benefit from being broken down into smaller subtasks?\n\
            - Is it a simple, single-step task that should be handled as a whole?\n\n\
            ## Response Format Requirements\n\
            Return ONLY a JSON object with no additional text, explanations, or decorations.\n\
            The JSON MUST follow this exact structure:\n\
            \n\
            {{\n  \
                \"should_decompose\": true or false,\n  \
                \"reasoning\": \"Brief explanation of your decision\"\n\
            }}\n\
            \n\n\
            DO NOT include any other text, markdown formatting, prefixes, or notes outside the JSON.\n\
            Your entire response must be valid JSON that can be parsed directly.",
            task_text
        );
        
        // Call LLM
        let response_result = self.llm_client.complete_json::<DecompositionResponse>(&prompt).await;
        
        // Parse response
        match response_result {
            Ok(response) => {
                // Log the reasoning
                println!("💭 LLM decomposition reasoning: {}", response.reasoning);
                Ok(response.should_decompose)
            },
            Err(e) => {
                println!("⚠️ LLM decomposition failed: {}. Trying with simplified prompt...", e);
                
                // Try with simplified prompt
                let simplified_prompt = format!(
                    "Analyze this task:\n\n{}\n\nWould this task benefit from being broken down into subtasks? Reply with ONLY this JSON format:\n{{\"should_decompose\": true or false, \"reasoning\": \"your reasoning\"}}\n\nNOTHING ELSE.",
                    task_text
                );
                
                match self.llm_client.complete_json::<DecompositionResponse>(&simplified_prompt).await {
                    Ok(response) => {
                        println!("💭 LLM decomposition reasoning (retry): {}", response.reasoning);
                        Ok(response.should_decompose)
                    },
                    Err(e2) => {
                        println!("⚠️ Second LLM decomposition attempt also failed: {}", e2);
                        Err(AgentError::RoutingError(format!("LLM decomposition failed: {} | {}", e, e2)))
                    }
                }
            }
        }
    }
    
    /// Use LLM to decompose a task into subtasks
    #[cfg(feature = "bidir-delegate")]
    async fn llm_decompose_task(&self, task_text: &str) -> Result<Vec<SubtaskDefinition>, AgentError> {
        // Create variables for template
        let mut variables = HashMap::new();
        variables.insert("task_description".to_string(), task_text.to_string());
        
        // Render template
        let prompt = match self.template_manager.render("decomposition", &variables) {
            Ok(prompt) => prompt,
            Err(e) => {
                println!("⚠️ Failed to render decomposition template: {}", e);
                
                // Fallback to hardcoded prompt
                format!(
                    "# Task Decomposition\n\n\
                    You are an AI assistant that breaks down complex tasks into manageable subtasks.\n\n\
                    ## Task Description\n{}\n\n\
                    ## Instructions\n\
                    Break down this task into 2-5 clear, logical subtasks that collectively achieve the overall goal.\n\
                    For each subtask, provide a clear, specific description of what needs to be done.\n\n\
                    ## Response Format Requirements\n\
                    {{\"subtasks\": [{{\n\"description\": \"subtask 1 details\"}}, \
                    {{\"description\": \"subtask 2 details\"}}]}}",
                    task_text
                )
            }
        };
        
        // Call LLM
        let response_result = self.llm_client.complete_json::<DecompositionResult>(&prompt).await;
        
        // Parse response
        match response_result {
            Ok(response) => {
                // Check if we received valid subtasks
                if response.subtasks.is_empty() {
                    println!("⚠️ LLM returned empty subtasks list, using default subtasks");
                    return Ok(self.default_subtasks());
                }
                
                // Convert to SubtaskDefinition format
                let subtasks = response.subtasks.into_iter()
                    .map(|subtask| SubtaskDefinition {
                        id: format!("subtask-{}", Uuid::new_v4()),
                        input_message: subtask.description,
                        metadata: Some(serde_json::Map::new()),
                    })
                    .collect::<Vec<_>>();
                
                println!("✅ Successfully decomposed task into {} subtasks", subtasks.len());
                Ok(subtasks)
            },
            Err(e) => {
                println!("⚠️ LLM decomposition failed: {}", e);
                Err(AgentError::RoutingError(format!("LLM decomposition failed: {}", e)))
            }
        }
    }
}

// Implement the LlmTaskRouterTrait for our refactored router
#[async_trait::async_trait]
impl LlmTaskRouterTrait for RefactoredLlmTaskRouter {
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError> {
        self.decide(params).await
    }
    
    #[cfg(feature = "bidir-delegate")]
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, AgentError> {
        self.should_decompose(params).await
    }
    
    #[cfg(feature = "bidir-delegate")]
    async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, AgentError> {
        self.decompose_task(params).await
    }
}

/// Factory for creating refactored LLM task routers
pub struct RefactoredLlmTaskRouterFactory;

impl RefactoredLlmTaskRouterFactory {
    /// Creates a new LLM task router with the provided LLM client
    pub fn create_with_client(
        agent_registry: Arc<AgentRegistry>,
        tool_executor: Arc<ToolExecutor>,
        llm_client: Arc<dyn LlmClient>,
        config: Option<LlmRoutingConfig>,
    ) -> Arc<RefactoredLlmTaskRouter> {
        Arc::new(RefactoredLlmTaskRouter::new(
            agent_registry,
            tool_executor,
            llm_client,
            config,
        ))
    }
    
    /// Creates a new LLM task router with a default Claude client
    pub fn create_with_claude(
        agent_registry: Arc<AgentRegistry>,
        tool_executor: Arc<ToolExecutor>,
        api_key: String,
        config: Option<LlmRoutingConfig>,
    ) -> Result<Arc<RefactoredLlmTaskRouter>, AgentError> {
        use crate::bidirectional_agent::llm_core::{ClaudeClient, LlmConfig};
        
        // Create Claude client
        let routing_config = config.clone().unwrap_or_default();
        let llm_config = LlmConfig {
            api_key,
            model: routing_config.model.clone(),
            max_tokens: routing_config.max_tokens,
            temperature: routing_config.temperature,
            ..LlmConfig::default()
        };
        
        let claude_client = ClaudeClient::new(llm_config)
            .map_err(|e| AgentError::ConfigError(format!("Failed to create Claude client: {}", e)))?;
        
        // Create router with Claude client
        Ok(Arc::new(RefactoredLlmTaskRouter::new(
            agent_registry,
            tool_executor,
            Arc::new(claude_client),
            config,
        )))
    }
}

// Provide a helper function to integrate with existing code
pub fn create_refactored_llm_task_router(
    agent_registry: Arc<AgentRegistry>,
    tool_executor: Arc<ToolExecutor>,
) -> Result<Arc<dyn LlmTaskRouterTrait>, AgentError> {
    // Try to get API key from environment
    match std::env::var("ANTHROPIC_API_KEY") {
        Ok(api_key) => {
            // Create with Claude client
            RefactoredLlmTaskRouterFactory::create_with_claude(
                agent_registry,
                tool_executor,
                api_key,
                None,
            )
        },
        Err(_) => {
            // No API key, use mock client for testing
            use crate::bidirectional_agent::llm_core::MockLlmClient;
            
            println!("⚠️ ANTHROPIC_API_KEY not set, using mock LLM client");
            
            // Create a mock client with some simple responses
            let mock_client = Arc::new(MockLlmClient::with_default_response(
                r#"{"decision_type": "LOCAL", "reason": "Using mock client", "tools": ["echo"]}"#.to_string()
            ));
            
            Ok(RefactoredLlmTaskRouterFactory::create_with_client(
                agent_registry,
                tool_executor,
                mock_client,
                None,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bidirectional_agent::config::DirectoryConfig;
    use crate::bidirectional_agent::agent_directory::AgentDirectory;
    use crate::bidirectional_agent::llm_core::MockLlmClient;
    use crate::types::{Message, Role, Part, TextPart};
    use std::collections::HashMap;
    use serde_json::json;
    
    // Helper to create basic TaskSendParams for testing
    fn create_test_params(id: &str, text: &str, metadata: Option<serde_json::Map<String, serde_json::Value>>) -> TaskSendParams {
        TaskSendParams {
            id: id.to_string(),
            message: Message {
                role: Role::User,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: text.to_string(),
                    metadata: None,
                })],
                metadata: None,
            },
            history_length: None,
            metadata,
            push_notification: None,
            session_id: None,
        }
    }
    
    async fn create_test_directory() -> Arc<AgentDirectory> {
        Arc::new(AgentDirectory::new(&DirectoryConfig::default()).await.expect("Failed to create test directory"))
    }
    
    #[tokio::test]
    async fn test_refactored_router_with_mock() {
        // Create directory if bidir-core is enabled
        #[cfg(feature = "bidir-core")]
        let directory = create_test_directory().await;
        
        // Create registry with or without directory based on feature
        let registry = Arc::new(AgentRegistry::new(
            #[cfg(feature = "bidir-core")]
            directory.clone()
        ));
        
        // Create executor with or without directory based on feature
        let tool_executor = Arc::new(ToolExecutor::new(
            #[cfg(feature = "bidir-core")]
            directory.clone()
        ));
        
        // Create mock LLM client with predefined responses
        let mock_client = Arc::new(MockLlmClient::new(vec![
            // For local execution
            (
                "List all files".to_string(),
                r#"{"decision_type": "LOCAL", "reason": "File listing is a local operation", "tools": ["directory"]}"#.to_string()
            ),
            // For remote delegation
            (
                "Complex task".to_string(),
                r#"{"decision_type": "REMOTE", "reason": "This task requires specialized capabilities", "agent_id": "search_agent"}"#.to_string()
            ),
            // For rejection
            (
                "Reject".to_string(),
                r#"{"decision_type": "REJECT", "reason": "This task violates guidelines"}"#.to_string()
            ),
        ]));
        
        // Create router with mock client
        let router = RefactoredLlmTaskRouter::new(
            registry.clone(),
            tool_executor.clone(),
            mock_client,
            None,
        );
        
        // Test local routing
        let local_params = create_test_params("task1", "List all files in the current directory", None);
        let result = router.decide(&local_params).await;
        assert!(result.is_ok());
        match result.unwrap() {
            RoutingDecision::Local { tool_names } => {
                assert!(tool_names.contains(&"directory".to_string()));
            },
            _ => panic!("Expected Local routing decision"),
        }
        
        // Test rejection
        let reject_params = create_test_params("task2", "Reject this task please", None);
        let result = router.decide(&reject_params).await;
        assert!(result.is_ok());
        match result.unwrap() {
            RoutingDecision::Reject { reason } => {
                assert!(reason.contains("violates guidelines"));
            },
            _ => panic!("Expected Reject routing decision"),
        }
        
        // Test routing hint
        let mut metadata = serde_json::Map::new();
        metadata.insert("_route_to".to_string(), json!("local"));
        let hint_params = create_test_params("task3", "Route with hint", Some(metadata));
        let result = router.decide(&hint_params).await;
        assert!(result.is_ok());
        match result.unwrap() {
            RoutingDecision::Local { tool_names } => {
                assert_eq!(tool_names, vec!["echo".to_string()]);
            },
            _ => panic!("Expected Local routing decision"),
        }
    }
}