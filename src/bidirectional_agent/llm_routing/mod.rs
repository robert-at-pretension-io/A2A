//! LLM-powered routing and task decomposition logic.

#![cfg(feature = "bidir-local-exec")]

pub mod claude_client;
pub use self::claude_client::{LlmClient, LlmClientConfig};

use crate::bidirectional_agent::{
    agent_registry::AgentRegistry,
    task_router::{RoutingDecision, SubtaskDefinition},
    tool_executor::ToolExecutor,
    error::AgentError,
};
use crate::types::{TaskSendParams, Message, Role, Part, TextPart};
use dashmap::DashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use async_trait::async_trait;
use uuid::Uuid;

/// Configuration for LLM-powered routing.
#[derive(Debug, Clone)]
pub struct LlmRoutingConfig {
    /// Max tokens to generate for routing decisions
    pub max_tokens: usize,
    
    /// Temperature for routing decisions (0.0 - 1.0)
    pub temperature: f32,
    
    /// Model to use for routing (e.g., "gpt-4-turbo")
    pub model: String,
    
    /// Prompt template for routing decisions
    pub routing_prompt_template: String,
    
    /// Prompt template for decomposition decisions
    pub decomposition_prompt_template: String,
}

#[derive(Debug, Clone)]
pub struct DecomposedSubtask {
    pub id: String,
    pub description: String,
    pub input_message: crate::types::Message,
    pub tools_needed: Vec<String>,
}

pub struct RoutingAgent {
    agent_registry: Arc<AgentRegistry>,
    tool_executor: Arc<ToolExecutor>,
    llm_config: Option<LlmRoutingConfig>,
}

#[async_trait]
impl RoutingAgentTrait for RoutingAgent {
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

impl RoutingAgent {
    /// Creates a new routing agent.
    pub fn new(
        agent_registry: Arc<AgentRegistry>,
        tool_executor: Arc<ToolExecutor>,
        llm_config: Option<LlmRoutingConfig>,
    ) -> Self {
        Self {
            agent_registry,
            tool_executor,
            llm_config,
        }
    }
    
