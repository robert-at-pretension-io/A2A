//! LLM-based routing and task decomposition.
//! 
//! This module provides functions to use large language models for making
//! intelligent routing decisions, selecting appropriate tools, decomposing
//! complex tasks, and synthesizing results.

#![cfg(feature = "bidir-local-exec")]

use crate::bidirectional_agent::{
    agent_registry::AgentRegistry,
    task_router::{RoutingDecision, SubtaskDefinition},
    tool_executor::ToolExecutor,
    error::AgentError,
};
use crate::types::{TaskSendParams, Message, Role, Part, TextPart, Task};
use async_trait::async_trait;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use anyhow::Result;

/// Configuration for LLM routing.
#[derive(Debug, Clone)]
pub struct LlmRoutingConfig {
    /// URL of the LLM API endpoint
    pub api_url: String,
    
    /// API key for authentication
    pub api_key: String,
    
    /// Model to use for routing decisions
    pub model: String,
    
    /// Temperature for routing (lower = more deterministic)
    pub routing_temperature: f32,
    
    /// Temperature for decomposition (may want more creativity)
    pub decomposition_temperature: f32,
    
    /// Maximum tokens to generate
    pub max_tokens: u32,
}

impl Default for LlmRoutingConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.anthropic.com/v1/messages".to_string(),
            api_key: "".to_string(),
            model: "claude-3-haiku-20240307".to_string(),
            routing_temperature: 0.1,
            decomposition_temperature: 0.3,
            max_tokens: 2048,
        }
    }
}

/// LLM-based router that makes intelligent routing decisions.
pub struct LlmRouter {
    /// Registry of agents for delegation
    agent_registry: Arc<AgentRegistry>,
    
    /// Tool executor for capability checking
    tool_executor: Arc<ToolExecutor>,
    
    /// Configuration for LLM API
    config: LlmRoutingConfig,
}

/// Structured output for routing decisions
#[derive(Serialize, Deserialize, Debug)]
struct RoutingOutput {
    /// Decision type: "local", "remote", "decompose", or "reject"
    decision: String,
    
    /// Reasoning behind the decision
    reasoning: String,
    
    /// Tools to use for local execution (if decision is "local")
    tools: Option<Vec<String>>,
    
    /// Agent ID for delegation (if decision is "remote")
    agent_id: Option<String>,
    
    /// Rejection reason (if decision is "reject")
    rejection_reason: Option<String>,
    
    /// Subtasks (if decision is "decompose")
    subtasks: Option<Vec<SubtaskInfo>>,
}

/// Information about a subtask for decomposition
#[derive(Serialize, Deserialize, Debug)]
struct SubtaskInfo {
    /// ID for the subtask
    id: String,
    
    /// Description of the subtask
    description: String,
    
    /// Input message for the subtask
    input: String,
    
    /// Tools or capabilities needed for this subtask
    tools_needed: Vec<String>,
}

impl LlmRouter {
    /// Creates a new LLM router.
    pub fn new(
        agent_registry: Arc<AgentRegistry>,
        tool_executor: Arc<ToolExecutor>,
        config: Option<LlmRoutingConfig>,
    ) -> Self {
        Self {
            agent_registry,
            tool_executor,
            config: config.unwrap_or_default(),
        }
    }
    
    /// Makes a routing decision based on the task.
    pub async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError> {
        println!("🧠 LLM Router analyzing task '{}'...", params.id);
        
        // Gather necessary context for LLM
        let local_capabilities = self.get_local_capabilities();
        let remote_agents = self.get_remote_agents_info();
        let task_text = self.extract_task_text(params);
        
        // Use LLM to make routing decision
        let routing_output = self.query_llm_for_routing(
            &task_text,
            &local_capabilities,
            &remote_agents,
        ).await?;
        
        // Convert LLM output to RoutingDecision
        self.convert_to_routing_decision(routing_output)
    }
    
