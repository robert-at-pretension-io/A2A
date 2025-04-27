//! Decides whether to execute a task locally or delegate it.

#![cfg(feature = "bidir-local-exec")]

use crate::bidirectional_agent::{
    agent_registry::AgentRegistry,
    tool_executor::ToolExecutor,
};
use crate::types::TaskSendParams;
use std::sync::Arc;

/// Represents the decision made by the TaskRouter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingDecision {
    /// Execute the task locally using the specified tools.
    Local { tool_names: Vec<String> },
    /// Delegate the task to the specified remote agent.
    Remote { agent_id: String },
    /// The task cannot be handled locally or remotely.
    Reject { reason: String },
    /// Decompose the task into subtasks.
    #[cfg(feature = "bidir-delegate")] // Only if delegation is enabled
    Decompose { subtasks: Vec<SubtaskDefinition> },
}

/// Definition of a subtask for decomposition.
#[cfg(feature = "bidir-delegate")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtaskDefinition {
    pub id: String, // Full ID for the subtask
    pub description: String, // Description or prompt for the subtask
    pub input_message: crate::types::Message, // Input message for the subtask
    pub tools_needed: Vec<String>, // Tools needed for this subtask
}


/// Routes incoming tasks to either local execution or remote delegation.
#[derive(Clone)]
pub struct TaskRouter {
    agent_registry: Arc<AgentRegistry>,
    tool_executor: Arc<ToolExecutor>,
    // Add routing policy/config later
}

// Forward declaration of LlmTaskRouterTrait to avoid circular dependency
#[async_trait::async_trait]
pub trait LlmTaskRouterTrait: Send + Sync + 'static {
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, crate::bidirectional_agent::error::AgentError>;
    
    #[cfg(feature = "bidir-delegate")]
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, crate::bidirectional_agent::error::AgentError>;
    
    #[cfg(feature = "bidir-delegate")]
    async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, crate::bidirectional_agent::error::AgentError>;
}

impl TaskRouter {
    pub fn new(agent_registry: Arc<AgentRegistry>, tool_executor: Arc<ToolExecutor>) -> Self {
        Self {
            agent_registry,
            tool_executor,
        }
    }

    /// Decides how to handle an incoming task based on its parameters.
    /// This is a simplified implementation for Slice 2.
    pub async fn decide(&self, params: &TaskSendParams) -> RoutingDecision {
        println!("🧠 Routing task '{}'...", params.id);

        // --- Simplified Routing Logic for Slice 2 ---
        // 1. Check for explicit routing hints in metadata (e.g., "_route_to": "local" or "_route_to": "agent_id")
        if let Some(metadata) = &params.metadata {
            if let Some(route_hint) = metadata.get("_route_to").and_then(|v| v.as_str()) {
                if route_hint == "local" {
                    println!("  Routing hint: Execute locally.");
                    // Basic local execution: assume "echo" tool for now
                    return RoutingDecision::Local { tool_names: vec!["echo".to_string()] };
                } else {
                     println!("  Routing hint: Delegate to '{}'.", route_hint);
                    // Check if the target agent is known
                    if self.agent_registry.get(route_hint).is_some() {
                        return RoutingDecision::Remote { agent_id: route_hint.to_string() };
                    } else {
                         println!("  ⚠️ Target agent '{}' from hint not found in registry.", route_hint);
                        // Fall through to default logic if hinted agent not found
                    }
                }
            }
        }

        // Check for task complexity under bidir-delegate feature
        self.check_task_complexity(params);

        // 2. Default Logic: Try local execution first (using placeholder "echo" tool)
        // In a real scenario, analyze capabilities needed vs. local tools available.
        println!("  Default routing: Attempting local execution first.");
        // For Slice 2, always assume we have a local "echo" tool.
        RoutingDecision::Local { tool_names: vec!["echo".to_string()] }
    }

    // Separate helper method to handle task complexity analysis
    #[cfg(feature = "bidir-delegate")]
    fn check_task_complexity(&self, params: &TaskSendParams) {
        use crate::types::Part;
        if params.message.parts.iter().any(|p| matches!(p, Part::TextPart(tp) if tp.text.contains(" and "))) {
            println!("  Task might be complex, considering decomposition (placeholder).");
            // return RoutingDecision::Decompose { subtasks: vec![...] };
        }
    }

    // No-op version when feature is disabled
    #[cfg(not(feature = "bidir-delegate"))]
    fn check_task_complexity(&self, _params: &TaskSendParams) {
        // Do nothing when bidir-delegate feature is not enabled
    }
}

// Implement LlmTaskRouterTrait for TaskRouter to allow for polymorphic usage
#[async_trait::async_trait]
impl LlmTaskRouterTrait for TaskRouter {
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, crate::bidirectional_agent::error::AgentError> {
        // The standard TaskRouter doesn't return a Result<>, so wrap it
        Ok(self.decide(params).await)
    }
    