    /// Makes a routing decision based on the task.
    pub async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError> {
        println!("🧠 Making routing decision for task '{}'", params.id);
        
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
        
        // If LLM config is not provided, use fallback routing
        if self.llm_config.is_none() {
            println!("⚠️ No LLM config provided, using fallback routing");
            return self.fallback_routing();
        }
        
        // Try to use LLM for routing
        let routing_result = self.llm_routing(
            &task_text, 
            &tools, 
            &available_agents
        ).await;
        
        match routing_result {
            Ok(decision) => {
                println!("✅ LLM routing decision: {:?}", decision);
                Ok(decision)
            },
            Err(e) => {
                println!("⚠️ LLM routing failed: {}, using fallback routing", e);
                self.fallback_routing()
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
        
        // If LLM config is not provided, use simple heuristic
        if self.llm_config.is_none() {
            println!("⚠️ No LLM config provided, using heuristic decomposition");
            return self.heuristic_should_decompose(&task_text);
        }
        
        // Try to use LLM to determine if decomposition is needed
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
        
        // If LLM config is not provided, return default subtasks
        if self.llm_config.is_none() {
            println!("⚠️ No LLM config provided, returning default subtasks");
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
    
    /// Use LLM to determine if a task should be decomposed, with retry logic
    #[cfg(feature = "bidir-delegate")]
    async fn llm_should_decompose(&self, task_text: &str) -> Result<bool, AgentError> {
        // Get LLM config
        let llm_config = self.llm_config.as_ref().ok_or_else(|| {
            AgentError::ConfigError("LLM configuration is not provided".to_string())
        })?;
        
        // Initialize LLM client
        let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
            AgentError::ConfigError("ANTHROPIC_API_KEY environment variable is not set".to_string())
        })?;
        
        let llm_client_config = LlmClientConfig {
            api_key,
            model: llm_config.model.clone(),
            max_tokens: 100, // Small token count for this simple decision
            temperature: 0.1, // Low temperature for deterministic results
            ..Default::default()
        };
        
        let llm_client = LlmClient::new(llm_client_config).map_err(|e| {
            AgentError::Other(format!("Failed to initialize LLM client: {}", e))
        })?;
        
        // Create prompt with improved formatting instructions
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
        
        // Define response structure
        #[derive(Deserialize)]
        struct DecompositionResponse {
            should_decompose: bool,
            reasoning: String,
        }
        
        // First attempt
        let response_result = llm_client.complete_json::<DecompositionResponse>(&prompt).await;
        
        // If first attempt fails, try with a simpler prompt
        let response = match response_result {
            Ok(response) => response,
            Err(first_error) => {
                println!("⚠️ First LLM decomposition attempt failed: {}. Retrying with simplified prompt...", first_error);
                
                // Simplified prompt for retry
                let retry_prompt = format!(
                    "Analyze this task:\n\n{}\n\nWould this task benefit from being broken down into subtasks? Reply with ONLY this JSON format:\n{{\"should_decompose\": true or false, \"reasoning\": \"your reasoning\"}}\n\nNOTHING ELSE.",
                    task_text
                );
                
                // Second attempt
                match llm_client.complete_json::<DecompositionResponse>(&retry_prompt).await {
                    Ok(retry_response) => retry_response,
                    Err(second_error) => {
                        // If both attempts fail, make a heuristic decision
                        println!("⚠️ Second LLM decomposition attempt also failed: {}. Using heuristic decision.", second_error);
                        // Fall back to heuristic
                        let heuristic_result = self.heuristic_should_decompose(task_text)?;
                        return Ok(heuristic_result);
                    }
                }
            }
        };
        
        // Log the reasoning
        println!("💭 LLM decomposition reasoning: {}", response.reasoning);
        
        Ok(response.should_decompose)
    }
    
    /// Use LLM to decompose a task into subtasks, with retry mechanism
    #[cfg(feature = "bidir-delegate")]
    async fn llm_decompose_task(&self, task_text: &str) -> Result<Vec<SubtaskDefinition>, AgentError> {
        // Get LLM config
        let llm_config = self.llm_config.as_ref().ok_or_else(|| {
            AgentError::ConfigError("LLM configuration is not provided".to_string())
        })?;
        
        // Initialize LLM client
        let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
            AgentError::ConfigError("ANTHROPIC_API_KEY environment variable is not set".to_string())
        })?;
        
        let llm_client_config = LlmClientConfig {
            api_key,
            model: llm_config.model.clone(),
            max_tokens: llm_config.max_tokens as u32,
            temperature: llm_config.temperature,
            ..Default::default()
        };
        
        let llm_client = LlmClient::new(llm_client_config).map_err(|e| {
            AgentError::Other(format!("Failed to initialize LLM client: {}", e))
        })?;
        
        // Create prompt with improved formatting instructions
        let prompt = format!(
            "# Task Decomposition\n\n\
            You are an AI assistant that breaks down complex tasks into manageable subtasks.\n\n\
            ## Task Description\n{}\n\n\
            ## Instructions\n\
            Break down this task into 2-5 clear, logical subtasks that collectively achieve the overall goal.\n\
            For each subtask:\n\
            - Provide a clear, specific description of what needs to be done\n\
            - Make sure it's a discrete unit of work\n\
            - Ensure the subtasks collectively cover the entire original task\n\n\
            ## Response Format Requirements\n\
            Return ONLY a JSON object with no additional text, explanations, or decorations.\n\
            The JSON MUST follow this exact structure:\n\
            \n\
            {{\n  \
                \"subtasks\": [\n    \
                    {{\n      \
                        \"description\": \"Clear description of the subtask\"\n    \
                    }},\n    \
                    {{\n      \
                        \"description\": \"Another subtask description\"\n    \
                    }}\n  \
                ]\n\
            }}\n\
            \n\n\
            DO NOT include any other text, markdown formatting, prefixes, or notes outside the JSON.\n\
            DO NOT add any other fields or change the structure.\n\
            DO NOT include backticks, JSON labels, or any other text outside of the JSON object itself.\n\
            Your entire response must be valid JSON that can be parsed directly.",
            task_text
        );
        
        // Define response structure
        #[derive(Deserialize)]
        struct DecompositionResponse {
            subtasks: Vec<Subtask>,
        }
        
        #[derive(Deserialize)]
        struct Subtask {
            description: String,
        }
        
        // Create a simplified version for retry
        let simplified_prompt = format!(
            "Break this task into subtasks: {}\n\n\
            Reply with ONLY this JSON format:\n\
            {{\"subtasks\": [{{\n\"description\": \"subtask 1 details\"}}, \
            {{\"description\": \"subtask 2 details\"}}]}}\n\n\
            Include 2-5 subtasks. NOTHING outside the JSON. NO markdown formatting.",
            task_text
        );
        
        // First attempt
        let response_result = llm_client.complete_json::<DecompositionResponse>(&prompt).await;
        
        // If first attempt fails, try with simplified prompt
        let decomposition_result = match response_result {
            Ok(response) => {
                if response.subtasks.is_empty() {
                    println!("⚠️ LLM returned empty subtasks list, retrying with simplified prompt...");
                    // Retry with simplified prompt if we got an empty list
                    match llm_client.complete_json::<DecompositionResponse>(&simplified_prompt).await {
                        Ok(retry_response) => Ok(retry_response),
                        Err(e) => Err(e),
                    }
                } else {
                    // Use the successful response
                    Ok(response)
                }
            },
            Err(first_error) => {
                println!("⚠️ First LLM decomposition attempt failed: {}. Retrying with simplified prompt...", first_error);
                // Try with simplified prompt
                match llm_client.complete_json::<DecompositionResponse>(&simplified_prompt).await {
                    Ok(retry_response) => Ok(retry_response),
                    Err(second_error) => {
                        println!("⚠️ Second LLM decomposition attempt also failed: {}. Using default subtasks.", second_error);
                        // If both attempts fail, use default subtasks
                        return Ok(self.default_subtasks());
                    }
                }
            }
        };
        
        // Process the decomposition result
        match decomposition_result {
            Ok(decomposition) => {
                // Check if we got a valid response with subtasks
                if decomposition.subtasks.is_empty() {
                    println!("⚠️ Received empty subtasks list, using default subtasks instead");
                    return Ok(self.default_subtasks());
                }
                
                // Log the number of subtasks
                println!("✅ Successfully decomposed task into {} subtasks", decomposition.subtasks.len());
                
                // Convert to SubtaskDefinition
                let subtasks = decomposition.subtasks.into_iter()
                    .map(|subtask| SubtaskDefinition {
                        id: format!("subtask-{}", Uuid::new_v4()),
                        input_message: subtask.description,
                        metadata: Some(serde_json::Map::new()),
                    })
                    .collect();
                
                Ok(subtasks)
            },
            Err(e) => {
                // This should not happen since we already handled errors above
                println!("⚠️ Unexpected error processing decomposition: {}", e);
                Ok(self.default_subtasks())
            }
        }
    }
    /// Creates a message for a subtask.
    fn create_subtask_message(&self, text: &str) -> Message {
        Message {
            role: Role::Agent, // Changed from Role::System to Role::Agent as System is not available
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: text.to_string(),
                metadata: None,
            })],
            metadata: None,
        }
    }
    
    /// Extract text from task parameters
    fn extract_text_from_params(&self, params: &TaskSendParams) -> String {
        let message = &params.message;
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
    
    /// Use LLM to make a routing decision with retry logic for format errors
    async fn llm_routing(
        &self, 
        task_text: &str, 
        tools: &[String], 
        agents: &[(String, String)]
    ) -> Result<RoutingDecision, AgentError> {
        // Get LLM config
        let llm_config = self.llm_config.as_ref().ok_or_else(|| {
            AgentError::ConfigError("LLM configuration is not provided".to_string())
        })?;
        
        // Initialize LLM client
        let api_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
            AgentError::ConfigError("ANTHROPIC_API_KEY environment variable is not set".to_string())
        })?;
        
        let llm_client_config = LlmClientConfig {
            api_key,
            model: llm_config.model.clone(),
            max_tokens: llm_config.max_tokens as u32,
            temperature: llm_config.temperature,
            ..Default::default()
        };
        
        let llm_client = LlmClient::new(llm_client_config).map_err(|e| {
            AgentError::Other(format!("Failed to initialize LLM client: {}", e))
        })?;
        
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
        
        // Create routing prompt with improved formatting instructions
        let prompt = format!(
            "# Task Routing Decision\n\n\
            You are a task router for a bidirectional A2A agent system. Your job is to determine the best way to handle a task.\n\n\
            ## Task Description\n{}\n\n\
            ## Routing Options\n\
            1. LOCAL: Handle the task locally using one or more tools\n\
            {}\n\
            2. REMOTE: Delegate the task to another agent\n\
            {}\n\n\
            ## Instructions\n\
            Analyze the task and decide whether to handle it locally with tools or delegate to another agent.\n\
            If handling locally, specify which tool(s) to use.\n\
            If delegating, specify which agent to delegate to.\n\n\
            ## Response Format Requirements\n\
            Return ONLY a JSON object with no additional text, explanations, or decorations.\n\
            The JSON MUST follow this exact structure:\n\
            \n\
            {{\n  \
                \"decision_type\": \"LOCAL\" or \"REMOTE\",\n  \
                \"reason\": \"Brief explanation of your decision\",\n  \
                \"tools\": [\"tool1\", \"tool2\"] (include only if decision_type is LOCAL),\n  \
                \"agent_id\": \"agent_id\" (include only if decision_type is REMOTE)\n\
            }}\n\
            \n\n\
            DO NOT include any other text, markdown formatting, or explanations outside the JSON.\n\
            DO NOT use non-existent tools or agents - only use the ones listed above.\n\
            If LOCAL decision, you MUST include at least one valid tool from the available tools list.\n\
            If REMOTE decision, you MUST include a valid agent_id from the available agents list.",
            task_text,
            tools_description,
            agents_description
        );
        
        // Define the response structure
        #[derive(Deserialize)]
        struct RoutingResponse {
            decision_type: String,
            reason: String,
            #[serde(default)]
            tools: Vec<String>,
            #[serde(default)]
            agent_id: String,
        }
        
        // First attempt
        let response_result = llm_client.complete_json::<RoutingResponse>(&prompt).await;
        
        // If first attempt fails, try with a simplified, more explicit prompt
        let response = match response_result {
            Ok(response) => response,
            Err(first_error) => {
                println!("⚠️ First LLM routing attempt failed: {}. Retrying with simplified prompt...", first_error);
                
                // Create a more explicit prompt for the retry
                let retry_prompt = format!(
                    "You need to choose how to handle this task in an A2A agent system.\n\n\
                    TASK: {}\n\n\
                    OPTIONS:\n\
                    - LOCAL tools: {}\n\
                    - REMOTE agents: {}\n\n\
                    RETURN ONLY VALID JSON IN THIS EXACT FORMAT:\n\
                    {{\"decision_type\": \"LOCAL or REMOTE\", \"reason\": \"reason\", \"tools\": [\"name\"], \"agent_id\": \"id\"}}\n\n\
                    LOCAL decision MUST have tools array with existing tools.\n\
                    REMOTE decision MUST have agent_id with an existing agent.\n\n\
                    NO explanations. NO markdown. ONLY the JSON object.",
                    task_text,
                    tools.join(", "),
                    agents.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>().join(", ")
                );
                
                // Second attempt with simplified prompt
                match llm_client.complete_json::<RoutingResponse>(&retry_prompt).await {
                    Ok(retry_response) => retry_response,
                    Err(second_error) => {
                        println!("⚠️ Second LLM routing attempt also failed: {}", second_error);
                        return Err(AgentError::RoutingError(format!("LLM routing failed on both attempts: {} | {}", first_error, second_error)));
                    }
                }
            }
        };
        
        // Log the reason for the decision
        println!("💭 Routing reason: {}", response.reason);
        
        // Parse response into routing decision
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
                    if !agents.is_empty() {
                        println!("⚠️ LLM returned REMOTE decision with no agent_id, defaulting to first available agent");
                        return Ok(RoutingDecision::Remote { agent_id: agents[0].0.clone() });
                    }
                    return Err(AgentError::RoutingError("LLM returned REMOTE decision but no agents available".to_string()));
                }
                
                // Validate that the agent exists
                let agent_exists = agents.iter().any(|(id, _)| id == &response.agent_id);
                if !agent_exists {
                    // If the specified agent doesn't exist, default to first agent if available
                    if !agents.is_empty() {
                        println!("⚠️ LLM suggested non-existent agent: {}, defaulting to first available agent", response.agent_id);
                        return Ok(RoutingDecision::Remote { agent_id: agents[0].0.clone() });
                    }
                    return Err(AgentError::RoutingError("No valid agents available".to_string()));
                }
                
                Ok(RoutingDecision::Remote { agent_id: response.agent_id })
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
                if !agents.is_empty() {
                    println!("  ↳ Recovering with REMOTE routing to first available agent");
                    return Ok(RoutingDecision::Remote { agent_id: agents[0].0.clone() });
                }
                
                // No recovery possible
                Err(AgentError::RoutingError(format!("Invalid decision_type from LLM: {} and no recovery options", invalid_type)))
            }
        }
    }
}

