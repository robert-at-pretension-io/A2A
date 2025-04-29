//! Decides whether to execute a task locally or delegate it.

#![cfg(feature = "bidir-local-exec")]

use crate::bidirectional_agent::{
    agent_registry::AgentRegistry,
    tool_executor::ToolExecutor,
    error::AgentError, // Import AgentError
};
// Import necessary types, including ToolCallPart if used
use crate::types::{TaskSendParams, Role, Part, TextPart, ToolCallPart, ToolCall};
use std::sync::Arc;
use serde_json::{json, Value}; // Import Value

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

    /// Decides how to handle an incoming task.
    /// Checks for directory tool requests first, then falls back to other logic.
    pub async fn decide(&self, task: &TaskSendParams) -> RoutingDecision {
        println!("🧠 Routing task '{}'...", task.id);

        // 1. Check if it's explicitly a directory tool request
        #[cfg(feature = "bidir-core")] // Directory tool only exists if core is enabled
        if let Some(_directory_action) = self.parse_directory_action(task) {
             println!("  Routing decision: Local execution using 'directory' tool.");
             return RoutingDecision::Local {
                 tool_names: vec!["directory".to_string()]
             };
        }

        // 2. Check for explicit routing hints in metadata
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
        #[cfg(feature = "bidir-delegate")]
        self.check_task_complexity(task);
        #[cfg(not(feature = "bidir-delegate"))]
        self.check_task_complexity(task); // Call the no-op version


        // 3. Default Logic: Attempt local execution (placeholder: use "echo")
        // TODO: Implement more sophisticated analysis of task vs. local tools.
        println!("  Default routing: Attempting local execution (defaulting to 'echo').");
        RoutingDecision::Local { tool_names: vec!["echo".to_string()] }
    }

    // Helper to parse directory actions from task messages.
    // Returns the parameters Value for the directory tool if found, otherwise None.
    #[cfg(feature = "bidir-core")] // Only needed if directory tool exists
    fn parse_directory_action(&self, task: &TaskSendParams) -> Option<Value> {
        // Iterate through messages, typically looking at the last user message
        // or specific tool call messages.
        for message in task.messages.iter().rev() { // Check recent messages first
            if message.role == Role::User || message.role == Role::Agent { // Agent might also request tools
                for part in &message.parts {
                    // 1. Check for explicit ToolCallPart
                    if let Part::ToolCallPart(tool_call_part) = part {
                        // Attempt to parse the tool_call Value to see if it's for 'directory'
                        if let Ok(parsed_call) = serde_json::from_value::<ToolCall>(tool_call_part.tool_call.clone()) {
                            if parsed_call.name == "directory" {
                                log::debug!(target: "task_router", task_id=%task.id, "Detected ToolCallPart for 'directory'");
                                return Some(parsed_call.params);
                            }
                        }
                    }
                    // 2. Check TextPart for embedded JSON representing a ToolCall
                    else if let Part::TextPart(text_part) = part {
                        // Try parsing the text as a ToolCall JSON object
                        if let Ok(tool_call) = serde_json::from_str::<ToolCall>(&text_part.text) {
                            if tool_call.name == "directory" {
                                log::debug!(target: "task_router", task_id=%task.id, "Detected JSON ToolCall in TextPart for 'directory'");
                                return Some(tool_call.params);
                            }
                        }

                        // 3. Fallback: Simple keyword matching in text content
                        let text = text_part.text.to_lowercase();
                        if text.contains("list active agents") || (text.contains("agent directory") && text.contains("list")) {
                             log::debug!(target: "task_router", task_id=%task.id, "Detected keyword match for 'list_active'");
                            // Return parameters suitable for DirectoryTool: {"action": "list_active"}
                            return Some(json!({"action": "list_active"}));
                        }
                        if text.contains("list inactive agents") {
                             log::debug!(target: "task_router", task_id=%task.id, "Detected keyword match for 'list_inactive'");
                            return Some(json!({"action": "list_inactive"}));
                        }
                        // Add more robust keyword/NLP parsing here if needed, e.g., for get_info
                        // Example: if text contains "info for agent" extract agent ID
                    }
                }
            }
        }
        None // No directory-related action detected in this task
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
             // Add messages field which is now expected by parse_directory_action
            messages: vec![Message {
                role: Role::User,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: text.to_string(),
                    metadata: None,
                })],
                metadata: None,
            }],
        }
    }

     // Helper to create TaskSendParams with a ToolCallPart using ToolCall struct
    #[cfg(feature = "bidir-core")] // Only needed for directory tool tests
    fn create_tool_call_struct_task_params(id: &str, tool_call: ToolCall, metadata: Option<serde_json::Map<String, Value>>) -> TaskSendParams {
        TaskSendParams {
            id: id.to_string(),
            message: Message { role: Role::User, parts: vec![], metadata: None }, // Outer message optional
            history_length: None,
            metadata,
            push_notification: None,
            session_id: None,
            messages: vec![Message {
                role: Role::User, // Or Agent
                parts: vec![Part::ToolCallPart(ToolCallPart {
                    type_: "tool_call".to_string(),
                    // Use a consistent or generated ID for the call part itself
                    id: format!("{}-call", tool_call.name),
                    tool_call: serde_json::to_value(tool_call).expect("Failed to serialize ToolCall"),
                })],
                metadata: None,
            }],
        }
    }


    // Helper to create a basic test router setup
    // Needs conditional compilation for directory
    async fn setup_test_router() -> TaskRouter {
        #[cfg(feature = "bidir-core")]
        {
            let (registry, directory) = create_test_registry_with_real_dir().await;
            let executor = Arc::new(ToolExecutor::new(directory)); // Pass directory
            TaskRouter::new(registry, executor)
        }
        #[cfg(not(feature = "bidir-core"))]
        {
             let registry = Arc::new(AgentRegistry::new());
             let executor = Arc::new(ToolExecutor::new()); // No directory needed
             TaskRouter::new(registry, executor)
        }
    }


    #[tokio::test]
    async fn test_routing_default_to_local_echo() {
        let router = setup_test_router().await;
        let params = create_text_task_params("task1", "A simple message with no keywords", None);
        let decision = router.decide(&params).await;
        // Default fallback should be local execution (e.g., echo tool)
        assert_eq!(decision, RoutingDecision::Local { tool_names: vec!["echo".to_string()] });
    }

    #[tokio::test]
    async fn test_routing_hint_local() {
        let router = setup_test_router().await;
        let mut metadata = HashMap::new();
        metadata.insert("_route_to".to_string(), json!("local"));
        let params = create_text_task_params("task2", "Message with local hint", Some(metadata));
        let decision = router.decide(&params).await;
        // Hint forces local execution (defaulting to echo here)
        assert_eq!(decision, RoutingDecision::Local { tool_names: vec!["echo".to_string()] });
    }

    #[tokio::test]
    #[cfg(feature = "bidir-core")] // This test requires registry interaction
    async fn test_routing_hint_remote_agent_exists() {
        // Setup router with a registry that has a known agent
        let router = setup_test_router().await; // Gets registry via helper

        // Add a mock agent to the registry
        let agent_id = "remote-agent-exists";
        // Use the public helper from registry tests
        let mock_card = crate::bidirectional_agent::agent_registry::tests::create_mock_agent_card(agent_id, "http://remote.test");
        router.agent_registry.agents.insert(agent_id.to_string(), crate::bidirectional_agent::agent_registry::CachedAgentInfo {
             name: agent_id.to_string(), url: "http://remote.com".to_string(), version: "1.0".to_string(),
             capabilities: Default::default(), skills: vec![], default_input_modes: vec![], default_output_modes: vec![],
             description: None, provider: None, documentation_url: None, authentication: None,
         };
         registry.agents.insert(agent_id.to_string(), crate::bidirectional_agent::agent_registry::CachedAgentInfo {
            card: mock_card, last_checked: chrono::Utc::now(),
        });

        let mut metadata = HashMap::new();
        metadata.insert("_route_to".to_string(), json!(agent_id));
        let params = create_text_task_params("task3", "Message with remote hint", Some(metadata));
        let decision = router.decide(&params).await;
        // Hint should route to the known remote agent
        assert_eq!(decision, RoutingDecision::Remote { agent_id: agent_id.to_string() });
    }

    #[tokio::test]
    async fn test_routing_hint_remote_agent_missing() {
        let router = setup_test_router().await; // Router with empty registry initially
        let mut metadata = HashMap::new();
        metadata.insert("_route_to".to_string(), json!("non-existent-agent"));
        let params = create_text_task_params("task4", "Hint for missing remote", Some(metadata));
        let decision = router.decide(&params).await;
        // Should fall back to default local routing if hinted agent isn't in registry
        assert_eq!(decision, RoutingDecision::Local { tool_names: vec!["echo".to_string()] });
    }

    #[tokio::test]
    #[cfg(feature = "bidir-core")] // Test requires directory tool
    async fn test_routing_directory_tool_call_struct() {
        let router = setup_test_router().await;
        // Use ToolCall struct for clarity
        let tool_call = ToolCall {
            name: "directory".to_string(),
            params: json!({"action": "list_active"}),
        };
        let params = create_tool_call_struct_task_params("task5", tool_call, None);
        let decision = router.decide(&params).await;
        // Should route locally to the directory tool
        assert_eq!(decision, RoutingDecision::Local { tool_names: vec!["directory".to_string()] });
    }

    #[tokio::test]
    #[cfg(feature = "bidir-core")] // Test requires directory tool
    async fn test_routing_directory_keyword_list_active() {
        let router = setup_test_router().await;
        let params = create_text_task_params("task6", "Can you list active agents from the agent directory?", None);
        let decision = router.decide(&params).await;
        // Keyword match should route to directory tool
        assert_eq!(decision, RoutingDecision::Local { tool_names: vec!["directory".to_string()] });
    }

    #[tokio::test]
    #[cfg(feature = "bidir-core")] // Test requires directory tool
    async fn test_routing_directory_keyword_list_inactive() {
        let router = setup_test_router().await;
        let params = create_text_task_params("task7", "Show me the list inactive agents.", None);
        let decision = router.decide(&params).await;
        assert_eq!(decision, RoutingDecision::Local { tool_names: vec!["directory".to_string()] });
    }


    #[tokio::test]
    #[cfg(feature = "bidir-core")] // Test requires directory tool setup
    async fn test_routing_non_directory_tool_call() {
        let router = setup_test_router().await;
        let tool_call = ToolCall {
            name: "shell".to_string(), // A different tool
            params: json!({"command": "pwd"}),
        };
        let params = create_tool_call_struct_task_params("task8", tool_call, None);
        let decision = router.decide(&params).await;
        // Should fall back to default local echo routing as it's not a directory call
        assert_eq!(decision, RoutingDecision::Local { tool_names: vec!["echo".to_string()] });
    }
}
//! Decides whether to execute a task locally or delegate it.

