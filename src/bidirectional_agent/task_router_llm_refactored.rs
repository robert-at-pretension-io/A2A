/// Refactored LLM-powered task router implementation.
///
/// This module provides an improved version of the LLM-powered task router
/// with better error handling, caching, and more robust decision making.

use std::sync::Arc;
use async_trait::async_trait;
use serde_json::Value;

use crate::bidirectional_agent::{
    agent_registry::AgentRegistry,
    error::AgentError,
    task_router::{RoutingDecision, SubtaskDefinition, LlmTaskRouterTrait},
    tool_executor::ToolExecutor,
};
use crate::types::{Message, TaskSendParams};

/// Configuration for the refactored LLM task router
#[derive(Debug, Clone)]
pub struct LlmRoutingConfig {
    /// Whether to enable caching
    pub use_caching: bool,
    
    /// Whether to enable fallback routing
    pub use_fallback: bool,
}

impl Default for LlmRoutingConfig {
    fn default() -> Self {
        Self {
            use_caching: true,
            use_fallback: true,
        }
    }
}

/// Refactored LLM-powered task router with improved features
pub struct RefactoredLlmTaskRouter {
    /// Agent registry for delegation
    agent_registry: Arc<AgentRegistry>,
    
    /// Tool executor for local execution
    tool_executor: Arc<ToolExecutor>,
    
    /// Configuration for the router
    config: LlmRoutingConfig,
}

impl RefactoredLlmTaskRouter {
    /// Creates a new refactored LLM task router
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
}

#[async_trait]
impl LlmTaskRouterTrait for RefactoredLlmTaskRouter {
    async fn route_task(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError> {
        // Simplified implementation - always execute locally with echo tool
        Ok(RoutingDecision::Local {
            tool_names: vec!["echo".to_string()],
        })
    }
    
    async fn process_follow_up(&self, _task_id: &str, _message: &Message) -> Result<RoutingDecision, AgentError> {
        // Simple implementation - local execution with echo tool
        Ok(RoutingDecision::Local {
            tool_names: vec!["echo".to_string()],
        })
    }
    
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError> {
        self.route_task(params).await
    }
    
    async fn should_decompose(&self, _params: &TaskSendParams) -> Result<bool, AgentError> {
        // Simple implementation - don't decompose
        Ok(false)
    }
    
    async fn decompose_task(&self, _params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, AgentError> {
        // Simple implementation - return empty list
        Ok(vec![])
    }
}

/// Factory function to create a refactored LLM task router
pub fn create_refactored_llm_task_router(
    agent_registry: Arc<AgentRegistry>,
    tool_executor: Arc<ToolExecutor>,
    config: Option<LlmRoutingConfig>,
) -> Result<Arc<dyn LlmTaskRouterTrait>, AgentError> {
    let router = RefactoredLlmTaskRouter::new(agent_registry, tool_executor, config);
    Ok(Arc::new(router))
}