// Define traits for injectable routing and synthesis components

/// Trait for a routing agent that can be used for task routing decisions.
#[async_trait]
pub trait RoutingAgentTrait: Send + Sync {
    /// Makes a routing decision based on the task.
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError>;
    
    /// Checks if a task should be decomposed into subtasks.
    #[cfg(feature = "bidir-delegate")]
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, AgentError>;
    
    /// Decomposes a complex task into subtasks.
    #[cfg(feature = "bidir-delegate")]
    async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, AgentError>;
}

/// Trait for a synthesis agent that can combine results from multiple subtasks.
#[async_trait]
#[cfg(feature = "bidir-delegate")]
pub trait SynthesisAgentTrait: Send + Sync {
    /// Synthesizes results from multiple subtasks into a combined result.
    async fn synthesize(&self, subtasks: &[(SubtaskDefinition, String)]) -> Result<String, AgentError>;
}

/// Default implementation of the synthesis agent.
#[cfg(feature = "bidir-delegate")]
pub struct SynthesisAgent {}

#[cfg(feature = "bidir-delegate")]
impl SynthesisAgent {
    /// Creates a new synthesis agent.
    pub fn new() -> Self {
        Self {}
    }
}

#[cfg(feature = "bidir-delegate")]
impl SynthesisAgent {
    /// Creates a new synthesis agent with an API key.
    pub fn with_api_key(api_key: String) -> Self {
        Self {}
    }
    