    /// Gets information about local capabilities.
    fn get_local_capabilities(&self) -> Vec<String> {
        // In real implementation, query tool_executor for available tool capabilities
        vec![
            "shell_command".to_string(),
            "http_request".to_string(),
            "ai_assistant".to_string(),
            // More capabilities...
        ]
    }
    
    /// Gets information about remote agents.
    fn get_remote_agents_info(&self) -> Vec<(String, Vec<String>)> {
        // In real implementation, get info from agent_registry
        let mut result = Vec::new();
        for (agent_id, info) in &self.agent_registry.agents {
            let skills: Vec<String> = info.card.skills
                .iter()
                .map(|s| s.id.clone())
                .collect();
            result.push((agent_id.clone(), skills));
        }
        result
    }
    
    /// Extracts text from a task.
    fn extract_task_text(&self, params: &TaskSendParams) -> String {
        // Extract text parts from message
        let mut result = String::new();
        for part in &params.message.parts {
            if let Part::TextPart(text_part) = part {
                result.push_str(&text_part.text);
                result.push_str("\n");
            }
        }
        result.trim().to_string()
    }
    
    /// Queries the LLM for a routing decision.
    async fn query_llm_for_routing(
        &self,
        task_text: &str,
        local_capabilities: &[String],
        remote_agents: &[(String, Vec<String>)],
    ) -> Result<RoutingOutput, AgentError> {
        // In a real implementation, this would call an actual LLM API
        println!("  Querying LLM for routing decision...");
        
        // Create the prompt
        let prompt = self.create_routing_prompt(task_text, local_capabilities, remote_agents);
        
        // In a real implementation, send prompt to LLM API
        // Here, just returning a mock output for placeholder purposes
        Ok(RoutingOutput {
            decision: "local".to_string(),
            reasoning: "Task involves simple text processing which can be handled locally".to_string(),
            tools: Some(vec!["echo".to_string()]),
            agent_id: None,
            rejection_reason: None,
            subtasks: None,
        })
    }
    
