//! Bidirectional Task Router Implementation

use crate::bidirectional::config::BidirectionalAgentConfig; // Import config
use crate::bidirectional::llm_client::LlmClient;
use crate::server::agent_registry::AgentRegistry;
use crate::server::error::ServerError;
use crate::server::repositories::task_repository::TaskRepository;
use crate::server::task_router::{LlmTaskRouterTrait, RoutingDecision, SubtaskDefinition};
use crate::types::{Message, Part, Role, Task, TaskSendParams, TaskState, TaskStatus};

use anyhow::Result; // Add anyhow
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error, info, instrument, trace, warn};
// Add regex for URL extraction

/// Utility function to extract JSON objects from text where the LLM might add
/// explanatory text before or after the actual JSON object.
pub fn extract_json_from_text(text: &str) -> String {
    // Find the first opening brace and last closing brace
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}')) {
        if start < end {
            // Extract the substring and validate it's valid JSON
            let json_candidate = &text[start..=end];
            if serde_json::from_str::<serde_json::Value>(json_candidate).is_ok() {
                return json_candidate.to_string();
            }
        }
    }

    // If no valid JSON found, return the original text
    text.to_string()
}

/// Task execution mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ExecutionMode {
    /// Process the task locally with the agent
    Local,
    /// Delegate to a remote agent
    Remote { agent_id: String },
}

// Define a struct that implements the server's TaskRouterTrait
#[derive(Clone)]
pub struct BidirectionalTaskRouter {
    llm: Arc<dyn LlmClient>,
    agent_registry: Arc<AgentRegistry>,
    enabled_tools: Arc<Vec<String>>,
    task_repository: Option<Arc<dyn TaskRepository + Send + Sync>>,
    // Store relevant config flags
    experimental_clarification: bool,
    experimental_decomposition: bool,
}

impl BidirectionalTaskRouter {
    pub fn new(
        llm: Arc<dyn LlmClient>,
        agent_registry: Arc<AgentRegistry>,
        enabled_tools: Arc<Vec<String>>,
        task_repository: Option<Arc<dyn TaskRepository + Send + Sync>>,
        config: &BidirectionalAgentConfig, // Pass reference to full config
    ) -> Self {
        // Ensure "echo" and "llm" are always considered enabled internally for fallback
        let mut tools = enabled_tools.as_ref().clone();
        if !tools.contains(&"echo".to_string()) {
            debug!("Implicitly enabling 'echo' tool for fallback.");
            tools.push("echo".to_string());
        }
        if !tools.contains(&"llm".to_string()) {
            debug!("Implicitly enabling 'llm' tool for fallback.");
            tools.push("llm".to_string()); // Fixed: was pushing "echo" again
        }
        Self {
            llm,
            agent_registry,
            enabled_tools: Arc::new(tools),
            task_repository,
            // Store flags from config
            experimental_clarification: config.mode.experimental_clarification,
            experimental_decomposition: config.mode.experimental_decomposition,
        }
    }