    #[cfg(feature = "bidir-delegate")]
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, crate::bidirectional_agent::error::AgentError> {
        // Simple heuristic implementation
        use crate::types::Part;
        
        let is_complex = params.message.parts.iter().any(|p| {
            if let Part::TextPart(tp) = p {
                tp.text.contains(" and ") || 
                tp.text.contains(",") || 
                tp.text.contains(";") ||
                tp.text.split_whitespace().count() > 30
            } else {
                false
            }
        });
        
        Ok(is_complex)
    }
    
    #[cfg(feature = "bidir-delegate")]
    async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, crate::bidirectional_agent::error::AgentError> {
        // Simple implementation that breaks the task into two parts
        use crate::types::{Message, Role, Part, TextPart};
        
        let text = params.message.parts.iter()
            .filter_map(|p| {
                if let Part::TextPart(tp) = p {
                    Some(tp.text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        
        // Very simplistic split - in a real implementation we'd use more sophisticated logic
        let midpoint = text.len() / 2;
        let mut split_point = midpoint;
        for (i, c) in text.char_indices().skip(midpoint) {
            if c == '.' || c == '!' || c == '?' || c == '\n' {
                split_point = i + 1;
                break;
            }
        }
        
        let part1 = text[..split_point].trim().to_string();
        let part2 = text[split_point..].trim().to_string();
        
        let subtasks = vec![
            SubtaskDefinition {
                id: format!("{}-sub1", params.id),
                description: "First part of the task".to_string(),
                input_message: Message {
                    role: Role::User,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: part1,
                        metadata: None,
                    })],
                    metadata: None,
                },
                tools_needed: vec!["echo".to_string()],
            },
            SubtaskDefinition {
                id: format!("{}-sub2", params.id),
                description: "Second part of the task".to_string(),
                input_message: Message {
                    role: Role::User,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: part2,
                        metadata: None,
                    })],
                    metadata: None,
                },
                tools_needed: vec!["echo".to_string()],
            },
        ];
        
        Ok(subtasks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bidirectional_agent::config::BidirectionalAgentConfig;
    use crate::types::{Message, Role, Part, TextPart}; // Import necessary types 
    use std::collections::HashMap;
    use serde_json::json;

    // Helper to create basic TaskSendParams
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

    #[tokio::test]
    async fn test_routing_default_to_local() {
        let registry = Arc::new(AgentRegistry::new());
        let executor = Arc::new(ToolExecutor::new());
        let router = TaskRouter::new(registry, executor);

        let params = create_test_params("task1", "Test message", None);
        let decision = router.decide(&params).await;

        assert_eq!(decision, RoutingDecision::Local { tool_names: vec!["echo".to_string()] });
    }

    #[tokio::test]
    async fn test_routing_hint_local() {
        let registry = Arc::new(AgentRegistry::new());
        let executor = Arc::new(ToolExecutor::new());
        let router = TaskRouter::new(registry, executor);

        let mut metadata = HashMap::new();
        metadata.insert("_route_to".to_string(), json!("local"));
        let params = create_test_params("task2", "Route local", Some(metadata.into_iter().collect()));
        let decision = router.decide(&params).await;

        assert_eq!(decision, RoutingDecision::Local { tool_names: vec!["echo".to_string()] });
    }

    #[tokio::test]
    async fn test_routing_hint_remote_agent_exists() {
         let registry = Arc::new(AgentRegistry::new());
         // Manually add a mock agent to the registry for the test
         let agent_id = "remote-agent-1";
         let mock_card = crate::types::AgentCard { /* ... fill with minimal valid data ... */
             name: agent_id.to_string(), url: "http://remote.com".to_string(), version: "1.0".to_string(),
             capabilities: Default::default(), skills: vec![], default_input_modes: vec![], default_output_modes: vec![],
             description: None, provider: None, documentation_url: None, authentication: None,
         };
         registry.agents.insert(agent_id.to_string(), crate::bidirectional_agent::agent_registry::CachedAgentInfo {
             card: mock_card, last_checked: chrono::Utc::now()
         });


        let executor = Arc::new(ToolExecutor::new());
        let router = TaskRouter::new(registry, executor);

        let mut metadata = HashMap::new();
        metadata.insert("_route_to".to_string(), json!(agent_id));
         let params = create_test_params("task3", "Route remote", Some(metadata.into_iter().collect()));
        let decision = router.decide(&params).await;

        assert_eq!(decision, RoutingDecision::Remote { agent_id: agent_id.to_string() });
    }

     #[tokio::test]
    async fn test_routing_hint_remote_agent_missing() {
        // Agent registry is empty
        let registry = Arc::new(AgentRegistry::new());
        let executor = Arc::new(ToolExecutor::new());
        let router = TaskRouter::new(registry, executor);

        let mut metadata = HashMap::new();
        metadata.insert("_route_to".to_string(), json!("non-existent-agent"));
         let params = create_test_params("task4", "Route remote missing", Some(metadata.into_iter().collect()));
        let decision = router.decide(&params).await;

        // Should fall back to default (local) because hinted agent wasn't found
        assert_eq!(decision, RoutingDecision::Local { tool_names: vec!["echo".to_string()] });
    }
}