    /// Creates a prompt for routing decisions.
    fn create_routing_prompt(
        &self,
        task_text: &str,
        local_capabilities: &[String],
        remote_agents: &[(String, Vec<String>)],
    ) -> String {
        // This is the key part - the prompt engineering for good routing decisions
        format!(
r#"# Task Routing Decision

## Task Description
The user has submitted the following task:

"{}"

## Available Local Capabilities
The system has access to the following local tools/capabilities:
{}

## Available Remote Agents
The system can delegate to the following agents with these capabilities:
{}

## Instructions
Analyze the task and decide whether to:
1. Execute locally using available tools
2. Delegate to a remote agent with suitable capabilities
3. Decompose into smaller subtasks (if complex)
4. Reject if the task cannot be handled

## Routing Decision Format
Provide your response in this JSON format:
```json
{{
  "decision": "local|remote|decompose|reject",
  "reasoning": "detailed reasoning for your decision",
  "tools": ["tool1", "tool2"] (if decision is "local"),
  "agent_id": "agent_id" (if decision is "remote"),
  "rejection_reason": "reason" (if decision is "reject"),
  "subtasks": [
    {{
      "id": "subtask1",
      "description": "what this subtask does",
      "input": "specific input for this subtask",
      "tools_needed": ["tool1", "tool2"]
    }}
  ] (if decision is "decompose")
}}
```"#,
            task_text,
            local_capabilities.join(", "),
            remote_agents.iter()
                .map(|(id, skills)| format!("- {}: {}", id, skills.join(", ")))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
    
    /// Converts the LLM output to a routing decision.
    fn convert_to_routing_decision(&self, output: RoutingOutput) -> Result<RoutingDecision, AgentError> {
        match output.decision.as_str() {
            "local" => {
                let tool_names = output.tools.unwrap_or_else(|| vec!["echo".to_string()]);
                Ok(RoutingDecision::Local { tool_names })
            },
            "remote" => {
                let agent_id = output.agent_id.ok_or_else(|| 
                    AgentError::RoutingError("LLM routing decision missing agent_id".to_string())
                )?;
                Ok(RoutingDecision::Remote { agent_id })
            },
            "reject" => {
                let reason = output.rejection_reason.unwrap_or_else(|| 
                    "Task cannot be handled".to_string()
                );
                Ok(RoutingDecision::Reject { reason })
            },
            "decompose" => {
                #[cfg(feature = "bidir-delegate")]
                {
                    let subtasks = output.subtasks.ok_or_else(|| 
                        AgentError::RoutingError("LLM routing decision missing subtasks".to_string())
                    )?;
                    
                    let subtask_definitions = subtasks.into_iter()
                        .map(|info| SubtaskDefinition {
                            id: info.id,
                            description: info.description,
                            input_message: self.create_subtask_message(&info.input),
                            tools_needed: info.tools_needed,
                        })
                        .collect();
                    
                    Ok(RoutingDecision::Decompose { subtasks: subtask_definitions })
                }
                
                #[cfg(not(feature = "bidir-delegate"))]
                {
                    println!("  ⚠️ Decomposition requested but bidir-delegate feature is not enabled");
                    Ok(RoutingDecision::Reject { 
                        reason: "Decomposition not supported (missing bidir-delegate feature)".to_string() 
                    })
                }
            },
            _ => Err(AgentError::RoutingError(
                format!("Unknown routing decision: {}", output.decision)
            )),
        }
    }
    
    /// Creates a message for a subtask from the input text.
    fn create_subtask_message(&self, input_text: &str) -> Message {
        Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: input_text.to_string(),
                metadata: None,
            })],
            metadata: None,
        }
    }
    
    /// Decides whether a task should be decomposed.
    pub async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, AgentError> {
        // Extract task text
        let task_text = self.extract_task_text(params);
        
        // Create decomposition analysis prompt
        let prompt = self.create_decomposition_analysis_prompt(&task_text);
        
        // In a real implementation, send to LLM API
        // For placeholder, just return a simple heuristic check
        let is_complex = task_text.contains(" and ") || 
                        task_text.contains(",") ||
                        task_text.contains(";") ||
                        task_text.split_whitespace().count() > 30;
        
        Ok(is_complex)
    }
    
    /// Creates a prompt for analyzing whether a task should be decomposed.
    fn create_decomposition_analysis_prompt(&self, task_text: &str) -> String {
        format!(
r#"# Task Complexity Analysis

## Task Description
The user has submitted the following task:

"{}"

## Instructions
Analyze whether this task should be decomposed into smaller subtasks.
Consider these factors:
1. Does the task contain multiple distinct operations?
2. Does it require different tools or capabilities?
3. Could it be parallelized into independent steps?
4. Is it too complex to handle in a single execution?

## Response Format
Provide your answer as a simple "yes" or "no", followed by brief reasoning.
If yes, also suggest how the task could be decomposed into 2-5 subtasks."#,
            task_text
        )
    }
    
    /// Decomposes a complex task into subtasks.
    pub async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, AgentError> {
        // Extract task text
        let task_text = self.extract_task_text(params);
        
        // Get local capabilities for context
        let local_capabilities = self.get_local_capabilities();
        
        // Create decomposition prompt
        let prompt = self.create_decomposition_prompt(&task_text, &local_capabilities);
        
        // In a real implementation, send to LLM API
        // For placeholder, return a mock decomposition
        let subtasks = vec![
            SubtaskDefinition {
                id: format!("{}-sub1", params.id),
                description: "First part of the task".to_string(),
                input_message: self.create_subtask_message("Subtask 1 content"),
                tools_needed: vec!["echo".to_string()],
            },
            SubtaskDefinition {
                id: format!("{}-sub2", params.id),
                description: "Second part of the task".to_string(),
                input_message: self.create_subtask_message("Subtask 2 content"),
                tools_needed: vec!["echo".to_string()],
            },
        ];
        
        Ok(subtasks)
    }
    
    /// Creates a prompt for decomposing a task into subtasks.
    fn create_decomposition_prompt(&self, task_text: &str, local_capabilities: &[String]) -> String {
        format!(
r#"# Task Decomposition

## Task Description
The user has submitted the following complex task:

"{}"

## Available Capabilities
The system has access to the following tools/capabilities:
{}

## Instructions
Decompose this complex task into 2-5 smaller, more manageable subtasks.
Each subtask should:
1. Be clearly defined and focused on a specific part of the overall task
2. Include specific input text that would be sent to that subtask
3. Indicate which tools or capabilities would be needed

## Decomposition Format
Provide your response in this JSON format:
```json
[
  {{
    "id": "subtask1",
    "description": "what this subtask does",
    "input": "specific input for this subtask",
    "tools_needed": ["tool1", "tool2"]
  }},
  {{
    "id": "subtask2",
    "description": "what this subtask does",
    "input": "specific input for this subtask",
    "tools_needed": ["tool3", "tool4"]
  }}
]
```"#,
            task_text,
            local_capabilities.join(", ")
        )
    }
}

