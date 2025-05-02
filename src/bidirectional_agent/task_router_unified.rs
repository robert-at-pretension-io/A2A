/// Unified task router implementation
///
/// This module provides a unified task router that combines local and remote execution options.

use std::sync::Arc;
use async_trait::async_trait;
use anyhow::Context;
use serde_json::Value;

use crate::bidirectional_agent::{
    agent_registry::AgentRegistry,
    error::AgentError,
    task_router::{RoutingDecision, SubtaskDefinition, LlmTaskRouterTrait, TaskRouter},
    tool_executor::ToolExecutor,
};
use crate::types::{Message, TaskSendParams};

/// Unified task router that handles both local and remote execution
pub struct UnifiedTaskRouter {
    /// Agent registry for looking up available agents
    agent_registry: Arc<AgentRegistry>,
    
    /// Tool executor for local execution
    tool_executor: Arc<ToolExecutor>,
    
    /// Whether to use LLM for routing decisions
    use_llm: bool,
}

/// Factory function to create a unified task router
pub fn create_unified_task_router(
    agent_registry: Arc<AgentRegistry>,
    tool_executor: Arc<ToolExecutor>,
    use_llm: bool,
) -> Result<Arc<dyn LlmTaskRouterTrait>, AgentError> {
    let router = UnifiedTaskRouter {
        agent_registry,
        tool_executor,
        use_llm,
    };
    
    Ok(Arc::new(router))
}

#[async_trait]
impl LlmTaskRouterTrait for UnifiedTaskRouter {
    async fn route_task(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError> {
        // Simple implementation - local execution with echo tool
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