/// Task routing interface and implementations
///
/// This module defines the interfaces and base implementations for routing tasks
/// to the appropriate execution methods (local tools, remote agents, etc.)

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

use crate::server::error::ServerError;
use crate::types::{Message, TaskSendParams};

/// Subtask definition for task decomposition
#[derive(Debug, Clone)]
pub struct SubtaskDefinition {
    /// Unique ID for the subtask
    pub id: String,
    
    /// Input message for the subtask
    pub input_message: String,
    
    /// Optional metadata for the subtask
    pub metadata: Option<serde_json::Map<String, Value>>,
}

impl SubtaskDefinition {
    /// Create a new subtask definition
    pub fn new(input_message: String) -> Self {
        Self {
            id: format!("subtask-{}", Uuid::new_v4()),
            input_message,
            metadata: Some(serde_json::Map::new()),
        }
    }
}

/// Routing decision enum for determining how to handle a task
#[derive(Debug, Clone)]
pub enum RoutingDecision {
    /// Handle the task locally with the specified tools and parameters
    Local {
        /// Name of the single tool to use for local execution
        tool_name: String, // Changed from Vec<String> to String
        /// JSON parameters extracted for the tool
        params: Value,
    },

    /// Delegate the task to a remote agent
    Remote {
        /// ID of the agent to delegate to
        agent_id: String,
    },
    
    /// Break the task down into subtasks
    Decompose {
        /// Subtasks to decompose the task into
        subtasks: Vec<SubtaskDefinition>,
    },
    
    /// Reject the task
    Reject {
        /// Reason for rejection
        reason: String,
    },
}

/// Trait for task routers that use LLM capabilities
#[async_trait]
pub trait LlmTaskRouterTrait: Send + Sync {
    /// Determines the routing strategy for a task
    async fn route_task(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError>;
    
    /// Processes a follow-up message for a task
    async fn process_follow_up(&self, task_id: &str, message: &Message) -> Result<RoutingDecision, ServerError>;
    
    /// Determines the routing decision for a task
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError>;
    
    /// Determines if a task should be decomposed
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, ServerError>;
    
    /// Decomposes a task into subtasks
    async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, ServerError>;
}

/// Basic implementation of a task router
pub struct TaskRouter {
    /// List of tools to use for local execution
    pub tools: Vec<String>,
}

impl TaskRouter {
    /// Create a new basic task router
    pub fn new(tools: Vec<String>) -> Self {
        Self { tools }
    }
    
    /// Route a task based on a simple heuristic
    pub fn route(&self, _params: &TaskSendParams) -> RoutingDecision {
        // Simple implementation: always use local tools
        RoutingDecision::Local {
            tool_names: self.tools.clone(),
        }
    }
}

#[async_trait]
impl LlmTaskRouterTrait for TaskRouter {
    async fn route_task(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError> {
        // Simple implementation that always routes to local tools
        Ok(self.route(params))
    }
    
    async fn process_follow_up(&self, _task_id: &str, _message: &Message) -> Result<RoutingDecision, ServerError> {
        // Simple implementation that always routes to local tools
        Ok(RoutingDecision::Local {
            tool_names: self.tools.clone(),
        })
    }
    
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError> {
        // Simple implementation that always routes to local tools
        Ok(self.route(params))
    }
    
    async fn should_decompose(&self, _params: &TaskSendParams) -> Result<bool, ServerError> {
        // Simple implementation that never decomposes tasks
        Ok(false)
    }
    
    async fn decompose_task(&self, _params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, ServerError> {
        // Simple implementation that returns an empty list (no decomposition)
        Ok(Vec::new())
    }
}
