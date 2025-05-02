//! Routes incoming tasks to appropriate handlers.

#![cfg(feature = "bidir-local-exec")]

use crate::bidirectional_agent::types::{ToolCall, ToolCallPart, ExtendedPart};
// Import AgentRegistry and ToolExecutor
use crate::bidirectional_agent::agent_registry::AgentRegistry;
use crate::bidirectional_agent::tool_executor::ToolExecutor;
use crate::types::{TaskSendParams, Role, Part, TextPart, Message}; // Import Message
use std::sync::Arc;
use std::collections::HashMap; // Import HashMap
use serde_json::{json, Value}; // Import Value
use tempfile; // For creating temp directories in tests

/// Represents the decision made by the TaskRouter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingDecision {
    /// Execute locally with the specified tool(s)
    Local { tool_names: Vec<String> },
    
    /// Forward to a remote agent with the specified ID
    Remote { agent_id: String },
    
    /// Reject the task with a provided reason
    Reject { reason: String },
    
    /// Decompose into multiple subtasks
    #[cfg(feature = "bidir-delegate")]
    Decompose { subtasks: Vec<SubtaskDefinition> },
}

/// Definition of a subtask for task decomposition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtaskDefinition {
    /// Unique ID for the subtask (will be used as task_id)
    pub id: String,
    /// Input message for the subtask
    pub input_message: String, // Input message text for the subtask
    /// Additional metadata for the subtask
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}

/// Task routing trait shared between standard router and LLM-based router.
#[async_trait::async_trait]
pub trait LlmTaskRouterTrait: Send + Sync + 'static {
    /// Decide how to route a task based on its content and metadata.
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, crate::bidirectional_agent::error::AgentError>;
    
    /// Optional: Check if a task should be decomposed into multiple subtasks.
    #[cfg(feature = "bidir-delegate")]
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, crate::bidirectional_agent::error::AgentError> {
        // Default implementation always returns false 
        // (derived implementations can override)
        Ok(false)
    }
    
    /// Optional: Decompose a task into multiple subtasks.
    #[cfg(feature = "bidir-delegate")]
    async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, crate::bidirectional_agent::error::AgentError> {
        // Default empty implementation - specialized routers should override 
        // if they support decomposition
        Err(crate::bidirectional_agent::error::AgentError::RoutingError(
            "This router does not support task decomposition".to_string()
        ))
    }
}

/// Routes tasks based on metadata or using keyword and agent capability matching.
pub struct TaskRouter {
    /// Agent registry for delegation
    agent_registry: Arc<AgentRegistry>,
    
    /// Tool executor for local execution
    tool_executor: Arc<ToolExecutor>,
}

impl TaskRouter {
    /// Creates a new TaskRouter.
    pub fn new(agent_registry: Arc<AgentRegistry>, tool_executor: Arc<ToolExecutor>) -> Self {
        Self {
            agent_registry,
            tool_executor,
        }
    }

    /// Decide where to route a task based on task parameters.
    pub async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, crate::bidirectional_agent::error::AgentError> {
        // A series of routing rules, checked in priority order:
        
        // 1. Check for explicit routing hints in metadata
        if let Some(metadata) = &params.metadata {
            if let Some(route_hint) = metadata.get("_route_to").and_then(|v| v.as_str()) {
                println!("  Found explicit routing hint: {}", route_hint);
                return self.handle_routing_hint(route_hint);
            }
        }
        
        // 2. Check for directory-specific actions
        #[cfg(feature = "bidir-core")]
        if let Some(directory_params) = self.parse_directory_action(params) {
            return Ok(RoutingDecision::Local { 
                tool_names: vec!["directory".to_string()] 
            });
        }
        
        // 3. Apply keyword-based routing
        // TODO: Implement more sophisticated routing based on task content
        
        // Default fallback: local execution with echo tool
        // Eventually could check agent capabilities against task requirements
        Ok(RoutingDecision::Local { tool_names: vec!["echo".to_string()] })
    }
    
