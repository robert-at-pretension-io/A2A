/// Task routing interface and implementations
///
/// This module defines the interfaces and base implementations for routing tasks
/// to the appropriate execution methods (local tools, remote agents, etc.)
use async_trait::async_trait; // Keep only one
                              // use serde::{Deserialize, Serialize}; // Unused
use serde_json::{json, Value}; // Keep json macro import
                               // use std::sync::Arc; // Unused
use uuid::Uuid;

use crate::server::error::ServerError;
use crate::types::{Message, TaskSendParams};
use serde::{Deserialize, Serialize}; // Add Serialize, Deserialize
use std::collections::HashMap; // Add HashMap for metadata

/// Subtask definition for task decomposition
#[derive(Debug, Clone, Serialize, Deserialize)] // Add Serialize, Deserialize
pub struct SubtaskDefinition {
    /// Unique ID for the subtask (e.g., kebab-case identifier)
    pub id: String,

    /// Input message/prompt for the subtask
    pub input_message: String,

    /// Optional metadata, including dependencies
    /// Example: { "depends_on": ["previous-step-id"] }
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, Value>>, // Use HashMap for easier construction

    // Add a field to store the routing decision for this specific subtask
    // This will be populated *after* the initial decomposition plan is generated.
    #[serde(skip)] // Don't serialize/deserialize this part initially
    pub routing_decision: Option<RoutingDecision>,
}

impl SubtaskDefinition {
    /// Create a new subtask definition with a specific ID
    pub fn new_with_id(id: String, input_message: String) -> Self {
        Self {
            id, // Use provided ID
            input_message,
            metadata: Some(HashMap::new()), // Initialize metadata map
            routing_decision: None,
        }
    }
    /// Create a new subtask definition
    pub fn new(input_message: String) -> Self {
        Self {
            id: format!("subtask-{}", Uuid::new_v4()),
            input_message,
            metadata: Some(HashMap::new()), // Use HashMap instead of serde_json::Map
            routing_decision: None,         // Initialize the missing field
        }
    }
}

/// Routing decision enum for determining how to handle a task
#[derive(Debug, Clone)]
pub enum RoutingDecision {
    /// Handle the task locally with the specified tool and parameters
    Local {
        /// Name of the single tool to use for local execution
        tool_name: String,
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

    // --- New Decisions ---
    /// Task requires clarification before proceeding
    NeedsClarification {
        /// The question to ask the user for clarification
        question: String,
    },
}

/// Trait for task routers that use LLM capabilities
#[async_trait] // <-- Add this attribute here
pub trait LlmTaskRouterTrait: Send + Sync {
    /// Determines the routing strategy for a task
    async fn route_task(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError>;

    /// Processes a follow-up message for a task
    async fn process_follow_up(
        &self,
        task_id: &str,
        message: &Message,
    ) -> Result<RoutingDecision, ServerError>;

    /// Determines the routing decision for a task
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError>;

    /// Determines if a task should be decomposed
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, ServerError>;

    /// Decomposes a task into subtasks
    async fn decompose_task(
        &self,
        params: &TaskSendParams,
    ) -> Result<Vec<SubtaskDefinition>, ServerError>;
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
            // Use the first tool as the tool_name, provide empty params
            tool_name: self
                .tools
                .first()
                .cloned()
                .unwrap_or_else(|| "echo".to_string()),
            params: json!({}),
        }
    }
}

#[async_trait]
impl LlmTaskRouterTrait for TaskRouter {
    async fn route_task(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError> {
        // Simple implementation that always routes to local tools
        Ok(self.route(params))
    }

    async fn process_follow_up(
        &self,
        _task_id: &str,
        _message: &Message,
    ) -> Result<RoutingDecision, ServerError> {
        // Simple implementation that always routes to local tools
        Ok(RoutingDecision::Local {
            // Use the first tool as the tool_name, provide empty params
            tool_name: self
                .tools
                .first()
                .cloned()
                .unwrap_or_else(|| "echo".to_string()),
            params: json!({}),
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

    async fn decompose_task(
        &self,
        _params: &TaskSendParams,
    ) -> Result<Vec<SubtaskDefinition>, ServerError> {
        // Simple implementation that returns an empty list (no decomposition)
        Ok(Vec::new())
    }
}
