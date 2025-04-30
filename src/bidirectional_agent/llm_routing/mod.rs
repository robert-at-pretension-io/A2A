//! LLM-powered routing and task decomposition logic.

#![cfg(feature = "bidir-local-exec")]

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
        // This is a simple implementation that doesn't actually use LLM yet
        // TODO: Add actual LLM-based routing logic
        
        println!("🧠 Making routing decision for '{}' based on capabilities", params.id);

        // 1. Check if the task matches any tool capability
        // Access the tools map directly from the executor
        let tools: Vec<String> = self.tool_executor.tools.keys().cloned().collect();
        if !tools.is_empty() {
            // If we have tools, use the first one
            // This is a simplistic approach; a real implementation would match task to capabilities
            // TODO: Implement better tool matching logic
            return Ok(RoutingDecision::Local { tool_names: vec![tools[0].clone()] });
        }
        
        // 2. Check if we have any agents in the registry to delegate to
        let mut available_agents = Vec::new();
        for agent_pair in self.agent_registry.agents.iter() {
            let agent_id = agent_pair.key();
            let info = agent_pair.value();
            available_agents.push(agent_id.to_string());
        }
        
        if !available_agents.is_empty() {
            // If we have agents, use the first one
            // This is a simplistic approach; a real implementation would match task to agent abilities
            return Ok(RoutingDecision::Remote { agent_id: available_agents[0].clone() });
        }
        
        // 3. Fallback to local echo by default
        Ok(RoutingDecision::Local { tool_names: vec!["echo".to_string()] })
    }
    
    /// Checks if a task should be decomposed into subtasks.
    #[cfg(feature = "bidir-delegate")]
    pub async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, AgentError> {
        // Simple heuristic: check if the task message is complex (contains multiple sentences)
        // In a real implementation, this would use LLM to analyze complexity
        let message = &params.message;
        for part in &message.parts {
            if let Part::TextPart(text_part) = part {
                // Count sentences as a simple heuristic for complexity
                let sentence_count = text_part.text.split('.').filter(|s| !s.trim().is_empty()).count();
                return Ok(sentence_count > 1);
            }
        }
        
        // If no text parts found, assume it doesn't need decomposition
        Ok(false)
    }
    
    /// Decomposes a complex task into subtasks.
    #[cfg(feature = "bidir-delegate")]
    pub async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, AgentError> {
        // This is a mock implementation that would be replaced with actual LLM-based decomposition
        println!("🧩 Decomposing task '{}' into subtasks", params.id);
        
        // In a real implementation, we'd use an LLM to analyze the task and create subtasks
        // For now, just create two dummy subtasks to demonstrate the interface
        let subtasks = vec![
            SubtaskDefinition {
                id: format!("subtask-{}", 1),
                input_message: "Subtask 1 content".to_string(),
                metadata: Some(serde_json::Map::new()),
            },
            SubtaskDefinition {
                id: format!("subtask-{}", 2),
                input_message: "Subtask 2 content".to_string(),
                metadata: Some(serde_json::Map::new()),
            }
        ];
        
        Ok(subtasks)
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

#[async_trait]
#[cfg(feature = "bidir-delegate")]
impl SynthesisAgentTrait for SynthesisAgent {
    async fn synthesize(&self, subtasks: &[(SubtaskDefinition, String)]) -> Result<String, AgentError> {
        // Simple implementation that just combines results with some formatting
        // In a real implementation, this would use LLM to intelligently combine the results
        let mut result = String::new();
        
        result.push_str("# Combined Results\n\n");
        
        for (i, (def, result_text)) in subtasks.iter().enumerate() {
            result.push_str(&format!("## Subtask {}: {}\n\n", i+1, def.input_message.clone()));
            result.push_str(result_text);
            result.push_str("\n\n");
        }
        
        Ok(result)
    }
}