    // Helper to format conversation history for prompts
    fn format_history(&self, history: Option<&Vec<Message>>) -> String {
        history
            .map(|h| {
                h.iter()
                    .map(|message| {
                        let role_str = match message.role {
                            Role::User => "User",
                            Role::Agent => "Agent",
                        };
                        let content = message
                            .parts
                            .iter()
                            .filter_map(|part| match part {
                                Part::TextPart(tp) => Some(tp.text.as_str()),
                                Part::FilePart(_) => Some("[File Content]"),
                                Part::DataPart(_) => Some("[Structured Data]"),
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        format!("{}: {}", role_str, content)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    }

    // Helper to format available agents for prompts
    fn format_agents(&self) -> String {
        let available_agents = self.agent_registry.list_all_agents();

        // First add a notice about agent names
        let mut result =
            String::from("=== IMPORTANT: AGENT IDs MUST BE USED EXACTLY AS SHOWN BELOW ===\n\n");

        // Format each agent with more emphasis on ID
        let agent_descriptions = available_agents
            .iter()
            .map(|(agent_id, card)| {
                let mut caps = Vec::new();
                if card.capabilities.push_notifications {
                    caps.push("pushNotifications");
                }
                if card.capabilities.state_transition_history {
                    caps.push("stateTransitionHistory");
                }
                if card.capabilities.streaming {
                    caps.push("streaming");
                }

                format!(
                    "AGENT ID: \"{}\"\nName: {}\nDescription: {}\nCapabilities: {}\nURL: {}",
                    agent_id, // Put in quotes to make it clear this is the exact string to use
                    card.name.as_str(),
                    card.description.as_deref().unwrap_or(""),
                    caps.join(", "),
                    card.url
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        result.push_str(&agent_descriptions);

        // If there are agents listed, add a reminder
        if !available_agents.is_empty() {
            result.push_str("\n\n=== REMINDER: USE EXACT AGENT ID FOR ROUTING ===");
        }

        result
    }

    // Helper to format rolling memory context for LLM prompts
    pub fn format_rolling_memory(
        &self,
        rolling_memory_tasks: &[Task],
        limit: Option<usize>,
    ) -> String {
        if rolling_memory_tasks.is_empty() {
            return "No previous outgoing requests found.".to_string();
        }

        // Limit the number of tasks if specified
        let tasks_to_include = if let Some(max_tasks) = limit {
            // Get the most recent tasks by taking from the end if we have too many
            if rolling_memory_tasks.len() > max_tasks {
                &rolling_memory_tasks[rolling_memory_tasks.len() - max_tasks..]
            } else {
                rolling_memory_tasks
            }
        } else {
            // Use all tasks if no limit specified
            rolling_memory_tasks
        };

        // Format header
        let mut result = format!(
            "=== AGENT MEMORY: {} PREVIOUS OUTGOING REQUESTS ===\n\n",
            tasks_to_include.len()
        );

        // Format each task
        for (i, task) in tasks_to_include.iter().enumerate() {
            // Extract original request text
            let request_text = task
                .history
                .as_ref()
                .and_then(|history| {
                    history
                        .iter()
                        .find(|msg| msg.role == Role::User)
                        .map(|msg| {
                            msg.parts
                                .iter()
                                .filter_map(|part| match part {
                                    Part::TextPart(tp) => Some(tp.text.as_str()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                })
                .unwrap_or_else(|| "Unknown request".to_string());

            // Extract response text
            let response_text = task
                .status
                .message
                .as_ref()
                .map(|msg| {
                    msg.parts
                        .iter()
                        .filter_map(|part| match part {
                            Part::TextPart(tp) => Some(tp.text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_else(|| "No response recorded".to_string());

            // Truncate texts if they're too long
            let truncated_request = if request_text.len() > 200 {
                format!("{}...", &request_text[..197])
            } else {
                request_text
            };

            let truncated_response = if response_text.len() > 300 {
                format!("{}...", &response_text[..297])
            } else {
                response_text
            };

            // Format the task entry
            result.push_str(&format!(
                "Memory #{}: Agent requested information about: \"{}\"\n",
                i + 1,
                truncated_request
            ));

            result.push_str(&format!("Response received: \"{}\"\n", truncated_response));

            // Add a separator between tasks
            if i < tasks_to_include.len() - 1 {
                result.push_str("\n---\n\n");
            }
        }

        result.push_str("\n=== END OF AGENT MEMORY ===\n");
        result
    }

    // Helper to format local tools for prompts
    fn format_tools(&self, include_params: bool) -> String {
        let mut tools_description = String::new();

        // Add special header for tool usage guidance
        tools_description.push_str("=== IMPORTANT TOOL USAGE GUIDANCE ===\n\n");

        // Convert to Vec for ordering and special handling
        let mut tools: Vec<(String, String, String)> = self.enabled_tools.iter()
            .map(|tool_name| {
                let (description, params_hint) = match tool_name.as_str() {
                    "llm" => ("General purpose LLM request. Good for questions, generation, analysis if no specific tool fits.", r#"{"text": "..."}"#),
                    "summarize" => ("Summarizes the input text.", r#"{"text": "..."}"#),
                    "list_agents" => ("Lists known agents. Use only if the task is explicitly about listing agents.", r#"{} or {"format": "simple" | "detailed"}"#),
                    "remember_agent" => ("IMPORTANT: Use this tool to store information about another agent given its URL and automatically connect to it. Use this whenever the user mentions an agent URL or wants to remember/connect to a specific agent. Even if the request is vague like 'remember them' or 'connect to it', use this tool with the most recently mentioned URL.", r#"{"agent_base_url": "http://..."}"#),
                    "execute_command" => ("Executes internal agent commands (like connect, disconnect, servers, session new, card). Use for requests matching these commands.", r#"{"command": "command_name", "args": "arguments_string"}"#),
                    "echo" => ("Simple echo tool for testing. Use only if the task is explicitly to echo.", r#"{"text": "..."}"#),
                    _ => ("A custom tool.", r#"{...}"#), // Generic hint for unknown tools
                };
                (tool_name.clone(), description.to_string(), params_hint.to_string())
            })
            .collect();

        // Sort so key tools appear first
        tools.sort_by(|(a, _, _), (b, _, _)| {
            // Custom sort order - put remember_agent near the top
            if a == "remember_agent" {
                return std::cmp::Ordering::Less;
            }
            if b == "remember_agent" {
                return std::cmp::Ordering::Greater;
            }
            // Then put execute_command next
            if a == "execute_command" {
                return std::cmp::Ordering::Less;
            }
            if b == "execute_command" {
                return std::cmp::Ordering::Greater;
            }
            // Otherwise alphabetical
            a.cmp(b)
        });

        // Format each tool
        for (tool_name, description, params_hint) in tools {
            if include_params {
                tools_description.push_str(&format!(
                    "- {}: {} Expects {}\n",
                    tool_name, description, params_hint
                ));
            } else {
                tools_description.push_str(&format!("- {}: {}\n", tool_name, description));
            }
        }

        // Add special section for using remember_agent
        if self.enabled_tools.contains(&"remember_agent".to_string())
            && self.enabled_tools.contains(&"execute_command".to_string())
        {
            tools_description.push_str("\n=== AGENT CONNECTION WORKFLOW ===\n");
            tools_description
                .push_str("For users asking to connect to or interact with an agent:\n");
            tools_description.push_str("1. Use 'remember_agent' with the agent's URL to store it in the registry AND connect to it\n");
            tools_description.push_str(
                "   The agent will be automatically remembered and connected in one step\n",
            );
            tools_description
                .push_str("Example: If user says \"connect to agent at http://localhost:4202\"\n");
            tools_description.push_str(
                "  - Use 'remember_agent' with {\"agent_base_url\": \"http://localhost:4202\"}\n",
            );
            tools_description.push_str(
                "  - This will both remember AND connect to the agent in one operation\n",
            );
        }

        tools_description
    }

    // Helper to extract rolling memory tasks from metadata and repository
    // Extract rolling memory tasks from metadata and repository
    // Only tasks that were successfully retrieved are included in the result
    async fn extract_rolling_memory(&self, task: &Task) -> Vec<Task> {
        let mut memory_tasks = Vec::new();

        if let Some(metadata) = &task.metadata {
            if let Some(memory_ids_value) = metadata.get("_rolling_memory") {
                debug!("Found rolling memory task IDs in metadata");
                if let Some(memory_ids) = memory_ids_value.as_array() {
                    if let Some(repo) = &self.task_repository {
                        debug!("Retrieving tasks from repository using rolling memory IDs");
                        for id_value in memory_ids {
                            if let Some(id_str) = id_value.as_str() {
                                match repo.get_task(id_str).await {
                                    Ok(Some(found_task)) => memory_tasks.push(found_task),
                                    Ok(None) => {
                                        warn!(task_id = %id_str, "Memory task not found in repository")
                                    }
                                    Err(e) => {
                                        warn!(task_id = %id_str, error = %e, "Failed to retrieve memory task from repository")
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        memory_tasks
    }

    // Helper function to extract URLs from conversation history
    // This is used to improve the "remember them" handling when URLs aren't explicitly provided
    fn extract_url_from_history(&self, history_text: &str) -> Option<String> {
        debug!("Attempting to extract URL from conversation history");

        // Define regex pattern to match URLs (simple pattern, can be improved)
        let url_pattern = regex::Regex::new(
            r"https?://(?:[-\w.]|(?:%[\da-fA-F]{2}))+(?::\d+)?(?:/[-\w%!$&'()*+,;=:@/~]*)?",
        )
        .ok()?;

        // Split history into lines and search from most recent to oldest
        let lines: Vec<&str> = history_text.lines().collect();

        // First search for URLs in messages about "connect to" or similar terms
        for line in lines.iter().rev() {
            // Check if this line contains "connect" or "agent at" and a URL
            if line.to_lowercase().contains("connect")
                || line.to_lowercase().contains("agent at")
                || line.to_lowercase().contains("remember")
            {
                // Look for URL in this line
                if let Some(url_match) = url_pattern.find(line) {
                    debug!(url = %url_match.as_str(), "Found URL in relevant line of conversation history");
                    return Some(url_match.as_str().to_string());
                }
            }
        }

        // If no URL found in relevant lines, look for any URL in history
        for line in lines.iter().rev() {
            if let Some(url_match) = url_pattern.find(line) {
                debug!(url = %url_match.as_str(), "Found URL in general conversation history");
                return Some(url_match.as_str().to_string());
            }
        }

        debug!("No URL found in conversation history");
        None
    }

    // NP1: Check if the request needs clarification
    #[instrument(skip(self, task), fields(task_id = %task.id))]
    async fn check_clarification(
        &self,
        task: &Task,
    ) -> Result<Option<RoutingDecision>, ServerError> {
        if !self.experimental_clarification {
            trace!("Skipping clarification check (disabled by config).");
            return Ok(None); // Skip if disabled
        }

        debug!("NP1: Checking if task requires clarification.");
        let history_text = self.format_history(task.history.as_ref());
        let latest_request = task
            .history
            .as_ref()
            .and_then(|h| h.last())
            .map(|m| {
                m.parts
                    .iter()
                    .filter_map(|p| match p {
                        Part::TextPart(tp) => Some(tp.text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();

        if latest_request.is_empty() {
            warn!("Latest request text is empty, cannot check clarification.");
            return Ok(None); // Cannot proceed without text
        }

        // Get memory context
        let memory_tasks = self.extract_rolling_memory(task).await;
        let memory_text = if !memory_tasks.is_empty() {
            format!(
                "\nAGENT MEMORY OF PREVIOUS OUTGOING REQUESTS:\n{}\n",
                self.format_rolling_memory(&memory_tasks, Some(3))
            )
        } else {
            String::new()
        };

        let prompt = format!(
            r#"SYSTEM:
You are an autonomous AI agent preparing to process a user request.
Your first task is to judge whether the request is specific and complete.{}

CONVERSATION_HISTORY:
{}

LATEST_REQUEST:
"{}"

TASK:
Analyze the LATEST_REQUEST in the context of CONVERSATION_HISTORY and AGENT_MEMORY.
Respond with a JSON object matching the following schema:
{{
  "type": "object",
  "properties": {{
    "clarity": {{ "type": "string", "enum": ["CLEAR", "NEEDS_CLARIFY"] }},
    "question": {{ "type": "string", "description": "A single sentence question for the human if clarity is NEEDS_CLARIFY. Omit if CLEAR." }}
  }},
  "required": ["clarity"]
}}

If clarity is NEEDS_CLARIFY, the question MUST be answerable in ≤ 1 sentence.
If clarity is CLEAR, omit the "question" field or set it to null.
Do not add any explanations or text outside the JSON object."#,
            memory_text, history_text, latest_request
        );

        let schema = json!({
            "type": "object",
            "properties": {
                "clarity": { "type": "string", "enum": ["CLEAR", "NEEDS_CLARIFY"] },
                "question": { "type": "string", "description": "A single sentence question for the human if clarity is NEEDS_CLARIFY. Omit if CLEAR." }
            },
            "required": ["clarity"]
        });

        trace!(prompt = %prompt, "NP1: Clarity check prompt.");
        let decision_val = self
            .llm
            .complete_structured(&prompt, None, schema.clone())
            .await
            .map_err(|e| {
                error!(error = %e, "NP1: LLM structured call failed during clarification check.");
                ServerError::Internal(format!("LLM error during clarification: {}", e))
            })?;
        trace!(?decision_val, "NP1: LLM structured response received.");

        if let Some(clarity_str) = decision_val.get("clarity").and_then(Value::as_str) {
            if clarity_str == "NEEDS_CLARIFY" {
                if let Some(question) = decision_val.get("question").and_then(Value::as_str) {
                    if !question.is_empty() {
                        info!(clarification_question = %question, "NP1: Task needs clarification.");
                        return Ok(Some(RoutingDecision::NeedsClarification {
                            question: question.to_string(),
                        }));
                    } else {
                        warn!(?decision_val, "NP1: LLM indicated clarification needed but question was empty. Proceeding without clarification.");
                    }
                } else {
                    warn!(?decision_val, "NP1: LLM indicated clarification needed but 'question' field missing or not a string. Proceeding without clarification.");
                }
            } else if clarity_str == "CLEAR" {
                debug!("NP1: Task is clear, proceeding to next step.");
            } else {
                warn!(
                    ?decision_val,
                    "NP1: Unexpected 'clarity' value in LLM response. Assuming clear."
                );
            }
        } else {
            warn!(
                ?decision_val,
                "NP1: LLM response missing 'clarity' field or not a string. Assuming clear."
            );
        }

        Ok(None) // Assume clear if check skipped, failed, or explicitly clear
    }

    // NP2: Check if the task should be decomposed and generate plan
    #[instrument(skip(self, task), fields(task_id = %task.id))]
    async fn check_decomposition(
        &self,
        task: &Task,
    ) -> Result<Option<RoutingDecision>, ServerError> {
        if !self.experimental_decomposition {
            trace!("Skipping decomposition check (disabled by config).");
            return Ok(None); // Skip if disabled
        }

        debug!("NP2: Checking if task should be decomposed.");
        let history_text = self.format_history(task.history.as_ref()); // Reuse helper
        let latest_request = task
            .history
            .as_ref()
            .and_then(|h| h.last())
            .map(|m| {
                m.parts
                    .iter()
                    .filter_map(|p| match p {
                        Part::TextPart(tp) => Some(tp.text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();

        if latest_request.is_empty() {
            warn!("Latest request text is empty, cannot check decomposition.");
            return Ok(None);
        }

        // Get memory context
        let memory_tasks = self.extract_rolling_memory(task).await;
        let memory_text = if !memory_tasks.is_empty() {
            format!(
                "\nAGENT MEMORY OF PREVIOUS OUTGOING REQUESTS:\n{}\n",
                self.format_rolling_memory(&memory_tasks, Some(3))
            )
        } else {
            String::new()
        };

        // --- NP2.A: Should Decompose? ---
        let tool_table = self.format_tools(false); // Don't need params hint here
        let agent_table = self.format_agents();
        let should_decompose_prompt = format!(
            r#"SYSTEM:
You are an expert AI planner.

REQUEST_GOAL:
"{}"{}

AVAILABLE_LOCAL_TOOLS:
{}

AVAILABLE_REMOTE_AGENTS:
{}

CRITERIA:
• If the goal obviously maps to ONE local tool or ONE remote agent, do NOT decompose.
• Decompose when fulfilling the goal clearly needs multiple distinct skills or ordered steps.
RESPONSE_FORMAT:
Respond with a JSON object matching this schema:
{{
  "type": "object",
  "properties": {{
    "should_decompose": {{ "type": "boolean" }},
    "reason": {{ "type": "string", "description": "A one-line reason for your decision." }}
  }},
  "required": ["should_decompose", "reason"]
}}
Do not add any explanations or text outside the JSON object."#,
            latest_request, memory_text, tool_table, agent_table
        );

        let schema_a = json!({
            "type": "object",
            "properties": {
                "should_decompose": { "type": "boolean" },
                "reason": { "type": "string" }
            },
            "required": ["should_decompose", "reason"]
        });

        trace!(prompt = %should_decompose_prompt, "NP2.A: Should-decompose prompt.");
        let decision_val_a = self.llm.complete_structured(&should_decompose_prompt, None, schema_a.clone()).await.map_err(|e| {
            error!(error = %e, "NP2.A: LLM structured call failed during should-decompose check.");
            ServerError::Internal(format!("LLM error during should-decompose: {}", e))
        })?;
        trace!(?decision_val_a, "NP2.A: LLM structured response received.");

        if !decision_val_a
            .get("should_decompose")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            debug!(
                "NP2.A: LLM decided not to decompose. Reason: {:?}",
                decision_val_a.get("reason").and_then(|v| v.as_str())
            );
            return Ok(None); // Don't decompose
        }

        info!("NP2.A: LLM decided task should be decomposed. Proceeding to generate plan. Reason: {:?}", decision_val_a.get("reason").and_then(|v| v.as_str()));

        // --- NP2.B: Generate Decomposition Plan ---
        let plan_prompt = format!(
            r#"SYSTEM:
You chose to decompose.

GOAL:
"{}"{}

Produce a JSON array where each element is an object matching this schema:
{{
  "type": "object",
  "properties": {{
    "id": {{ "type": "string", "description": "A unique kebab-case identifier for this subtask step." }},
    "input_message": {{ "type": "string", "description": "The prompt or instruction to execute for this subtask." }},
    "metadata": {{
      "type": "object",
      "properties": {{
        "depends_on": {{ "type": "array", "items": {{ "type": "string" }}, "description": "Array of 'id's of subtasks that must complete before this one can start. Empty if no dependencies." }}
      }},
      "required": ["depends_on"]
    }}
  }},
  "required": ["id", "input_message", "metadata"]
}}
• Keep the plan to ≤ 5 steps.
• Ensure correct dependency order.
• Respond ONLY with the JSON array. Do not add any explanations or text outside the JSON array."#,
            latest_request, memory_text
        );

        let schema_b = json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "id": { "type": "string" },
                    "input_message": { "type": "string" },
                    "metadata": {
                        "type": "object",
                        "properties": {
                            "depends_on": { "type": "array", "items": { "type": "string" } }
                        },
                        "required": ["depends_on"]
                    }
                },
                "required": ["id", "input_message", "metadata"]
            }
        });

        trace!(prompt = %plan_prompt, "NP2.B: Decomposition plan prompt.");
        let decision_val_b = self
            .llm
            .complete_structured(&plan_prompt, None, schema_b.clone())
            .await
            .map_err(|e| {
                error!(error = %e, "NP2.B: LLM structured call failed during plan generation.");
                ServerError::Internal(format!("LLM error during plan generation: {}", e))
            })?;
        trace!(
            ?decision_val_b,
            "NP2.B: LLM structured response received (plan JSON)."
        );

        match serde_json::from_value::<Vec<SubtaskDefinition>>(decision_val_b.clone()) {
            // Clone decision_val_b for logging on error
            Ok(subtasks) if !subtasks.is_empty() => {
                info!(
                    subtask_count = subtasks.len(),
                    "NP2.B: Successfully parsed decomposition plan."
                );
                Ok(Some(RoutingDecision::Decompose { subtasks }))
            }
            Ok(_) => {
                warn!(
                    ?decision_val_b,
                    "NP2.B: LLM returned empty subtask list. Proceeding without decomposition."
                );
                Ok(None)
            }
            Err(e) => {
                warn!(error = %e, ?decision_val_b, "NP2.B: Failed to parse decomposition plan JSON. Proceeding without decomposition.");
                Ok(None)
            }
        }
    }

    async fn decide_simple_routing(&self, task: &Task) -> Result<RoutingDecision, ServerError> {
        debug!("DP2/DP3: Performing simple routing decision (Local/Remote/Reject + Tool Choice).");
        let history_text = self.format_history(task.history.as_ref());

        if history_text.is_empty() {
            warn!("Extracted conversation history is empty. Falling back to local 'echo'.");
            return Ok(RoutingDecision::Local {
                tool_name: "echo".to_string(),
                params: json!({}),
            });
        }

        // Get memory context if available in metadata
        let memory_tasks = self.extract_rolling_memory(task).await;
        let has_memory = !memory_tasks.is_empty();
        let memory_text = if has_memory {
            self.format_rolling_memory(&memory_tasks, Some(3))
        } else {
            String::new()
        };
        debug!(has_memory = %has_memory, memory_task_count = %memory_tasks.len(), "Memory context prepared");

        // --- DP2: Local vs Remote vs Reject ---
        let local_tools_desc = self.format_tools(false);
        let remote_agents_desc = self.format_agents();

        // Build the routing prompt with memory context if available
        let routing_prompt = if has_memory {
            format!(
                r#"You need to decide whether to handle a task locally using your own tools, delegate it to another available agent, or reject it entirely.

YOUR LOCAL TOOLS:
{}

AVAILABLE REMOTE AGENTS:
{}

AGENT MEMORY OF PREVIOUS OUTGOING REQUESTS:
{}

CONVERSATION HISTORY (User/Agent turns):
{}

Based on the full CONVERSATION HISTORY, AGENT MEMORY, YOUR LOCAL TOOLS, and AVAILABLE REMOTE AGENTS, decide the best course of action for the *latest* user request:

1. Handle it locally if one of YOUR LOCAL TOOLS is suitable (respond with "LOCAL").
   - IMPORTANT: If the latest request matches an internal command like 'connect', 'disconnect', 'list servers', 'session new', 'card', etc., you MUST choose LOCAL execution so the 'execute_command' tool can handle it. Do NOT reject these internal commands based on the history.

2. Delegate to a specific remote agent if it's more appropriate (respond with "REMOTE: [agent-id]").
   - CRITICAL: When delegating, you MUST use EXACTLY the agent ID as listed in the AVAILABLE REMOTE AGENTS section above.
   - The agent-id must match PRECISELY, including capitalization, spaces, and any special characters.
   - Example: If an agent is listed as "Agent Three -- can remember", you must use "REMOTE: Agent Three -- can remember", not "REMOTE: Agent Three" or any other variation.
   - Do not abbreviate, shorten, or modify the agent ID in any way.

3. Reject the task ONLY if it's inappropriate, harmful, impossible, OR if it's an internal command that cannot be handled by the 'execute_command' tool (e.g., ':listen', ':stop', ':quit'). Provide a brief explanation for rejection.

Your response should be exactly one of those formats (LOCAL, REMOTE: agent-id, or REJECT: reason), with no additional text."#,
                local_tools_desc, remote_agents_desc, memory_text, history_text
            )
        } else {
            format!(
                r#"You need to decide whether to handle a task locally using your own tools, delegate it to another available agent, or reject it entirely.

YOUR LOCAL TOOLS:
{}

AVAILABLE REMOTE AGENTS:
{}

CONVERSATION HISTORY (User/Agent turns):
{}

Based on the full CONVERSATION HISTORY, YOUR LOCAL TOOLS, and AVAILABLE REMOTE AGENTS, decide the best course of action for the *latest* user request.

Your response should be a JSON object matching this schema:
{{
  "type": "object",
  "properties": {{
    "decision_type": {{ "type": "string", "enum": ["LOCAL", "REMOTE", "REJECT"] }},
    "agent_id": {{ "type": "string", "description": "Required if decision_type is REMOTE. Must be an exact ID from AVAILABLE REMOTE AGENTS." }},
    "reason": {{ "type": "string", "description": "Required if decision_type is REJECT. A brief explanation." }}
  }},
  "required": ["decision_type"]
}}
Do not add any explanations or text outside the JSON object."#,
                local_tools_desc, remote_agents_desc, history_text
            )
        }; // Correctly end the if/else expression for routing_prompt

        let schema_dp2 = json!({
            "type": "object",
            "properties": {
                "decision_type": { "type": "string", "enum": ["LOCAL", "REMOTE", "REJECT"] },
                "agent_id": { "type": "string" },
                "reason": { "type": "string" }
            },
            "required": ["decision_type"]
        });

        trace!(prompt = %routing_prompt, "DP2: Routing prompt.");

        info!("DP2: Requesting routing decision from LLM.");
        let decision_val = match self
            .llm
            .complete_structured(&routing_prompt, None, schema_dp2.clone())
            .await
        {
            Ok(val) => val,
            Err(e) => {
                error!(error = %e, "DP2: LLM structured routing decision failed. Falling back to local 'llm'.");
                return Ok(RoutingDecision::Local {
                    tool_name: "llm".to_string(),
                    params: json!({"text": history_text}),
                });
            }
        };

        info!(
            ?decision_val,
            "DP2: LLM structured routing decision response received."
        );

        // --- Parse DP2 Decision ---
        match decision_val.get("decision_type").and_then(Value::as_str) {
            Some("LOCAL") => {
                // --- DP3: Choose Local Tool & Parameters ---
                info!("DP2 decided LOCAL. Proceeding to DP3 (Tool Selection).");
                let local_tools_with_params_desc = self.format_tools(true); // Include param hints

                let tool_param_prompt = if has_memory {
                    format!(
                        r#"You have decided to handle the latest request in the following CONVERSATION HISTORY locally:
{}

AGENT MEMORY OF PREVIOUS OUTGOING REQUESTS:
{}

Based on the CONVERSATION HISTORY (especially the latest request), AGENT MEMORY, and the AVAILABLE LOCAL TOOLS listed below, choose the SINGLE most appropriate tool and extract its required parameters.

AVAILABLE LOCAL TOOLS:
{}

Respond with a JSON object matching this schema:
{{
  "type": "object",
  "properties": {{
    "tool_name": {{ "type": "string", "description": "The name of the chosen tool." }},
    "params": {{ "type": "object", "description": "A JSON object containing parameters for the tool. Can be empty {{}} if no params needed." }}
  }},
  "required": ["tool_name", "params"]
}}
CRITICAL INSTRUCTIONS:
1. For agent connection tasks: If the request mentions an agent URL (e.g., "connect to agent at http://localhost:4202"), you MUST FIRST use the 'remember_agent' tool.
2. For internal commands: If the original TASK was a request to perform an internal agent action (like connecting, listing servers, managing sessions), use the 'execute_command' tool.
3. For agent management: ALWAYS use 'remember_agent' for any request that involves remembering, storing, or registering an agent URL.
Do not add any explanations or text outside the JSON object."#,
                        history_text, memory_text, local_tools_with_params_desc
                    )
                } else {
                    format!(
                        r#"You have decided to handle the latest request in the following CONVERSATION HISTORY locally:
{}

Based on the CONVERSATION HISTORY (especially the latest request) and the AVAILABLE LOCAL TOOLS listed below, choose the SINGLE most appropriate tool and extract its required parameters.

AVAILABLE LOCAL TOOLS:
{}

Respond with a JSON object matching this schema:
{{
  "type": "object",
  "properties": {{
    "tool_name": {{ "type": "string", "description": "The name of the chosen tool." }},
    "params": {{ "type": "object", "description": "A JSON object containing parameters for the tool. Can be empty {{}} if no params needed." }}
  }},
  "required": ["tool_name", "params"]
}}
CRITICAL INSTRUCTIONS:
1. For agent connection tasks: If the request mentions an agent URL (e.g., "connect to agent at http://localhost:4202"), you MUST FIRST use the 'remember_agent' tool.
2. For internal commands: If the original TASK was a request to perform an internal agent action (like connecting, listing servers, managing sessions), use the 'execute_command' tool.
3. For agent management: ALWAYS use 'remember_agent' for any request that involves remembering, storing, or registering an agent URL.
Do not add any explanations or text outside the JSON object."#,
                        history_text, local_tools_with_params_desc
                    )
                };

                let schema_dp3 = json!({
                    "type": "object",
                    "properties": {
                        "tool_name": { "type": "string" },
                        "params": { "type": "object" }
                    },
                    "required": ["tool_name", "params"]
                });
                trace!(prompt = %tool_param_prompt, "DP3: Tool/param extraction prompt.");

                info!("DP3: Asking LLM to choose tool and extract parameters.");

                match self
                    .llm
                    .complete_structured(&tool_param_prompt, None, schema_dp3.clone())
                    .await
                {
                    Ok(json_value) => {
                        trace!(?json_value, "DP3: Received tool/param JSON from LLM.");
                        if let (Some(tool_name), Some(params)) = (
                            json_value.get("tool_name").and_then(Value::as_str),
                            json_value.get("params").cloned(), // Cloned as it's a Value
                        ) {
                            if self.enabled_tools.contains(&tool_name.to_string()) {
                                info!(tool_name = %tool_name, ?params, "DP3: Successfully parsed tool and params.");
                                if tool_name == "remember_agent"
                                    && params
                                        .get("agent_base_url")
                                        .and_then(Value::as_str)
                                        .is_none()
                                {
                                    debug!("remember_agent chosen but no URL in params, trying to extract from history.");
                                    if let Some(url) = self.extract_url_from_history(&history_text)
                                    {
                                        debug!(extracted_url = %url, "Found URL in history for remember_agent.");
                                        let enhanced_params = json!({"agent_base_url": url});
                                        return Ok(RoutingDecision::Local {
                                            tool_name: tool_name.to_string(),
                                            params: enhanced_params,
                                        });
                                    }
                                }
                                Ok(RoutingDecision::Local {
                                    tool_name: tool_name.to_string(),
                                    params,
                                })
                            } else {
                                warn!(chosen_tool = %tool_name, enabled_tools = ?self.enabled_tools, "DP3: LLM chose an unknown/disabled tool. Falling back to 'llm'.");
                                Ok(RoutingDecision::Local {
                                    tool_name: "llm".to_string(),
                                    params: json!({"text": history_text}),
                                })
                            }
                        } else {
                            warn!(?json_value, "DP3: LLM JSON missing 'tool_name' or 'params'. Falling back to 'llm'.");
                            Ok(RoutingDecision::Local {
                                tool_name: "llm".to_string(),
                                params: json!({"text": history_text}),
                            })
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "DP3: LLM failed to choose tool/extract params. Falling back to 'llm'.");
                        Ok(RoutingDecision::Local {
                            tool_name: "llm".to_string(),
                            params: json!({"text": history_text}),
                        })
                    }
                }
            }
            Some("REMOTE") => {
                if let Some(agent_id) = decision_val.get("agent_id").and_then(Value::as_str) {
                    info!(remote_agent_id = %agent_id, "DP2 decided REMOTE execution.");
                    // Exact agent ID matching logic (as before)
                    let mut actual_agent_id = None;
                    if self.agent_registry.get(agent_id).is_some() {
                        actual_agent_id = Some(agent_id.to_string());
                    } else {
                        let known_agents = self.agent_registry.list_all_agents();
                        for (known_id, _) in known_agents {
                            if known_id.to_lowercase().contains(&agent_id.to_lowercase())
                                || agent_id.to_lowercase().contains(&known_id.to_lowercase())
                            {
                                info!(requested_agent = %agent_id, matched_agent = %known_id, "Found partial match for agent name");
                                actual_agent_id = Some(known_id);
                                break;
                            }
                        }
                    }
                    if let Some(actual_id) = actual_agent_id {
                        info!(remote_agent_id = %actual_id, "DP2: Remote delegation confirmed.");
                        Ok(RoutingDecision::Remote {
                            agent_id: actual_id,
                        })
                    } else {
                        warn!(remote_agent_id = %agent_id, "DP2: LLM delegated to unknown agent. Falling back to local 'llm'.");
                        Ok(RoutingDecision::Local {
                            tool_name: "llm".to_string(),
                            params: json!({"text": history_text}),
                        })
                    }
                } else {
                    warn!(
                        ?decision_val,
                        "DP2: REMOTE decision missing 'agent_id'. Falling back to local 'llm'."
                    );
                    Ok(RoutingDecision::Local {
                        tool_name: "llm".to_string(),
                        params: json!({"text": history_text}),
                    })
                }
            }
            Some("REJECT") => {
                let reason = decision_val
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or("No reason provided.")
                    .to_string();
                info!(%reason, "DP2 decided REJECT.");
                Ok(RoutingDecision::Reject { reason })
            }
            _ => {
                warn!(?decision_val, "DP2: LLM routing decision_type unclear or missing. Falling back to local 'llm'.");
                Ok(RoutingDecision::Local {
                    tool_name: "llm".to_string(),
                    params: json!({"text": history_text}),
                })
            }
        }
    }

    // DP1: Process follow-up message for tasks with memory context
    #[instrument(skip(self, message), fields(task_id = %task_id))]
    async fn process_follow_up_with_memory(
        &self,
        task_id: &str,
        message: &Message,
        memory_tasks: &[Task],
    ) -> Result<RoutingDecision, ServerError> {
        debug!("Processing follow-up message for task with memory context.");
        trace!(
            ?message,
            memory_task_count = memory_tasks.len(),
            "Follow-up message details."
        );

        // Extract text from the follow-up message
        let follow_up_text = message
            .parts
            .iter()
            .filter_map(|p| match p {
                Part::TextPart(tp) => Some(tp.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Get the task from the appropriate repository
        let task_result = if let Some(repo) = &self.task_repository {
            // Use the provided task repository
            repo.get_task(task_id).await
        } else {
            // In production, this would use another method to get the task
            debug!("No task_repository available, cannot access task details");
            return Ok(RoutingDecision::Local {
                tool_name: "llm".to_string(),
                params: json!({"text": follow_up_text}),
            });
        };

        // Format memory context if available
        let memory_text = if !memory_tasks.is_empty() {
            self.format_rolling_memory(memory_tasks, Some(3))
        } else {
            String::new()
        };
        let has_memory = !memory_text.is_empty();

        if let Ok(Some(task)) = task_result {
            // Check task metadata to determine if it's a remote task coming back to us
            let is_returning_remote_task = if let Some(md) = &task.metadata {
                md.get("delegated_from").is_some()
                    || md.get("remote_agent_id").is_some()
                    || md.get("source_agent_id").is_some()
            } else {
                false
            };

            // Check if this task is in InputRequired state and is returning from another agent
            if task.status.state == TaskState::InputRequired && is_returning_remote_task {
                info!("Task is in InputRequired state and is returning from another agent");

                // Use the LLM to decide if we can handle it ourselves or need human input
                let llm = &self.llm;

                if let Some(history) = &task.history {
                    // Create prompt for the LLM asking it to decide if we can handle this ourselves
                    let history_text = history
                        .iter()
                        .map(|msg| {
                            let role_str = match msg.role {
                                Role::User => "User",
                                Role::Agent => "Agent",
                            };

                            let content = msg
                                .parts
                                .iter()
                                .filter_map(|p| match p {
                                    Part::TextPart(tp) => Some(tp.text.as_str()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n");

                            format!("{}: {}", role_str, content)
                        })
                        .collect::<Vec<_>>()
                        .join("\n\n");

                    // Get the current status message which often describes why input is required
                    let status_message = if let Some(msg) = &task.status.message {
                        msg.parts
                            .iter()
                            .filter_map(|p| match p {
                                Part::TextPart(tp) => Some(tp.text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    } else {
                        "No status message available".to_string()
                    };

                    // Build the decision prompt with memory context if available
                    let decision_prompt = if has_memory {
                        format!(
                            r#"You are assisting with handling a returning delegated task that requires additional input.

AGENT MEMORY OF PREVIOUS OUTGOING REQUESTS:
{}

TASK HISTORY:
{}

REASON INPUT IS REQUIRED:
{}

FOLLOW-UP MESSAGE/INPUT FROM USER:
{}

You need to decide how to handle this situation:
1. HANDLE_DIRECTLY - If you have enough information to answer the question or address the issue directly
2. NEED_HUMAN_INPUT - If the input required is complex/specific and needs human feedback

Consider:
- Do you understand what input is needed?
- Is the follow-up message clear enough to proceed?
- Could this require specific expertise or personal preferences that only the human would know?

Respond with a JSON object matching this schema:
{{
  "type": "object",
  "properties": {{
    "decision_type": {{ "type": "string", "enum": ["HANDLE_DIRECTLY", "NEED_HUMAN_INPUT"] }}
  }},
  "required": ["decision_type"]
}}
Do not add any explanations or text outside the JSON object."#,
                            memory_text, history_text, status_message, follow_up_text
                        )
                    } else {
                        format!(
                            r#"You are assisting with handling a returning delegated task that requires additional input.

TASK HISTORY:
{}

REASON INPUT IS REQUIRED:
{}

FOLLOW-UP MESSAGE/INPUT FROM USER:
{}

You need to decide how to handle this situation.
Consider:
- Do you understand what input is needed?
- Is the follow-up message clear enough to proceed?
- Could this require specific expertise or personal preferences that only the human would know?

Respond with a JSON object matching this schema:
{{
  "type": "object",
  "properties": {{
    "decision_type": {{ "type": "string", "enum": ["HANDLE_DIRECTLY", "NEED_HUMAN_INPUT"] }}
  }},
  "required": ["decision_type"]
}}
Do not add any explanations or text outside the JSON object."#,
                            history_text, status_message, follow_up_text
                        )
                    };

                    let schema_follow_up = json!({
                        "type": "object",
                        "properties": {
                            "decision_type": { "type": "string", "enum": ["HANDLE_DIRECTLY", "NEED_HUMAN_INPUT"] }
                        },
                        "required": ["decision_type"]
                    });

                    let decision_val_follow_up = match llm
                        .complete_structured(&decision_prompt, None, schema_follow_up.clone())
                        .await
                    {
                        Ok(val) => val,
                        Err(e) => {
                            warn!("LLM structured decision failed for follow-up: {}, defaulting to NEED_HUMAN_INPUT", e);
                            json!({"decision_type": "NEED_HUMAN_INPUT"})
                        }
                    };

                    match decision_val_follow_up
                        .get("decision_type")
                        .and_then(Value::as_str)
                    {
                        Some("HANDLE_DIRECTLY") => {
                            info!("LLM decided to handle the InputRequired task directly");
                            return Ok(RoutingDecision::Local {
                                tool_name: "llm".to_string(),
                                params: json!({"text": follow_up_text}),
                            });
                        }
                        Some("NEED_HUMAN_INPUT") | _ => {
                            // Default to human input
                            info!("LLM decided to request human input for the task (or decision was unclear).");
                            return Ok(RoutingDecision::Local {
                                tool_name: "human_input".to_string(),
                                params: json!({
                                    "text": follow_up_text,
                                    "require_human_input": true,
                                    "prompt": status_message
                                }),
                            });
                        }
                    }
                }
            }
        }

        // Default case: Handle locally with LLM tool but include memory context
        info!("DP1: Routing follow-up to LOCAL execution using 'llm' tool with memory context.");

        // Set up parameters to include memory context if available
        let mut params = json!({
            "text": follow_up_text
        });

        if has_memory {
            if let Value::Object(ref mut map) = params {
                map.insert("memory_context".to_string(), Value::String(memory_text));
            }
        }

        Ok(RoutingDecision::Local {
            tool_name: "llm".to_string(),
            params,
        })
    }
}

// --- LlmTaskRouterTrait Implementation ---

#[async_trait]
impl LlmTaskRouterTrait for BidirectionalTaskRouter {
    #[instrument(skip(self, params), fields(task_id = %params.id))]
    async fn route_task(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError> {
        debug!("Routing task based on TaskSendParams.");
        trace!(?params, "TaskSendParams for routing.");

        // We need a Task object to make the decision based on history/message.
        // Construct a temporary Task from TaskSendParams.
        trace!("Constructing temporary Task object for routing decision.");
        let task = Task {
            id: params.id.clone(),
            status: TaskStatus {
                // Default status for routing decision
                state: TaskState::Submitted,
                timestamp: Some(Utc::now()),
                message: None, // Status message is usually set later
            },
            // Use the incoming message as the start of history for decision making
            history: Some(vec![params.message.clone()]),
            artifacts: None,
            metadata: params.metadata.clone(),
            session_id: params.session_id.clone(),
        };
        trace!(?task, "Temporary Task object created.");

        // --- Execute Decision Pipeline ---

        // NP1: Check Clarification
        if let Some(clarification_decision) = self.check_clarification(&task).await? {
            return Ok(clarification_decision);
        }
        // If clear or disabled, proceed...

        // NP2: Check Decomposition
        if let Some(decomposition_decision) = self.check_decomposition(&task).await? {
            // TODO: NP2.C - Resource Allocation per Sub-task
            // This would involve iterating `decomposition_decision.subtasks`,
            // calling `decide_simple_routing` for each, and storing the result
            // within each `SubtaskDefinition.routing_decision`.
            // TaskService would then need to execute this plan.
            // For now, just return the plan structure.
            return Ok(decomposition_decision);
        }
        // If no decomposition needed or disabled, proceed...

        // DP2/DP3: Simple Routing
        debug!("Proceeding to simple routing (Local/Remote/Reject + Tool Choice).");
        let final_decision = self.decide_simple_routing(&task).await?;
        debug!(?final_decision, "Final routing decision made.");
        Ok(final_decision)
    }

    // DP1: Process follow-up message for tasks, including enhanced InputRequired handling
    #[instrument(skip(self, message), fields(task_id = %task_id))]
    async fn process_follow_up(
        &self,
        task_id: &str,
        message: &Message,
    ) -> Result<RoutingDecision, ServerError> {
        debug!("Processing follow-up message for task.");
        trace!(?message, "Follow-up message details.");

        // Extract memory tasks from the task if available
        let mut memory_tasks = Vec::new();

        if let Some(repo) = &self.task_repository {
            if let Ok(Some(task)) = repo.get_task(task_id).await {
                if let Some(metadata) = &task.metadata {
                    if let Some(memory_ids_value) = metadata.get("_rolling_memory") {
                        if let Some(memory_ids) = memory_ids_value.as_array() {
                            for id_value in memory_ids {
                                if let Some(id_str) = id_value.as_str() {
                                    if let Ok(Some(memory_task)) = repo.get_task(id_str).await {
                                        memory_tasks.push(memory_task);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        debug!(memory_task_count = %memory_tasks.len(), "Retrieved memory tasks for follow-up");

        // Use the enhanced process_follow_up with memory context
        self.process_follow_up_with_memory(task_id, message, memory_tasks.as_slice())
            .await
    }

    // --- Trait Methods Implementation ---

    // `decide` is now the main entry point that orchestrates the pipeline
    #[instrument(skip(self, params), fields(task_id = %params.id))]
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError> {
        debug!("'decide' method called, initiating routing pipeline.");
        // Delegate to route_task, which now contains the full pipeline logic
        self.route_task(params).await
    }

    // `should_decompose` now uses the LLM check if enabled
    #[instrument(skip(self, params), fields(task_id = %params.id))]
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, ServerError> {
        debug!("'should_decompose' method called.");
        if !self.experimental_decomposition {
            trace!("Decomposition disabled by config, returning false.");
            return Ok(false);
        }
        // Construct temporary task to call check_decomposition helper
        let task = Task {
            id: params.id.clone(),
            status: TaskStatus {
                state: TaskState::Submitted,
                timestamp: Some(Utc::now()),
                message: None,
            },
            history: Some(vec![params.message.clone()]),
            artifacts: None,
            metadata: params.metadata.clone(),
            session_id: params.session_id.clone(),
        };
        trace!(
            ?task,
            "Temporary Task object created for should_decompose check."
        );
        match self.check_decomposition(&task).await? {
            Some(RoutingDecision::Decompose { .. }) => {
                debug!("Decomposition check returned YES.");
                Ok(true)
            }
            _ => {
                debug!("Decomposition check returned NO or None.");
                Ok(false)
            }
        }
    }

    // `decompose_task` now uses the LLM plan generation if enabled
    #[instrument(skip(self, params), fields(task_id = %params.id))]
    async fn decompose_task(
        &self,
        params: &TaskSendParams,
    ) -> Result<Vec<SubtaskDefinition>, ServerError> {
        debug!("'decompose_task' method called.");
        if !self.experimental_decomposition {
            trace!("Decomposition disabled by config, returning empty list.");
            return Ok(Vec::new());
        }
        // Construct temporary task to call check_decomposition helper
        let task = Task {
            id: params.id.clone(),
            status: TaskStatus {
                state: TaskState::Submitted,
                timestamp: Some(Utc::now()),
                message: None,
            },
            history: Some(vec![params.message.clone()]),
            artifacts: None,
            metadata: params.metadata.clone(),
            session_id: params.session_id.clone(),
        };
        trace!(
            ?task,
            "Temporary Task object created for decompose_task execution."
        );
        match self.check_decomposition(&task).await? {
            Some(RoutingDecision::Decompose { subtasks }) => {
                info!(
                    subtask_count = subtasks.len(),
                    "Returning decomposition plan."
                );
                Ok(subtasks)
            }
            _ => {
                warn!("Decomposition check did not yield a plan. Returning empty list.");
                Ok(Vec::new()) // Return empty if check didn't result in decomposition
            }
        }
    }
}