    /// Handles explicit routing hints from metadata.
    fn handle_routing_hint(&self, hint: &str) -> Result<RoutingDecision, crate::bidirectional_agent::error::AgentError> {
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
                    Err(crate::bidirectional_agent::error::AgentError::RoutingError(
                        format!("Unknown routing hint: {}", hint)
                    ))
                }
            }
        }
    }
    
    /// Parses a task to detect if it's a directory-specific action.
    #[cfg(feature = "bidir-core")]
    fn parse_directory_action(&self, params: &TaskSendParams) -> Option<Value> {
        println!("🔍 Checking for directory tool actions in task");

        // Check the message for directory mentions
        let message = &params.message;
        for part in &message.parts {
            // Convert Part to ExtendedPart if possible
            let ext_part = ExtendedPart::from(part.clone());

            match ext_part {
                // 1. Check for explicit ToolCallPart
                ExtendedPart::ToolCallPart(tool_call_part) => {
                    // Attempt to parse the tool_call Value
                    if let Ok(parsed_call) = serde_json::from_value::<ToolCall>(tool_call_part.tool_call.clone()) {
                        if parsed_call.name == "directory" {
                            println!("Detected ToolCallPart for 'directory'");
                            return Some(parsed_call.params);
                        }
                    }
                }
                // 2. Check TextPart for directory-related text
                ExtendedPart::TextPart(text_part) => {
                    let lower_text = text_part.text.to_lowercase();
                    // Check if the text mentions directory or listing agents
                    if lower_text.contains("agent directory") ||
                       lower_text.contains("list agent") || // Original check
                       lower_text.contains("list active agents") || // Added check
                       lower_text.contains("list inactive agents") || // Added check
                       lower_text.contains("show me the list") || // Added check
                       lower_text.contains("directory tool") || // Tool call mentions
                       lower_text.contains("tool directory") || // Tool call mentions
                       lower_text.contains("use directory") // Tool call mentions
                    {
                        // Basic intent detection - determine action based on keywords
                        let action = if lower_text.contains("inactive") {
                            "list_inactive"
                        } else {
                            "list_active" // Default to active if not specified otherwise
                        };
                        println!("Detected directory mention in text, action: {}", action);
                        return Some(json!({
                            "action": action
                            // We don't extract agent_id from simple text here
                            // Fixed: removed duplicate "action" field that was causing issues
                        }));
                    }
                }
                _ => {}
            }
        }
        
        // No directory action detected
        None
    }
}

#[async_trait::async_trait]
impl LlmTaskRouterTrait for TaskRouter {
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, crate::bidirectional_agent::error::AgentError> {
        self.decide(params).await
    }
}

// --- Test Helpers (moved outside the tests module) ---

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

// Helper to create a test router
async fn setup_test_router() -> TaskRouter {
    // If bidir-core feature is enabled, use the directory version
    #[cfg(feature = "bidir-core")]
    {
        // Set up a minimal registry and directory for testing
        let temp_dir = tempfile::tempdir().expect("Failed to create temp directory for test");
        let db_path = temp_dir.path().join("test_router_dir.db");
        let dir_config = crate::bidirectional_agent::config::DirectoryConfig {
            db_path: db_path.to_string_lossy().to_string(),
            ..Default::default()
        };
        let directory = Arc::new(crate::bidirectional_agent::agent_directory::AgentDirectory::new(&dir_config).await.expect("Failed to create directory"));
        let registry = crate::bidirectional_agent::agent_registry::AgentRegistry::new(directory.clone());
        let registry = Arc::new(registry); // Convert to Arc
        let executor = Arc::new(ToolExecutor::new(directory));
        TaskRouter::new(registry, executor)
    }

    // If bidir-core is not enabled, use version without directory
    #[cfg(not(feature = "bidir-core"))]
    {
         let registry = Arc::new(AgentRegistry::new()); // Needs Arc::new
         let executor = Arc::new(ToolExecutor::new()); // Needs Arc::new
         TaskRouter::new(registry, executor)
    }
}

// Helper to create test params that mention the directory
#[cfg(feature = "bidir-core")]
fn create_test_directory_params(id: &str, action: &str, filter: &str) -> TaskSendParams {
    TaskSendParams {
        id: id.to_string(),
        message: Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: format!("Please use the agent directory to {} agents with filter: {}", action, filter),
                metadata: None,
            })],
            metadata: None,
        },
        history_length: None,
        metadata: None,
        push_notification: None,
        session_id: None,
    }
}