    /// Synthesize results using LLM if possible, with retry logic and fallback to simple combination if needed
    async fn llm_synthesize(&self, subtasks: &[(SubtaskDefinition, String)]) -> Result<String, AgentError> {
        // Try to get API key from environment
        let api_key = match std::env::var("ANTHROPIC_API_KEY") {
            Ok(key) => key,
            Err(_) => {
                println!("⚠️ ANTHROPIC_API_KEY not set, using fallback synthesis");
                return self.fallback_synthesize(subtasks);
            }
        };
        
        // Create LLM client
        let llm_client_config = LlmClientConfig {
            api_key,
            model: "claude-3-haiku-20240307".to_string(), // Faster model for synthesis
            max_tokens: 2048, // More tokens for synthesis
            temperature: 0.3, // Some creativity in synthesis
            ..Default::default()
        };
        
        let llm_client = match LlmClient::new(llm_client_config) {
            Ok(client) => client,
            Err(e) => {
                println!("⚠️ Failed to initialize LLM client: {}", e);
                return self.fallback_synthesize(subtasks);
            }
        };
        
        // Construct the prompt with subtask results
        let mut subtask_results = String::new();
        for (i, (def, result_text)) in subtasks.iter().enumerate() {
            subtask_results.push_str(&format!("### Subtask {}: {}\n", i+1, def.input_message));
            subtask_results.push_str(&format!("Result:\n{}\n\n", result_text));
        }
        
        // Main prompt with improved formatting instructions
        let prompt = format!(
            "# Task Result Synthesis\n\n\
            You are an AI assistant that synthesizes results from multiple subtasks into a cohesive, comprehensive response.\n\n\
            ## Subtask Results\n\n{}\n\n\
            ## Instructions\n\
            Synthesize the results from all subtasks into a single, unified response:\n\
            - Combine information logically and avoid redundancy\n\
            - Ensure all key insights from each subtask are preserved\n\
            - Structure the response in a clear, coherent manner\n\
            - Make connections between related pieces of information\n\
            - Present a holistic answer that addresses the overall task\n\n\
            ## Response Format Requirements\n\
            - Provide your synthesized response as plain text\n\
            - Use markdown formatting only for basic structure (headings, lists, etc.)\n\
            - Do not include any meta-commentary about your synthesis process\n\
            - Do not include phrases like \"Based on the subtasks\" or \"Here is my synthesis\"\n\
            - Start directly with the synthesized content\n\
            - Do not include the original subtask texts or labels unless they form part of your answer\n\
            - Focus on delivering a coherent, unified response as if it were written as a single piece",
            subtask_results
        );
        
        // Simplified prompt for retry
        let simplified_prompt = format!(
            "Combine these subtask results into one cohesive response:\n\n{}\n\n\
            Important: Write ONLY the combined answer. Do NOT include explanations about your process, \
            do NOT refer to 'subtasks' in your answer, and do NOT include phrases like \
            'based on the results' or 'here is my synthesis'. Just write a clean, cohesive answer \
            that integrates all the information.",
            subtask_results
        );
        
        // First attempt
        match llm_client.complete(&prompt).await {
            Ok(synthesis) => {
                // Check if synthesis looks valid (not empty, not too short)
                if synthesis.trim().is_empty() || synthesis.trim().len() < 20 {
                    println!("⚠️ LLM synthesis appears invalid (too short), retrying with simplified prompt");
                    return self.retry_synthesis(&llm_client, &simplified_prompt, subtasks).await;
                }
                
                // Check if synthesis starts with common error patterns
                let lower_synthesis = synthesis.to_lowercase();
                let error_patterns = [
                    "based on the subtask", "here is my synthesis", "i'll synthesize", 
                    "my synthesis", "synthesized response", "here's a synthesis"
                ];
                
                if error_patterns.iter().any(|&pattern| lower_synthesis.contains(pattern)) {
                    println!("⚠️ LLM synthesis contains meta-commentary, retrying with simplified prompt");
                    return self.retry_synthesis(&llm_client, &simplified_prompt, subtasks).await;
                }
                
                println!("✅ Successfully synthesized results with LLM");
                Ok(synthesis)
            },
            Err(e) => {
                println!("⚠️ LLM synthesis failed: {}, retrying with simplified prompt", e);
                self.retry_synthesis(&llm_client, &simplified_prompt, subtasks).await
            }
        }
    }
    