/// Service for LLM-based result synthesis from multiple subtasks.
pub struct LlmResultSynthesizer {
    /// Configuration for LLM API
    config: LlmRoutingConfig,
}

impl LlmResultSynthesizer {
    /// Creates a new result synthesizer.
    pub fn new(config: Option<LlmRoutingConfig>) -> Self {
        Self {
            config: config.unwrap_or_default(),
        }
    }
    
    /// Synthesizes results from multiple subtasks.
    pub async fn synthesize_results(
        &self,
        parent_task_text: &str,
        subtasks: &[(SubtaskDefinition, Task)],
    ) -> Result<String, AgentError> {
        println!("🔄 Synthesizing results from {} subtasks...", subtasks.len());
        
        // Extract results from each subtask
        let subtask_results: Vec<(String, String)> = subtasks.iter()
            .map(|(def, task)| {
                let result_text = self.extract_result_text(task);
                (def.description.clone(), result_text)
            })
            .collect();
        
        // Create synthesis prompt
        let prompt = self.create_synthesis_prompt(parent_task_text, &subtask_results);
        
        // In a real implementation, send to LLM API
        // For placeholder, create a simple combined result
        let synthesis = format!(
            "Combined results from {} subtasks:\n\n{}",
            subtasks.len(),
            subtask_results.iter()
                .map(|(desc, result)| format!("- {}: {}", desc, result))
                .collect::<Vec<_>>()
                .join("\n\n")
        );
        
        Ok(synthesis)
    }
    
    /// Extracts result text from a completed task.
    fn extract_result_text(&self, task: &Task) -> String {
        // Get the final message if available
        if let Some(ref msg) = task.status.message {
            for part in &msg.parts {
                if let Part::TextPart(text_part) = part {
                    return text_part.text.clone();
                }
            }
        }
        
        // Fallback if no text part found
        "No textual result available".to_string()
    }
    
    /// Creates a prompt for synthesizing results.
    fn create_synthesis_prompt(
        &self,
        parent_task_text: &str,
        subtask_results: &[(String, String)],
    ) -> String {
        let subtasks_text = subtask_results.iter()
            .map(|(desc, result)| format!("## Subtask: {}\nResult: {}", desc, result))
            .collect::<Vec<_>>()
            .join("\n\n");
            
        format!(
r#"# Result Synthesis

## Original Task
The user requested:

"{}"

## Subtask Results
{}

## Instructions
Synthesize these subtask results into a single, coherent response that addresses the original task.
The synthesis should:
1. Present a unified answer to the original request
2. Integrate information from all subtasks logically
3. Remove redundancies and resolve any contradictions
4. Present the information in a clear, concise manner

## Response Format
Provide a well-formatted, coherent response that could be presented directly to the user."#,
            parent_task_text,
            subtasks_text
        )
    }
}

// Re-export for easier access
pub use LlmRouter as RoutingAgent;
pub use LlmResultSynthesizer as SynthesisAgent;
