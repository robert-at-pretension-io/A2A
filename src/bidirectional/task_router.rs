//! Bidirectional Task Router Implementation

use crate::bidirectional::llm_client::LlmClient;
use crate::server::agent_registry::AgentRegistry;
use crate::server::error::ServerError;
use crate::server::repositories::task_repository::TaskRepository;
use crate::bidirectional::config::BidirectionalAgentConfig; // Import config
use crate::server::task_router::{LlmTaskRouterTrait, RoutingDecision, SubtaskDefinition};
use crate::types::{Message, Part, Role, Task, TaskSendParams, TaskState, TaskStatus};

use anyhow::{anyhow, Result}; // Add anyhow
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, error, info, instrument, trace, warn};

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
            tools.push("echo".to_string());
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
        history.map(|h| {
            h.iter().map(|message| {
                let role_str = match message.role {
                    Role::User => "User",
                    Role::Agent => "Agent",
                };
                let content = message.parts.iter()
                    .filter_map(|part| match part {
                        Part::TextPart(tp) => Some(tp.text.as_str()),
                        Part::FilePart(_) => Some("[File Content]"),
                        Part::DataPart(_) => Some("[Structured Data]"),
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("{}: {}", role_str, content)
            }).collect::<Vec<_>>().join("\n")
        }).unwrap_or_else(|| "".to_string())
    }

    // Helper to format available agents for prompts
    fn format_agents(&self) -> String {
        let available_agents = self.agent_registry.list_all_agents();
        available_agents.iter()
            .map(|(agent_id, card)| {
                let mut caps = Vec::new();
                if card.capabilities.push_notifications { caps.push("pushNotifications"); }
                if card.capabilities.state_transition_history { caps.push("stateTransitionHistory"); }
                if card.capabilities.streaming { caps.push("streaming"); }
                format!("ID: {}\nName: {}\nDescription: {}\nCapabilities: {}",
                        agent_id,
                        card.name.as_str(),
                        card.description.as_deref().unwrap_or(""),
                        caps.join(", "))
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    // Helper to format local tools for prompts
    fn format_tools(&self, include_params: bool) -> String {
        self.enabled_tools.iter()
            .map(|tool_name| {
                let (description, params_hint) = match tool_name.as_str() {
                    "llm" => ("General purpose LLM request. Good for questions, generation, analysis if no specific tool fits.", r#"{"text": "..."}"#),
                    "summarize" => ("Summarizes the input text.", r#"{"text": "..."}"#),
                    "list_agents" => ("Lists known agents. Use only if the task is explicitly about listing agents.", r#"{} or {"format": "simple" | "detailed"}"#),
                    "remember_agent" => ("Stores information about another agent given its URL.", r#"{"agent_base_url": "http://..."}"#),
                    "execute_command" => ("Executes internal agent commands (like connect, disconnect, servers, session new, card). Use for requests matching these commands.", r#"{"command": "command_name", "args": "arguments_string"}"#),
                    "echo" => ("Simple echo tool for testing. Use only if the task is explicitly to echo.", r#"{"text": "..."}"#),
                    _ => ("A custom tool.", r#"{...}"#), // Generic hint for unknown tools
                };
                if include_params {
                    format!("- {}: {} Expects {}", tool_name, description, params_hint)
                } else {
                    format!("- {}: {}", tool_name, description)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }


    // NP1: Check if the request needs clarification
    #[instrument(skip(self, task), fields(task_id = %task.id))]
    async fn check_clarification(&self, task: &Task) -> Result<Option<RoutingDecision>, ServerError> {
        if !self.experimental_clarification {
            trace!("Skipping clarification check (disabled by config).");
            return Ok(None); // Skip if disabled
        }

        debug!("NP1: Checking if task requires clarification.");
        let history_text = self.format_history(task.history.as_ref());
        let latest_request = task.history.as_ref()
            .and_then(|h| h.last())
            .map(|m| m.parts.iter().filter_map(|p| match p {
                Part::TextPart(tp) => Some(tp.text.as_str()), _ => None
            }).collect::<Vec<_>>().join(" "))
            .unwrap_or_else(|| "".to_string());

        if latest_request.is_empty() {
            warn!("Latest request text is empty, cannot check clarification.");
            return Ok(None); // Cannot proceed without text
        }

        let prompt = format!(
r#"SYSTEM:
You are an autonomous AI agent preparing to process a user request.
Your first task is to judge whether the request is specific and complete.

CONVERSATION_HISTORY:
{}

LATEST_REQUEST:
“{}”

TASK:
1. If the request is sufficiently specific, respond exactly:
   CLARITY: CLEAR
2. If clarification is needed, respond using BOTH lines:
   CLARITY: NEEDS_CLARIFY
   QUESTION: "<single sentence question for the human>"

Rules:
• Do not add anything else.
• If you choose NEEDS_CLARIFY your question MUST be answerable in ≤ 1 sentence."#,
            history_text, latest_request
        );

        trace!(prompt = %prompt, "NP1: Clarity check prompt.");
        let llm_response = self.llm.complete(&prompt).await.map_err(|e| {
            error!(error = %e, "NP1: LLM call failed during clarification check.");
            ServerError::Internal(format!("LLM error during clarification: {}", e))
        })?;
        trace!(llm_response = %llm_response, "NP1: LLM response received.");

        if llm_response.contains("CLARITY: NEEDS_CLARIFY") {
            if let Some(question_line) = llm_response.lines().find(|line| line.starts_with("QUESTION:")) {
                 let question = question_line.strip_prefix("QUESTION:").unwrap_or("").trim().trim_matches('"').to_string();
                 if !question.is_empty() {
                    info!(clarification_question = %question, "NP1: Task needs clarification.");
                    return Ok(Some(RoutingDecision::NeedsClarification { question }));
                 } else {
                    warn!(llm_response = %llm_response, "NP1: LLM indicated clarification needed but question was empty/malformed. Proceeding without clarification.");
                 }
            } else {
                 warn!(llm_response = %llm_response, "NP1: LLM indicated clarification needed but QUESTION line missing. Proceeding without clarification.");
            }
        } else if llm_response.contains("CLARITY: CLEAR") {
            debug!("NP1: Task is clear, proceeding to next step.");
        } else {
            warn!(llm_response = %llm_response, "NP1: Unexpected LLM response for clarification check. Assuming clear.");
        }

        Ok(None) // Assume clear if check skipped, failed, or explicitly clear
    }


    // NP2: Check if the task should be decomposed and generate plan
    #[instrument(skip(self, task), fields(task_id = %task.id))]
    async fn check_decomposition(&self, task: &Task) -> Result<Option<RoutingDecision>, ServerError> {
        if !self.experimental_decomposition {
            trace!("Skipping decomposition check (disabled by config).");
            return Ok(None); // Skip if disabled
        }

        debug!("NP2: Checking if task should be decomposed.");
        let history_text = self.format_history(task.history.as_ref()); // Reuse helper
        let latest_request = task.history.as_ref()
            .and_then(|h| h.last())
            .map(|m| m.parts.iter().filter_map(|p| match p {
                Part::TextPart(tp) => Some(tp.text.as_str()), _ => None
            }).collect::<Vec<_>>().join(" "))
            .unwrap_or_else(|| "".to_string());

        if latest_request.is_empty() {
            warn!("Latest request text is empty, cannot check decomposition.");
            return Ok(None);
        }

        // --- NP2.A: Should Decompose? ---
        let tool_table = self.format_tools(false); // Don't need params hint here
        let agent_table = self.format_agents();
        let should_decompose_prompt = format!(
r#"SYSTEM:
You are an expert AI planner.

REQUEST_GOAL:
“{}”

AVAILABLE_LOCAL_TOOLS:
{}

AVAILABLE_REMOTE_AGENTS:
{}

CRITERIA:
• If the goal obviously maps to ONE local tool or ONE remote agent, do NOT decompose.
• Decompose when fulfilling the goal clearly needs multiple distinct skills or ordered steps.
RESPONSE_FORMAT:
SHOULD_DECOMPOSE: YES|NO
REASON: "<one line>""#,
            latest_request, tool_table, agent_table
        );

        trace!(prompt = %should_decompose_prompt, "NP2.A: Should-decompose prompt.");
        let llm_response_a = self.llm.complete(&should_decompose_prompt).await.map_err(|e| {
            error!(error = %e, "NP2.A: LLM call failed during should-decompose check.");
            ServerError::Internal(format!("LLM error during should-decompose: {}", e))
        })?;
        trace!(llm_response = %llm_response_a, "NP2.A: LLM response received.");

        if !llm_response_a.contains("SHOULD_DECOMPOSE: YES") {
            debug!("NP2.A: LLM decided not to decompose.");
            return Ok(None); // Don't decompose
        }

        info!("NP2.A: LLM decided task should be decomposed. Proceeding to generate plan.");

        // --- NP2.B: Generate Decomposition Plan ---
        let plan_prompt = format!(
r#"SYSTEM:
You chose to decompose.

GOAL:
“{}”

Produce a JSON array where each element is:
{{
 "id": "<kebab-case-step-id>",
 "input_message": "<prompt to execute>",
 "metadata": {{ "depends_on": [<ids>] }}
}}
• Keep ≤ 5 steps.
• Maintain correct dependency order.
• No extra keys."#,
            latest_request
        );

        trace!(prompt = %plan_prompt, "NP2.B: Decomposition plan prompt.");
        let llm_response_b = self.llm.complete(&plan_prompt).await.map_err(|e| {
            error!(error = %e, "NP2.B: LLM call failed during plan generation.");
            ServerError::Internal(format!("LLM error during plan generation: {}", e))
        })?;
        trace!(llm_response = %llm_response_b, "NP2.B: LLM response received (raw plan JSON).");

        // Parse the JSON plan
        match serde_json::from_str::<Vec<SubtaskDefinition>>(&llm_response_b) {
            Ok(subtasks) if !subtasks.is_empty() => {
                info!(subtask_count = subtasks.len(), "NP2.B: Successfully parsed decomposition plan.");
                // TODO: NP2.C - Resource Allocation per Sub-task would ideally happen here,
                // calling decide_execution_mode recursively for each subtask.
                // For now, we just return the plan structure. TaskService needs to handle it.
                Ok(Some(RoutingDecision::Decompose { subtasks }))
            }
            Ok(_) => {
                warn!(json_plan = %llm_response_b, "NP2.B: LLM returned empty subtask list. Proceeding without decomposition.");
                Ok(None)
            }
            Err(e) => {
                warn!(error = %e, json_plan = %llm_response_b, "NP2.B: Failed to parse decomposition plan JSON. Proceeding without decomposition.");
                Ok(None)
            }
        }
    }


    async fn decide_simple_routing(&self, task: &Task) -> Result<RoutingDecision, ServerError> {
        debug!("DP2/DP3: Performing simple routing decision (Local/Remote/Reject + Tool Choice).");
        let history_text = self.format_history(task.history.as_ref());

        if history_text.is_empty() {
            warn!("Extracted conversation history is empty. Falling back to local 'echo'.");
            return Ok(RoutingDecision::Local { tool_name: "echo".to_string(), params: json!({}) });
        }

        // --- DP2: Local vs Remote vs Reject ---
        let local_tools_desc = self.format_tools(false);
        let remote_agents_desc = self.format_agents();
        let routing_prompt = format!(
r#"You need to decide whether to handle a task locally using your own tools, delegate it to another available agent, or reject it entirely.

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

Your response should be exactly one of those formats (LOCAL, REMOTE: agent-id, or REJECT: reason), with no additional text."#,
            local_tools_desc, remote_agents_desc, history_text
        );
        trace!(prompt = %routing_prompt, "DP2: Routing prompt.");

        info!("DP2: Requesting routing decision from LLM.");
        let decision_result = self.llm.complete(&routing_prompt).await;
        let decision = match decision_result {
            Ok(d) => d.trim().to_string(),
            Err(e) => {
                error!(error = %e, "DP2: LLM routing decision failed. Falling back to local 'llm'.");
                return Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": history_text}) });
            }
        };
        info!(llm_response = %decision, "DP2: LLM routing decision response received.");

        // --- Parse DP2 Decision ---
        if decision.starts_with("LOCAL") {
            // --- DP3: Choose Local Tool & Parameters ---
            info!("DP2 decided LOCAL. Proceeding to DP3 (Tool Selection).");
            let local_tools_with_params_desc = self.format_tools(true); // Include param hints
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
- For a task like "connect to http://bar.com": {{"tool_name": "execute_command", "params": {{"command": "connect", "args": "http://bar.com"}}}}
- For a task like "list servers": {{"tool_name": "execute_command", "params": {{"command": "servers", "args": ""}}}}
- For a task like "start a new session": {{"tool_name": "execute_command", "params": {{"command": "session", "args": "new"}}}}
- For a general question: {{"tool_name": "llm", "params": {{"text": "original question text..."}}}}
- If no specific parameters are needed for the chosen tool (like list_agents with default format): {{"tool_name": "list_agents", "params": {{}}}}

Ensure the 'params' value is always a JSON object (even if empty: {{}})."#,
                history_text, local_tools_with_params_desc
            );
            trace!(prompt = %tool_param_prompt, "DP3: Tool/param extraction prompt.");

            info!("DP3: Asking LLM to choose tool and extract parameters.");
            let tool_param_result = self.llm.complete(&tool_param_prompt).await;

            match tool_param_result {
                Ok(json_str_raw) => {
                    let json_str = json_str_raw.trim();
                    trace!(raw_json = %json_str_raw, trimmed_json = %json_str, "DP3: Received tool/param JSON string from LLM.");
                    match serde_json::from_str::<Value>(json_str) {
                        Ok(json_value) => {
                            if let (Some(tool_name), Some(params)) = (
                                json_value.get("tool_name").and_then(Value::as_str),
                                json_value.get("params").cloned()
                            ) {
                                if self.enabled_tools.contains(&tool_name.to_string()) {
                                    info!(tool_name = %tool_name, ?params, "DP3: Successfully parsed tool and params.");
                                    Ok(RoutingDecision::Local { tool_name: tool_name.to_string(), params })
                                } else {
                                    warn!(chosen_tool = %tool_name, enabled_tools = ?self.enabled_tools, "DP3: LLM chose an unknown/disabled tool. Falling back to 'llm'.");
                                    Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": history_text}) })
                                }
                            } else {
                                warn!(json_response = %json_str, "DP3: LLM JSON missing 'tool_name' or 'params'. Falling back to 'llm'.");
                                Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": history_text}) })
                            }
                        },
                        Err(e) => {
                            warn!(error = %e, json_response = %json_str, "DP3: LLM returned invalid JSON for tool/params. Falling back to 'llm'.");
                            Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": history_text}) })
                        }
                    }
                },
                Err(e) => {
                    warn!(error = %e, "DP3: LLM failed to choose tool/extract params. Falling back to 'llm'.");
                    Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": history_text}) })
                }
            }
        } else if decision.starts_with("REMOTE: ") {
            let agent_id = decision.strip_prefix("REMOTE: ").unwrap().trim().to_string();
            info!(remote_agent_id = %agent_id, "DP2 decided REMOTE execution.");
            if self.agent_registry.get(&agent_id).is_none() {
                 warn!(remote_agent_id = %agent_id, "DP2: LLM delegated to unknown agent. Falling back to local 'llm'.");
                 Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": history_text}) })
            } else {
                 info!(remote_agent_id = %agent_id, "DP2: Remote delegation confirmed.");
                 Ok(RoutingDecision::Remote { agent_id })
            }
        } else if decision.starts_with("REJECT: ") {
            let reason = decision.strip_prefix("REJECT: ").unwrap().trim().to_string();
            info!(reason = %reason, "DP2 decided REJECT.");
            Ok(RoutingDecision::Reject { reason })
        } else {
            warn!(llm_decision = %decision, "DP2: LLM routing decision unclear. Falling back to local 'llm'.");
            Ok(RoutingDecision::Local { tool_name: "llm".to_string(), params: json!({"text": history_text}) })
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
        info!("DP1: Routing follow-up to LOCAL execution using 'llm' tool.");
        Ok(RoutingDecision::Local {
            tool_name: "llm".to_string(),
            params: json!({"text": follow_up_text})
        })
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
            status: TaskStatus { state: TaskState::Submitted, timestamp: Some(Utc::now()), message: None },
            history: Some(vec![params.message.clone()]),
            artifacts: None, metadata: params.metadata.clone(), session_id: params.session_id.clone(),
        };
        trace!(?task, "Temporary Task object created for should_decompose check.");
        match self.check_decomposition(&task).await? {
            Some(RoutingDecision::Decompose { .. }) => {
                debug!("Decomposition check returned YES.");
                Ok(true)
            },
            _ => {
                debug!("Decomposition check returned NO or None.");
                Ok(false)
            },
        }
    }

    // `decompose_task` now uses the LLM plan generation if enabled
    #[instrument(skip(self, params), fields(task_id = %params.id))]
    async fn decompose_task(&self, params: &TaskSendParams) -> Result<Vec<SubtaskDefinition>, ServerError> {
        debug!("'decompose_task' method called.");
        if !self.experimental_decomposition {
             trace!("Decomposition disabled by config, returning empty list.");
            return Ok(Vec::new());
        }
        // Construct temporary task to call check_decomposition helper
        let task = Task {
            id: params.id.clone(),
            status: TaskStatus { state: TaskState::Submitted, timestamp: Some(Utc::now()), message: None },
            history: Some(vec![params.message.clone()]),
            artifacts: None, metadata: params.metadata.clone(), session_id: params.session_id.clone(),
        };
         trace!(?task, "Temporary Task object created for decompose_task execution.");
        match self.check_decomposition(&task).await? {
            Some(RoutingDecision::Decompose { subtasks }) => {
                info!(subtask_count = subtasks.len(), "Returning decomposition plan.");
                Ok(subtasks)
            },
            _ => {
                warn!("Decomposition check did not yield a plan. Returning empty list.");
                Ok(Vec::new()) // Return empty if check didn't result in decomposition
            },
        }
    }
}