    /// Retry synthesis with a simplified prompt
    async fn retry_synthesis(
        &self, 
        llm_client: &LlmClient, 
        simplified_prompt: &str,
        subtasks: &[(SubtaskDefinition, String)]
    ) -> Result<String, AgentError> {
        match llm_client.complete(simplified_prompt).await {
            Ok(retry_synthesis) => {
                // Check if retry synthesis is valid
                if retry_synthesis.trim().is_empty() || retry_synthesis.trim().len() < 20 {
                    println!("⚠️ Retry synthesis also invalid, falling back to manual synthesis");
                    return self.fallback_synthesize(subtasks);
                }
                
                println!("✅ Successfully synthesized results with retry prompt");
                Ok(retry_synthesis)
            },
            Err(e) => {
                println!("⚠️ Retry synthesis also failed: {}, using fallback", e);
                self.fallback_synthesize(subtasks)
            }
        }
    }
    
    /// Fallback synthesis method when LLM is not available
    fn fallback_synthesize(&self, subtasks: &[(SubtaskDefinition, String)]) -> Result<String, AgentError> {
        let mut result = String::new();
        
        result.push_str("# Combined Results\n\n");
        
        if subtasks.is_empty() {
            result.push_str("No results available.\n");
            return Ok(result);
        }
        
        // Add summary line
        let subtask_count = subtasks.len();
        result.push_str(&format!("Combined results from {} subtasks:\n\n", subtask_count));
        
        // Add each subtask result
        for (i, (def, result_text)) in subtasks.iter().enumerate() {
            result.push_str(&format!("## Subtask {}: {}\n\n", i+1, def.input_message.clone()));
            result.push_str(result_text);
            result.push_str("\n\n");
        }
        
        // Add a conclusion
        result.push_str("## Overall Summary\n\n");
        result.push_str("The above results represent the combined output from all subtasks. ");
        result.push_str("Please review each section for detailed information.\n");
        
        Ok(result)
    }
}

#[async_trait]
#[cfg(feature = "bidir-delegate")]
impl SynthesisAgentTrait for SynthesisAgent {
    async fn synthesize(&self, subtasks: &[(SubtaskDefinition, String)]) -> Result<String, AgentError> {
        println!("🔄 Synthesizing results from {} subtasks", subtasks.len());
        
        // Try to use LLM for synthesis, fallback to simple combination if it fails
        let synthesis_result = self.llm_synthesize(subtasks).await;
        
        // Return the result
        synthesis_result
    }
}
