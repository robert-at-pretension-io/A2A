//! Bidirectional Task Router Implementation

use crate::bidirectional::llm_client::LlmClient;
use crate::server::agent_registry::AgentRegistry;
use crate::server::error::ServerError;
use crate::server::repositories::task_repository::TaskRepository;
use crate::server::task_router::{LlmTaskRouterTrait, RoutingDecision, SubtaskDefinition};
use crate::types::{Message, Part, Role, Task, TaskSendParams, TaskState, TaskStatus};

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error, info, instrument, trace, warn};

// Define a struct that implements the server's TaskRouterTrait
#[derive(Clone)]
pub struct BidirectionalTaskRouter {
    llm: Arc<dyn LlmClient>,
    agent_registry: Arc<AgentRegistry>, // Use the canonical registry
    enabled_tools: Arc<Vec<String>>,
    task_repository: Option<Arc<dyn TaskRepository + Send + Sync>>, // Add task repository field
}

impl BidirectionalTaskRouter {
    pub fn new(
        llm: Arc<dyn LlmClient>,
        agent_registry: Arc<AgentRegistry>, // Use canonical registry
        enabled_tools: Arc<Vec<String>>,
        task_repository: Option<Arc<dyn TaskRepository + Send + Sync>>, // Add task repository parameter
    ) -> Self {
        // Ensure "echo" is always considered enabled internally for fallback, even if not in config
        let mut tools = enabled_tools.as_ref().clone();
        if !tools.contains(&"echo".to_string()) {
            tools.push("echo".to_string());
        }
        Self {
            llm,
            agent_registry, // Store the registry
            enabled_tools: Arc::new(tools),
            task_repository, // Store the task repository
        }
    }

