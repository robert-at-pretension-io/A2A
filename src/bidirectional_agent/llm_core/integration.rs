//! Integration utilities for the LLM core module.
//!
//! This module provides utilities to help integrate the new LLM core
//! components with the existing codebase.

use crate::bidirectional_agent::{
    agent_registry::AgentRegistry,
    tool_executor::ToolExecutor,
    task_router::LlmTaskRouterTrait,
    llm_routing::LlmRoutingConfig,
    error::AgentError,
};

use crate::bidirectional_agent::llm_core::{
    LlmClient, LlmConfig, ClaudeClient,
    template::TemplateManager,
};

use crate::bidirectional_agent::task_router_llm_refactored::{
    RefactoredLlmTaskRouter,
    RefactoredLlmTaskRouterFactory,
    create_refactored_llm_task_router,
};

use std::sync::Arc;
use std::env;

/// Create an LLM client based on environment configuration.
/// 
/// This function will:
/// 1. Look for ANTHROPIC_API_KEY in the environment
/// 2. If found, create a ClaudeClient
/// 3. If not, create a MockLlmClient with reasonable defaults
pub fn create_llm_client(model: Option<String>) -> Result<Arc<dyn LlmClient>, AgentError> {
    // Try to get API key from environment
    match env::var("ANTHROPIC_API_KEY") {
        Ok(api_key) => {
            // Configure Claude client
            let config = LlmConfig {
                api_key,
                model: model.unwrap_or_else(|| "claude-3-haiku-20240307".to_string()),
                max_tokens: 2048,
                temperature: 0.1,
                timeout_seconds: 30,
            };
            
            // Create Claude client
            let claude = ClaudeClient::new(config)
                .map_err(|e| AgentError::ConfigError(format!("Failed to create Claude client: {}", e)))?;
            
            Ok(Arc::new(claude))
        },
        Err(_) => {
            // No API key, use mock client
            use crate::bidirectional_agent::llm_core::MockLlmClient;
            
            // Create a simple mock client with default responses
            let mock = MockLlmClient::with_default_response(
                r#"{"decision_type": "LOCAL", "reason": "Using mock LLM client", "tools": ["echo"]}"#.to_string()
            );
            
            Ok(Arc::new(mock))
        }
    }
}

/// Convert from legacy LlmRoutingConfig to the new format.
pub fn convert_legacy_config(legacy_config: &LlmRoutingConfig) -> crate::bidirectional_agent::task_router_llm_refactored::LlmRoutingConfig {
    crate::bidirectional_agent::task_router_llm_refactored::LlmRoutingConfig {
        model: legacy_config.model.clone(),
        temperature: legacy_config.temperature,
        max_tokens: legacy_config.max_tokens as u32,
    }
}

/// Create a task router using the new LLM core.
///
/// This function provides a drop-in replacement for create_llm_task_router
/// but using the new LLM core infrastructure.
pub fn create_integrated_llm_router(
    agent_registry: Arc<AgentRegistry>,
    tool_executor: Arc<ToolExecutor>,
    legacy_config: Option<LlmRoutingConfig>,
) -> Result<Arc<dyn LlmTaskRouterTrait>, AgentError> {
    // Convert legacy config to new format if provided
    let config = legacy_config.map(|cfg| convert_legacy_config(&cfg));
    
    // Use the model from config or default
    let model = config.as_ref().map(|cfg| cfg.model.clone());
    
    // Create LLM client
    let llm_client = create_llm_client(model)?;
    
    // Create task router with the client
    RefactoredLlmTaskRouterFactory::create_with_client(
        agent_registry,
        tool_executor,
        llm_client,
        config,
    ).map(|r| r as Arc<dyn LlmTaskRouterTrait>)
}

/// Create a task router that prefers the new implementation but falls back to legacy.
///
/// This is a transitional function that attempts to use the new implementation,
/// but will fall back to the legacy implementation if there's an error.
pub fn create_transitional_llm_router(
    agent_registry: Arc<AgentRegistry>,
    tool_executor: Arc<ToolExecutor>,
    legacy_config: Option<LlmRoutingConfig>,
) -> Arc<dyn LlmTaskRouterTrait> {
    // Try to create the new router
    match create_integrated_llm_router(agent_registry.clone(), tool_executor.clone(), legacy_config.clone()) {
        Ok(router) => {
            println!("✅ Using new LLM core infrastructure for routing");
            router
        },
        Err(e) => {
            // If it fails, fall back to legacy implementation
            println!("⚠️ Failed to initialize new LLM router: {}", e);
            println!("⚠️ Falling back to legacy LLM router implementation");
            
            match crate::bidirectional_agent::task_router_llm::create_llm_task_router(
                agent_registry,
                tool_executor,
            ) {
                Ok(router) => router,
                Err(e) => {
                    // If both fail, create a basic router that always routes to echo
                    println!("⚠️ Legacy router also failed: {}", e);
                    println!("⚠️ Creating fallback router that always routes to echo");
                    
                    use crate::bidirectional_agent::task_router::TaskRouter;
                    Arc::new(TaskRouter::new(agent_registry, tool_executor)) as Arc<dyn LlmTaskRouterTrait>
                }
            }
        }
    }
}