// Only compile if local execution feature is enabled
#![cfg(feature = "bidir-local-exec")]

use crate::bidirectional_agent::{
    agent_registry::AgentRegistry,
    tool_executor::ToolExecutor,
    error::AgentError, // Import AgentError
};
use crate::types::TaskSendParams;
use std::sync::Arc;
use async_trait::async_trait; // Import async_trait

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

// Define the trait for LLM-based routing (can be implemented by LlmTaskRouter)
#[async_trait]
pub trait LlmTaskRouterTrait: Send + Sync + 'static {
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError>;

    #[cfg(feature = "bidir-delegate")]
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, AgentError>;

    #[cfg(feature = "bidir-delegate")]
    async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, AgentError>;
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

        // 2. Default Logic: Try local execution first (using placeholder "echo" tool)
        // In a real scenario, analyze capabilities needed vs. local tools available.
        println!("  Default routing: Attempting local execution first.");
        // For Slice 2, always assume we have a local "echo" tool.
        RoutingDecision::Local { tool_names: vec!["echo".to_string()] }
    }
}

// Implement the LlmTaskRouterTrait for the standard TaskRouter
// This allows using Arc<dyn LlmTaskRouterTrait> polymorphically
#[async_trait]
impl LlmTaskRouterTrait for TaskRouter {
     async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, AgentError> {
         // The standard TaskRouter doesn't return a Result<>, so wrap it
         Ok(self.decide(params).await)
     }

     #[cfg(feature = "bidir-delegate")]
     async fn should_decompose(&self, _params: &TaskSendParams) -> Result<bool, AgentError> {
         // Standard router doesn't support decomposition
         Ok(false)
     }

     #[cfg(feature = "bidir-delegate")]
     async fn decompose_task(&self, _params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, AgentError> {
         // Standard router doesn't support decomposition
         Err(AgentError::RoutingError("Standard router does not support task decomposition".to_string()))
     }
}


#[cfg(test)]
mod tests {
#[cfg(test)]
mod tests {
    use super::*;
    // use crate::bidirectional_agent::config::BidirectionalAgentConfig; // Not directly needed for router tests
    use crate::{
        bidirectional_agent::{
            agent_registry::AgentRegistry,
            tool_executor::ToolExecutor,
            // Import mock/test helpers if needed for AgentDirectory
            #[cfg(feature = "bidir-core")] // Conditionally import test helpers
            agent_registry::tests::create_test_registry_with_real_dir,
        },
        types::{Message, Role, Part, TextPart, ToolCallPart, TaskSendParams, ToolCall}, // Ensure ToolCall is imported
    };
    use std::{collections::HashMap, sync::Arc};
    use serde_json::{json, Value};

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