    // Helper to perform the actual routing logic AND tool selection
    // Now returns RoutingDecision directly
    #[instrument(skip(self, task), fields(task_id = %task.id))]
    pub async fn decide_execution_mode(&self, task: &Task) -> Result<RoutingDecision, ServerError> {
        debug!("Deciding execution mode for task.");
        trace!(?task, "Task details for routing decision.");

        // --- Extract Full Conversation History ---
        trace!("Extracting full conversation history for LLM prompt.");
        let conversation_history_text = task.history.as_ref()
            .map(|history| {
                history.iter().map(|message| {
                    let role_str = match message.role {
                        Role::User => "User",
                        Role::Agent => "Agent",
                        // Handle other roles if they exist
                    };
                    let content = message.parts.iter()
                        .filter_map(|part| match part {
                            Part::TextPart(tp) => Some(tp.text.as_str()),
                            // Optionally represent other part types
                            Part::FilePart(_) => Some("[File Content]"),
                            Part::DataPart(_) => Some("[Structured Data]"),
                        })
                        .collect::<Vec<_>>()
                        .join(" "); // Join parts with space
                    format!("{}: {}", role_str, content)
                }).collect::<Vec<_>>().join("\n") // Join messages with newline
            })
            .unwrap_or_else(|| {
                warn!("Task history is empty or None. Cannot extract conversation.");
                "".to_string() // Return empty string if no history
            });
        trace!(conversation_history = %conversation_history_text, "Extracted conversation history text.");

        if conversation_history_text.is_empty() {
            warn!("Extracted conversation history is empty. Falling back to local 'echo'.");
            return Ok(RoutingDecision::Local { tool_name: "echo".to_string(), params: json!({}) });
        }
        // --- End History Extraction ---

        // Get the list of available agents from the canonical registry
        debug!("Fetching available agents from AgentRegistry for routing prompt.");
        let available_agents = self.agent_registry.list_all_agents(); // Returns Vec<(String, AgentCard)>
        trace!(count = available_agents.len(), "Found available agents in registry.");
        let agent_descriptions = available_agents.iter()
            .map(|(agent_id, card)| { // Use agent_id and card directly from registry list
                trace!(%agent_id, agent_name = %card.name, "Formatting agent description for prompt.");
                // Construct capabilities string manually
                let mut caps = Vec::new();
                if card.capabilities.push_notifications { caps.push("pushNotifications"); }
                if card.capabilities.state_transition_history { caps.push("stateTransitionHistory"); }
                if card.capabilities.streaming { caps.push("streaming"); }
                // Add other capabilities fields if they exist in AgentCapabilities struct

                format!("ID: {}\nName: {}\nDescription: {}\nCapabilities: {}",
                    agent_id, // Use the ID from the registry list
                    card.name.as_str(),
                    card.description.as_deref().unwrap_or(""), // Use deref for Option<String>
                    caps.join(", "))
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        trace!(agent_descriptions = %agent_descriptions, "Formatted agent descriptions for prompt.");

        // --- Add Local Tool Descriptions to Prompt ---
        debug!("Fetching descriptions of locally enabled tools for routing prompt.");
        let local_tool_descriptions = self.enabled_tools.iter()
            .map(|tool_name| {
                let description = match tool_name.as_str() {
                    "llm" => "General purpose LLM request. Good for questions, generation, analysis if no specific tool fits.",
                    "summarize" => "Summarizes the input text.",
                    "list_agents" => "Lists known agents registered with this agent.",
                    "remember_agent" => "Stores information about another agent given its URL.",
                    "execute_command" => "Executes internal agent commands (like connect, disconnect, servers, session new, card). Use for requests matching these commands. Expects {\"command\": \"command_name\", \"args\": \"arguments_string\"}",
                    "echo" => "Simple echo tool for testing.",
                    _ => "A custom tool.",
                };
                format!("- {}: {}", tool_name, description)
            })
            .collect::<Vec<_>>()
            .join("\n");
        trace!(local_tool_descriptions = %local_tool_descriptions, "Formatted local tool descriptions for routing prompt.");
        // --- End Add Local Tool Descriptions ---


        // Build a prompt for the LLM to decide routing
        debug!("Building routing prompt for LLM.");
        let routing_prompt = format!(r#"
You need to decide whether to handle a task locally using your own tools, delegate it to another available agent, or reject it entirely.

YOUR LOCAL TOOLS:
{}

AVAILABLE REMOTE AGENTS:
{}

CONVERSATION HISTORY (User/Agent turns):
{}

Based on the full CONVERSATION HISTORY, YOUR LOCAL TOOLS, and AVAILABLE REMOTE AGENTS, decide the best course of action for the *latest* user request:
1. Handle it locally if one of YOUR LOCAL TOOLS is suitable (respond with "LOCAL").
   - IMPORTANT: If the latest request matches an internal command like 'connect', 'disconnect', 'list servers', 'session new', 'card', etc., you MUST choose LOCAL execution so the 'execute_command' tool can handle it. Do NOT reject these internal commands based on the history.
2. Delegate to a specific remote agent if it's more appropriate (respond with "REMOTE: [agent-id]"). Choose the most relevant agent if multiple are available.
3. Reject the task ONLY if it's inappropriate, harmful, impossible, OR if it's an internal command that cannot be handled by the 'execute_command' tool (e.g., ':listen', ':stop', ':quit'). Provide a brief explanation for rejection.

Your response should be exactly one of those formats (LOCAL, REMOTE: agent-id, or REJECT: reason), with no additional text.
"#, local_tool_descriptions, agent_descriptions, conversation_history_text); // Use full history
        trace!(routing_prompt = %routing_prompt, "Constructed routing prompt.");

        info!("Requesting routing decision from LLM.");
        // Get the routing decision from the LLM
        let decision_result = self.llm.complete(&routing_prompt).await;
        trace!(?decision_result, "LLM routing decision result received.");

        // Map anyhow::Error from LLM to ServerError::Internal
        let decision = match decision_result {
            Ok(d) => {
                let trimmed_decision = d.trim().to_string();
                info!(llm_response = %trimmed_decision, "LLM routing decision response received.");
                trimmed_decision
            }
            Err(e) => {
                error!(error = %e, "LLM routing decision failed. Falling back to local 'echo'.");
                // Fallback to local echo on LLM error
                // Use tool_name and add default params
                return Ok(RoutingDecision::Local { tool_name: "echo".to_string(), params: json!({}) });
            }
        };
        trace!(decision = %decision, "Raw LLM decision text.");

        // Parse the decision more robustly - check prefixes
        if decision.starts_with("LOCAL") {
            info!("LLM decided LOCAL execution. Proceeding to tool selection and parameter extraction.");

            // --- Combined LLM Tool Selection & Parameter Extraction ---
            debug!("Fetching descriptions of locally enabled tools for combined prompt.");
            let local_tool_descriptions = self.enabled_tools.iter()
                .map(|tool_name| {
                    // Provide descriptions and expected parameters for better LLM guidance
                    let description = match tool_name.as_str() {
                        "llm" => "General purpose LLM request. Good for questions, generation, analysis if no specific tool fits. Expects {\"text\": \"...\"}",
                        "summarize" => "Summarizes the input text. Expects {\"text\": \"...\"}",
                        "list_agents" => "Lists known agents. Use only if the task is explicitly about listing agents. Expects {} or {\"format\": \"simple\" | \"detailed\"}",
                        "remember_agent" => "Stores information about another agent given its URL. Expects {\"agent_base_url\": \"http://...\"}",
                        "execute_command" => "Executes internal agent commands (like connect, disconnect, servers, session new, card). Use for requests matching these commands. Expects {\"command\": \"command_name\", \"args\": \"arguments_string\"}", // <-- Add description
                        "echo" => "Simple echo tool for testing. Use only if the task is explicitly to echo. Expects {\"text\": \"...\"}",
                        _ => "A custom tool with potentially unknown parameters.",
                    };
                    format!("- {}: {}", tool_name, description)
                })
                .collect::<Vec<_>>()
                .join("\n");
            trace!(local_tool_descriptions = %local_tool_descriptions, "Formatted local tool descriptions for combined prompt.");

            let tool_param_prompt = format!(
r#"You have decided to handle the latest request in the following CONVERSATION HISTORY locally:
{}

Based on the CONVERSATION HISTORY (especially the latest request) and the AVAILABLE LOCAL TOOLS listed below, choose the SINGLE most appropriate tool and extract its required parameters.

AVAILABLE LOCAL TOOLS:
{}

Respond ONLY with a valid JSON object containing the chosen tool's name and its parameters. The JSON object MUST have the following structure:
{{
  "tool_name": "<chosen_tool_name>",
  "params": {{ <parameters_object> }}
}}

CRITICAL: If the original TASK was a request to perform an internal agent action (like connecting, listing servers, managing sessions), you MUST select the 'execute_command' tool and extract the command name and arguments into the 'params' object (e.g., {{"command": "connect", "args": "http://..."}}). Do NOT select the 'llm' tool for these internal commands.

Examples:
- For a task like "remember agent at http://foo.com": {{"tool_name": "remember_agent", "params": {{"agent_base_url": "http://foo.com"}}}}
- For a task like "list known agents simply": {{"tool_name": "list_agents", "params": {{"format": "simple"}}}}
- For a task like "echo hello": {{"tool_name": "echo", "params": {{"text": "hello"}}}}
- For a task like "connect to http://bar.com": {{"tool_name": "execute_command", "params": {{"command": "connect", "args": "http://bar.com"}}}} // <-- Add example
- For a task like "list servers": {{"tool_name": "execute_command", "params": {{"command": "servers", "args": ""}}}} // <-- Add example
- For a task like "start a new session": {{"tool_name": "execute_command", "params": {{"command": "session", "args": "new"}}}} // <-- Add example
- For a general question: {{"tool_name": "llm", "params": {{"text": "original question text..."}}}}
- If no specific parameters are needed for the chosen tool (like list_agents with default format): {{"tool_name": "list_agents", "params": {{}}}}

Ensure the 'params' value is always a JSON object (even if empty: {{}})."#,
                conversation_history_text, // Use full history
                local_tool_descriptions
            );
            trace!(tool_param_prompt = %tool_param_prompt, "Constructed combined tool/param extraction prompt.");

            info!("Asking LLM to choose tool and extract parameters.");
            let tool_param_result = self.llm.complete(&tool_param_prompt).await;

            match tool_param_result {
                Ok(json_str_raw) => {
                    let json_str = json_str_raw.trim();
                    trace!(raw_json = %json_str_raw, trimmed_json = %json_str, "Received tool/param JSON string from LLM.");
                    // Attempt to parse the JSON response
                    match serde_json::from_str::<Value>(json_str) {
                        Ok(json_value) => {
                            if let (Some(tool_name), Some(params)) = (
                                json_value.get("tool_name").and_then(Value::as_str),
                                json_value.get("params").cloned() // Clone the params Value
                            ) {
                                // Validate the chosen tool name
                                if self.enabled_tools.contains(&tool_name.to_string()) {
                                    info!(tool_name = %tool_name, ?params, "Successfully parsed tool and params from LLM response.");
                                    Ok(RoutingDecision::Local { tool_name: tool_name.to_string(), params })
                                } else {
                                    warn!(chosen_tool = %tool_name, enabled_tools = ?self.enabled_tools, "LLM chose an unknown/disabled tool in JSON response. Falling back to 'llm' tool with full history.");
                                    Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": conversation_history_text}) }) // Use history
                                }
                            } else {
                                warn!(json_response = %json_str, "LLM returned valid JSON but missing 'tool_name' or 'params'. Falling back to 'llm' tool with full history.");
                                Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": conversation_history_text}) }) // Use history
                            }
                        },
                        Err(e) => {
                            warn!(error = %e, json_response = %json_str, "LLM returned invalid JSON for tool/params. Falling back to 'llm' tool with full history.");
                            Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": conversation_history_text}) }) // Use history
                        }
                    }
                },
                Err(e) => {
                    warn!(error = %e, "LLM failed to choose tool/extract params. Falling back to 'llm' tool with full history.");
                    Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": conversation_history_text}) }) // Use history
                }
            }
            // --- End Combined LLM Tool Selection & Parameter Extraction ---

        } else if decision.starts_with("REMOTE: ") {
            let agent_id = decision.strip_prefix("REMOTE: ").unwrap().trim().to_string();
            info!(remote_agent_id = %agent_id, "LLM decided REMOTE execution.");

            // Verify the agent exists in the canonical registry
            trace!(remote_agent_id = %agent_id, "Verifying remote agent existence in AgentRegistry.");
            if self.agent_registry.get(&agent_id).is_none() { // Check canonical registry
                 warn!(remote_agent_id = %agent_id, "LLM decided to delegate to unknown agent (not in registry), falling back to local execution with 'llm' tool and full history.");
                 // Fall back to local if agent not found, using llm tool with full history
                 Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": conversation_history_text}) }) // Use history
            } else {
                 info!(remote_agent_id = %agent_id, "Routing decision: Remote delegation confirmed.");
                 Ok(RoutingDecision::Remote { agent_id })
            }
        } else if decision.starts_with("REJECT: ") {
            let reason = decision.strip_prefix("REJECT: ").unwrap().trim().to_string();
            info!(reason = %reason, "LLM decided to REJECT the task.");
            Ok(RoutingDecision::Reject { reason })
        } else {
            warn!(llm_decision = %decision, "LLM routing decision was unclear, falling back to local execution with 'llm' tool and full history.");
            // Default to local llm tool if the decision isn't clear, using full history
            Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": conversation_history_text}) }) // Use history
        }
    }
}

#[async_trait]
impl LlmTaskRouterTrait for BidirectionalTaskRouter {
    // Match the trait signature: takes TaskSendParams, returns Result<RoutingDecision, ServerError>
    // REMOVED #[instrument(skip(self, params), fields(task_id = %params.id))]
    async fn route_task(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError> {
        debug!("Routing task based on TaskSendParams.");
        trace!(?params, "TaskSendParams for routing.");
        // We need a Task object to make the decision based on history/message.
        // Construct a temporary Task from TaskSendParams.
        trace!("Constructing temporary Task object for routing decision.");
        let task = Task {
            id: params.id.clone(),
            status: TaskStatus { // Default status for routing decision
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

        // Call the updated decide_execution_mode which now returns RoutingDecision
        debug!("Delegating routing decision to decide_execution_mode.");
        let decision = self.decide_execution_mode(&task).await;
        debug!(?decision, "Routing decision made.");
        decision
    }

    // Process follow-up message for tasks, including enhanced InputRequired handling
    #[instrument(skip(self, message), fields(task_id = %task_id))]
    async fn process_follow_up(&self, task_id: &str, message: &Message) -> Result<RoutingDecision, ServerError> {
        debug!("Processing follow-up message for task.");
        trace!(?message, "Follow-up message details.");

        // Extract text from the follow-up message
        let follow_up_text = message.parts.iter()
            .filter_map(|p| match p {
                Part::TextPart(tp) => Some(tp.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Get the task from the appropriate repository
        let task_result = if let Some(repo) = &self.task_repository {
            // Use the provided task repository (mainly for tests)
            repo.get_task(task_id).await
        } else {
            // In production, this would use another method to get the task
            debug!("No task_repository available, cannot access task details");
            return Ok(RoutingDecision::Local {
                tool_name: "llm".to_string(),
                params: json!({"text": follow_up_text})
            });
        };

        if let Ok(Some(task)) = task_result {
            // Check task metadata to determine if it's a remote task coming back to us
            let is_returning_remote_task = if let Some(md) = &task.metadata {
                md.get("delegated_from").is_some() ||
                md.get("remote_agent_id").is_some() ||
                md.get("source_agent_id").is_some()
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
                    let history_text = history.iter()
                        .map(|msg| {
                            let role_str = match msg.role {
                                Role::User => "User",
                                Role::Agent => "Agent",
                            };

                            let content = msg.parts.iter()
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
                        msg.parts.iter()
                            .filter_map(|p| match p {
                                Part::TextPart(tp) => Some(tp.text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                    } else {
                        "No status message available".to_string()
                    };

                    // Build the decision prompt
                    let decision_prompt = format!(
                        r#"You are assisting with handling a returning delegated task that requires additional input.

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

Respond with EXACTLY ONE of these options: HANDLE_DIRECTLY or NEED_HUMAN_INPUT"#,
                        history_text, status_message, follow_up_text
                    );

                    let decision = match llm.complete(&decision_prompt).await {
                        Ok(decision) => decision.trim().to_uppercase(),
                        Err(e) => {
                            // LLM failed, default to needing human input
                            warn!("LLM decision failed: {}, defaulting to needing human input", e);
                            "NEED_HUMAN_INPUT".to_string()
                        }
                    };

                    if decision.contains("HANDLE_DIRECTLY") {
                        info!("LLM decided to handle the InputRequired task directly");
                        // Pass to local LLM tool for handling
                        return Ok(RoutingDecision::Local {
                            tool_name: "llm".to_string(),
                            params: json!({"text": follow_up_text})
                        });
                    } else if decision.contains("NEED_HUMAN_INPUT") { // Removed the "|| true" to prevent always taking this branch
                        info!("LLM decided to request human input for the task");
                        // This is a special case - we'll modify the returned decision to flag it as needing human input
                        // The ToolExecutor will then handle this special case differently
                        return Ok(RoutingDecision::Local {
                            tool_name: "human_input".to_string(),
                            params: json!({
                                "text": follow_up_text,
                                "require_human_input": true,
                                "prompt": status_message
                            })
                        });
                    } else {
                        // Default to human input if the decision isn't clear
                        info!("LLM decision not clear, defaulting to human input");
                        return Ok(RoutingDecision::Local {
                            tool_name: "human_input".to_string(),
                            params: json!({
                                "text": follow_up_text,
                                "require_human_input": true,
                                "prompt": status_message
                            })
                        });
                    }
                }
            }
        }

        // Default case: Handle locally with LLM tool
        info!("Routing follow-up to LOCAL execution using 'llm' tool.");
        Ok(RoutingDecision::Local {
            tool_name: "llm".to_string(),
            params: json!({"text": follow_up_text})
        })
    }

    // Implement the decide method required by LlmTaskRouterTrait
    // This method is now effectively the same as route_task, as the full decision happens there.
    #[instrument(skip(self, params), fields(task_id = %params.id))]
    async fn decide(&self, params: &TaskSendParams) -> Result<RoutingDecision, ServerError> {
        debug!("Decide method called, delegating to route_task.");
        // Delegate to route_task, which now handles the full decision including tool choice
        self.route_task(params).await
    }

    // Implement should_decompose method required by LlmTaskRouterTrait
    #[instrument(skip(self, params), fields(task_id = %params.id))]
    async fn should_decompose(&self, params: &TaskSendParams) -> Result<bool, ServerError> {
        debug!("Checking if task should be decomposed.");
        trace!(?params, "Task parameters for decomposition check.");
        // Simple implementation that never decomposes tasks
        let should = false;
        debug!(should_decompose = %should, "Decomposition decision.");
        Ok(should)
    }

    // Implement decompose_task method required by LlmTaskRouterTrait
    #[instrument(skip(self, params), fields(task_id = %params.id))]
    async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, ServerError> {
        debug!("Decomposing task (currently no-op).");
        trace!(?params, "Task parameters for decomposition.");
        // Simple implementation that returns an empty list (no decomposition)
        let subtasks = Vec::new();
        debug!("Returning empty subtask list.");
        Ok(subtasks)
    }
}