// Helper to create TaskSendParams with a ToolCallPart using ToolCall struct
#[cfg(feature = "bidir-core")]
fn create_tool_call_struct_task_params(id: &str, tool_call: ToolCall, metadata: Option<serde_json::Map<String, Value>>) -> TaskSendParams {
    TaskSendParams {
        id: id.to_string(),
        message: Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: format!("Call directory tool with action: {}",
                        serde_json::to_string(&tool_call).unwrap_or_default()),
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

// Helper function to convert HashMap to serde_json::Map for testing
fn hashmap_to_json_map(map: HashMap<String, serde_json::Value>) -> serde_json::Map<String, serde_json::Value> {
    let mut json_map = serde_json::Map::new();
    for (k, v) in map {
        json_map.insert(k, v);
    }
    json_map
}


// --- Tests Module ---
#[cfg(test)]
mod tests {
    use super::*; // Import items from parent module, including helpers
    use crate::bidirectional_agent::config::BidirectionalAgentConfig;
    // No need to re-import types already imported above
    // use crate::types::{Message, Role, Part, TextPart};
    // No need to re-import HashMap
    // use std::collections::HashMap;
    use serde_json::json;
    use serde_json::Value;
    // Import the test helper conditionally
    #[cfg(feature = "bidir-core")]
    use crate::bidirectional_agent::agent_registry::tests::create_test_registry_with_real_dir;


    #[tokio::test]
    async fn test_routing_default_to_local_echo() {
        let router = setup_test_router().await;
        let params = create_test_params("task1", "A simple message with no keywords", None);
        let decision = router.decide(&params).await;
        // Default fallback should be local execution (e.g., echo tool)
        assert!(matches!(decision, Ok(RoutingDecision::Local { tool_names }) if tool_names == vec!["echo".to_string()]));
    }

    #[tokio::test]
    async fn test_routing_hint_local() {
        let router = setup_test_router().await;
        let mut metadata = HashMap::new();
        metadata.insert("_route_to".to_string(), json!("local"));
        let params = create_test_params("task2", "Message with local hint", Some(hashmap_to_json_map(metadata)));
        let decision = router.decide(&params).await;
        // Hint forces local execution (defaulting to echo here)
        assert!(matches!(decision, Ok(RoutingDecision::Local { tool_names }) if tool_names == vec!["echo".to_string()]));
    }
    
    #[tokio::test]
    async fn test_routing_hint_remote() {
        let router = setup_test_router().await;
        let agent_id = "test-agent";
        // First register the agent so it exists in the registry
        router.agent_registry.add_test_agent(agent_id, "http://localhost:9000");
        
        // Now test with hint to that agent
        let mut metadata = HashMap::new();
        metadata.insert("_route_to".to_string(), json!(agent_id));
        let params = create_test_params("task3", "Message with remote hint", Some(hashmap_to_json_map(metadata)));
        let decision = router.decide(&params).await;
        // Hint should route to the known remote agent
        assert!(matches!(decision, Ok(RoutingDecision::Remote { agent_id: ref aid }) if aid == agent_id));
    }
    
    #[tokio::test]
    async fn test_routing_hint_unknown_agent() {
        let router = setup_test_router().await;
        let mut metadata = HashMap::new();
        metadata.insert("_route_to".to_string(), json!("non-existent-agent"));
        let params = create_test_params("task4", "Hint for missing remote", Some(hashmap_to_json_map(metadata)));
        let decision = router.decide(&params).await;
        // Should fall back to default local routing if hinted agent isn't in registry
        assert!(decision.is_err());
    }
    
    #[tokio::test]
    async fn test_routing_hint_reject() {
        let router = setup_test_router().await;
        let mut metadata = HashMap::new();
        metadata.insert("_route_to".to_string(), json!("reject"));
        let params = create_test_params("task5", "Message with reject hint", Some(hashmap_to_json_map(metadata)));
        let decision = router.decide(&params).await;
        // Should honor the reject hint
        assert!(matches!(decision, Ok(RoutingDecision::Reject { .. })));
    }
    
    #[cfg(feature = "bidir-core")]
    #[tokio::test]
    async fn test_directory_keyword_routing() {
        let router = setup_test_router().await;
        let params = create_test_params("task6", "Can you list active agents from the agent directory?", None);
        let decision = router.decide(&params).await;
        // Keyword match should route to directory tool
        assert!(matches!(decision, Ok(RoutingDecision::Local { tool_names }) if tool_names == vec!["directory".to_string()]));
    }
    
    #[cfg(feature = "bidir-core")]
    #[tokio::test]
    async fn test_directory_alternate_wording() {
        let router = setup_test_router().await;
        let params = create_test_params("task7", "Show me the list inactive agents.", None);
        let decision = router.decide(&params).await;
        assert!(matches!(decision, Ok(RoutingDecision::Local { tool_names }) if tool_names == vec!["directory".to_string()]));
    }
    
    // Use the global helper function for hashmap_to_json_map
}
