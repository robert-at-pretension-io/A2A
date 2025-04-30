//! LLM-powered task routing implementation.
//! 
//! This module provides a specialized TaskRouter that uses LLMs for
//! making sophisticated routing decisions.

#[cfg(feature = "bidir-local-exec")]
use crate::bidirectional_agent::llm_routing::RoutingAgentTrait;
use crate::bidirectional_agent::agent_directory::AgentDirectory;

use crate::bidirectional_agent::{
    agent_registry::AgentRegistry,
    task_router::{RoutingDecision, SubtaskDefinition, LlmTaskRouterTrait},
    tool_executor::ToolExecutor,
    error::AgentError,
    llm_routing::{RoutingAgent, LlmRoutingConfig},
};
use crate::types::{TaskSendParams, Message};
use std::sync::Arc;
use anyhow::Result;

/// Enhanced TaskRouter that uses LLM-based decision making
pub struct LlmTaskRouter {
    /// Agent registry for delegation
    agent_registry: Arc<AgentRegistry>,
    
    /// Tool executor for local execution
    tool_executor: Arc<ToolExecutor>,
    
    /// LLM-based routing agent
    routing_agent: RoutingAgent,
}

impl LlmTaskRouter {
    /// Creates a new LLM-powered task router.
    pub fn new(
        agent_registry: Arc<AgentRegistry>,
        tool_executor: Arc<ToolExecutor>,
        llm_config: Option<LlmRoutingConfig>,
    ) -> Self {
        let routing_agent = RoutingAgent::new(
            agent_registry.clone(),
            tool_executor.clone(),
            llm_config,
        );
        
        Self {
            agent_registry,
            tool_executor,
            routing_agent,
        }
    }
    
    /// Makes a routing decision based on the task.
    pub async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError> {
        println!("🧠 LLM-powered TaskRouter analyzing task '{}'...", params.id);
        
        // First check for explicit routing hints in metadata
        if let Some(metadata) = &params.metadata {
            if let Some(route_hint) = metadata.get("_route_to").and_then(|v| v.as_str()) {
                println!("  Found explicit routing hint: {}", route_hint);
                return self.handle_routing_hint(route_hint);
            }
        }
        
        // No explicit hint, use LLM for intelligent routing
        self.routing_agent.decide(params).await
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
        self.routing_agent.should_decompose(params).await
    }
    
    /// Decomposes a complex task into subtasks.
    #[cfg(feature = "bidir-delegate")]
    pub async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, AgentError> {
        self.routing_agent.decompose_task(params).await
    }
}

/// Factory for creating LLM-powered task routers
pub struct LlmTaskRouterFactory;

impl LlmTaskRouterFactory {
    /// Creates a new LLM task router with default configuration.
    pub fn create_default(
        agent_registry: Arc<AgentRegistry>,
        tool_executor: Arc<ToolExecutor>,
    ) -> Arc<LlmTaskRouter> {
        Arc::new(LlmTaskRouter::new(
            agent_registry,
            tool_executor,
            None, // Use default LLM config
        ))
    }
    
    /// Creates a new LLM task router with custom configuration.
    pub fn create_with_config(
        agent_registry: Arc<AgentRegistry>,
        tool_executor: Arc<ToolExecutor>,
        llm_config: LlmRoutingConfig,
    ) -> Arc<LlmTaskRouter> {
        Arc::new(LlmTaskRouter::new(
            agent_registry,
            tool_executor,
            Some(llm_config),
        ))
    }
}

// Provide a helper function to integrate with existing code
pub fn create_llm_task_router(
    agent_registry: Arc<AgentRegistry>,
    tool_executor: Arc<ToolExecutor>,
) -> Arc<dyn LlmTaskRouterTrait> {
    LlmTaskRouterFactory::create_default(agent_registry, tool_executor)
}

// Use the LlmTaskRouterTrait from task_router.rs

#[async_trait::async_trait]
impl LlmTaskRouterTrait for LlmTaskRouter {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bidirectional_agent::config::DirectoryConfig;
    use crate::bidirectional_agent::agent_directory::AgentDirectory;
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
    async fn test_handle_routing_hint() {
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
        
        let router = LlmTaskRouter::new(registry.clone(), tool_executor, None);
        
        // Test "local" hint
        let result = router.handle_routing_hint("local");
        assert!(result.is_ok());
        if let Ok(RoutingDecision::Local { tool_names }) = result {
            assert_eq!(tool_names, vec!["echo".to_string()]);
        } else {
            panic!("Expected Local routing decision");
        }
        
        // Test "reject" hint
        let result = router.handle_routing_hint("reject");
        assert!(result.is_ok());
        if let Ok(RoutingDecision::Reject { reason }) = result {
            assert!(reason.contains("rejected by routing hint"));
        } else {
            panic!("Expected Reject routing decision");
        }
        
        // Test non-existent agent hint
        let result = router.handle_routing_hint("non-existent-agent");
        assert!(result.is_err());
    }
    
    #[tokio::test]
    async fn test_routing_with_explicit_hint() {
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
        
        let router = LlmTaskRouter::new(registry, tool_executor, None);
        
        // Create metadata with routing hint
        let mut metadata = serde_json::Map::new();
        metadata.insert("_route_to".to_string(), json!("local"));
        
        let params = create_test_params("task1", "Test task", Some(metadata));
        let result = router.decide(&params).await;
        
        assert!(result.is_ok());
        if let Ok(RoutingDecision::Local { tool_names }) = result {
            assert_eq!(tool_names, vec!["echo".to_string()]);
        } else {
            panic!("Expected Local routing decision");
        }
    }
}
