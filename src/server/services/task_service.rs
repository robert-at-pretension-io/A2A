use crate::types::{Task, TaskStatus, TaskState, TaskSendParams, TaskQueryParams, TaskIdParams};
use crate::types::{Message, Role, Part, TextPart};
use crate::server::repositories::task_repository::{/* InMemoryTaskRepository, */ TaskRepository}; // Removed unused
use crate::server::ServerError;
use std::sync::Arc;
use chrono::Utc;
use tracing::{debug, error, info, trace, warn, instrument}; // Import tracing macros
use uuid::Uuid;

// Import the LlmClient trait from the new llm_client module
use crate::bidirectional::llm_client::LlmClient; // <-- Update import path
// Import helper functions and traits
use crate::bidirectional::agent_helpers::MessageExt; // <-- Import our extension trait
// Import from local server components instead of bidirectional_agent
use crate::server::{
    task_router::{RoutingDecision, LlmTaskRouterTrait}, // Removed unused TaskRouter import
    tool_executor::ToolExecutor,
    // task_flow::TaskFlow, // TaskFlow logic will be integrated here
    client_manager::ClientManager,
    agent_registry::AgentRegistry,
};
use tokio::time::{sleep, Duration}; // Added for polling delay


pub struct TaskService {
    task_repository: Arc<dyn TaskRepository>,
    // Use the LlmTaskRouterTrait for polymorphism
    
    task_router: Option<Arc<dyn LlmTaskRouterTrait>>,
    
    pub tool_executor: Option<Arc<ToolExecutor>>, // <-- Make this field public
    // Add components needed by TaskFlow if TaskService orchestrates it
    
    client_manager: Option<Arc<ClientManager>>,
    
    agent_registry: Option<Arc<AgentRegistry>>,
    
    agent_id: Option<String>, // ID of the agent running this service
    llm_client: Option<Arc<dyn LlmClient>>, // Add LLM client for rewriting
}

impl TaskService {
    /// Creates a new TaskService for standalone server mode.
    #[instrument(skip(task_repository))]
    pub fn standalone(task_repository: Arc<dyn TaskRepository>) -> Self {
        info!("Creating TaskService in standalone mode.");
        Self {
            task_repository,
            task_router: None,
            
            tool_executor: None,
            
            client_manager: None,
            
            agent_registry: None,
                
            agent_id: None,
            llm_client: None, // Initialize the new field to None for standalone mode
        }
    }

    // Removed with_task_flow constructor as TaskFlow is being removed

    /// Creates a new TaskService configured for bidirectional operation.
    /// Requires the corresponding features to be enabled.
    // Skip llm_client as it doesn't implement Debug
    #[instrument(skip(task_repository, task_router, tool_executor, client_manager, agent_registry, llm_client), fields(agent_id))]
    pub fn bidirectional(
        task_repository: Arc<dyn TaskRepository>,
        // Accept the trait object for router
         task_router: Arc<dyn LlmTaskRouterTrait>,
         tool_executor: Arc<ToolExecutor>,
        // Add Slice 3 components
         client_manager: Arc<ClientManager>,
         agent_registry: Arc<AgentRegistry>,
         agent_id: String,
         llm_client: Option<Arc<dyn LlmClient>>, // Add LLM client parameter
    ) -> Self {
        info!(%agent_id, llm_available = llm_client.is_some(), "Creating TaskService in bidirectional mode.");
        // Compile-time check for feature consistency (example)
        //
        // compile_error!("Feature 'bidir-local-exec' requires 'bidir-delegate' in this configuration.");

        Self {
            task_repository,
            task_router: Some(task_router),
            
            tool_executor: Some(tool_executor),
            
            client_manager: Some(client_manager),
            
            agent_registry: Some(agent_registry),
            
            agent_id: Some(agent_id),
            llm_client, // Store the LLM client
        }
    }


    /// Process a new task or a follow-up message
    #[instrument(skip(self, params), fields(task_id = %params.id, session_id = ?params.session_id))]
    pub async fn process_task(&self, params: TaskSendParams) -> Result<Task, ServerError> {
        info!("Processing task request."); // Keep info for start
        trace!(?params, "Full task parameters received.");
        let task_id = params.id.clone();

        // Check if task exists (for follow-up messages)
        debug!("Checking if task already exists in repository.");
        if let Some(existing_task) = self.task_repository.get_task(&task_id).await? {
            debug!("Task already exists. Processing as follow-up."); // Changed to debug
            trace!(?existing_task, "Existing task details.");
            // Pass only the new message for follow-up processing
            return self.process_follow_up(existing_task, Some(params.message)).await;
        }
        debug!("Task does not exist. Processing as new task."); // Changed to debug

        // Clone the necessary parts to avoid partial moves
        let metadata_clone = params.metadata.clone();
        let session_id_clone = params.session_id.clone();
        let initial_message = params.message.clone(); // Clone the initial message

        // Create new task
        debug!("Creating new task structure.");
        // Use provided session_id if available, otherwise generate a new one
        let session_id = session_id_clone.unwrap_or_else(|| {
            let new_id = format!("session-{}", Uuid::new_v4());
            debug!(new_session_id = %new_id, "No session ID provided, generating new one.");
            new_id
        });
        debug!(%session_id, "Using session ID for task."); // Log the session ID being used

        let mut task = Task {
            id: task_id.clone(),
            session_id: Some(session_id.clone()), // Use the determined session_id
            status: TaskStatus {
                state: TaskState::Submitted, // Start as Submitted
                timestamp: Some(Utc::now()),
                message: None,
            },
            artifacts: None,
            // Start history with the initial message
            history: Some(vec![initial_message.clone()]),
            metadata: metadata_clone,
        };
        trace!(?task, "Initial task structure created.");

        // First save the initial state (Submitted) of the task for state history
        debug!("Saving initial (Submitted) task state to repository and history.");
        self.task_repository.save_task(&task).await?;
        self.task_repository.save_state_history(&task.id, &task).await?;

        // Update status to Working before processing
        task.status.state = TaskState::Working;
        task.status.timestamp = Some(Utc::now());
        debug!("Updating task state to Working.");
        self.task_repository.save_task(&task).await?;
        self.task_repository.save_state_history(&task.id, &task).await?;
        trace!(?task, "Task state updated to Working.");


        // --- Routing and Execution Logic (Integrated from TaskFlow) ---
        debug!("Entering integrated routing and execution logic.");
        if let (Some(router), Some(executor), Some(cm), Some(_reg), Some(_agent_id)) =
            (&self.task_router, &self.tool_executor, &self.client_manager, &self.agent_registry, &self.agent_id)
        {
            debug!("Bidirectional components found. Proceeding with routing and execution."); // Changed to debug
            // Make routing decision
            debug!("Calling task router decide method.");
            let decision_result = match router.decide(&params).await {
                Ok(decision) => decision,
                Err(e) => {
                    error!(error = %e, "Task routing decision failed.");
                    task.status.state = TaskState::InputRequired; // <-- Change state
                    task.status.message = Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Internal routing decision failed: {}. Please clarify your request or try again.", e), // <-- Update message
                            metadata: None,
                        })],
                        metadata: Some(serde_json::json!({ "error_context": e.to_string() }).as_object().unwrap().clone()),
                    });
                    task.status.timestamp = Some(Utc::now());
                    self.task_repository.save_task(&task).await?;
                    self.task_repository.save_state_history(&task.id, &task).await?;
                    return Ok(task); // Return the failed task
                }
            };
            debug!(?decision_result, "Routing decision received."); // Changed to debug

            // Process the decision
            match decision_result {
                RoutingDecision::Local { tool_name, params } => {
                    info!(%tool_name, ?params, "Executing task locally using tool and extracted parameters."); // Keep info for local execution start
                    // Execute locally and wait for completion, passing the extracted params
                    match executor.execute_task_locally(&mut task, &tool_name, params).await {
                        Ok(_) => {
                            debug!("Local execution finished by tool executor.");
                            // Task state should be updated by the executor (Completed or Failed)
                        }
                        Err(e) => {
                            error!(error = %e, "Local tool execution failed.");
                            task.status.state = TaskState::Failed;
                            task.status.message = Some(Message { role: Role::Agent, parts: vec![Part::TextPart(TextPart { type_: "text".to_string(), text: format!("Local execution failed: {}", e), metadata: None })], metadata: None });
                            task.status.timestamp = Some(Utc::now());
                        }
                    }
                    // Save the final state after local execution
                    self.task_repository.save_task(&task).await?;
                    self.task_repository.save_state_history(&task.id, &task).await?;
                }
                RoutingDecision::Remote { agent_id: remote_agent_id } => {
                    info!(%remote_agent_id, "Delegating task to remote agent."); // Keep info for delegation start

                    // --- LLM Rewrite Logic ---
                    let mut message_to_send = initial_message.clone(); // Start with original message
                    let delegating_agent_id = self.agent_id.as_deref().unwrap_or("UnknownAgent");

                    if let Some(llm) = &self.llm_client {
                        debug!("Attempting to rewrite message using LLM for delegation.");
                        // Extract original text
                        let original_text = initial_message.parts.iter()
                            .filter_map(|p| match p {
                                Part::TextPart(tp) => Some(tp.text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n");

                        if !original_text.is_empty() {
                            let rewrite_prompt = format!(
                                r#"You are helping Agent '{delegating_agent_id}' delegate a task to Agent '{remote_agent_id}'.
Agent '{delegating_agent_id}' received the following request from a user:
"{original_text}"

Rewrite this request as a message FROM Agent '{delegating_agent_id}' TO Agent '{remote_agent_id}'.
The rewritten message should:
1. Briefly introduce Agent '{delegating_agent_id}'.
2. Explain that it's forwarding a request from a user.
3. Clearly state the user's original request (rephrased naturally if needed, keeping the core intent).

Respond ONLY with the rewritten message text, suitable for sending directly to Agent '{remote_agent_id}'."#
                            );
                            trace!(prompt = %rewrite_prompt, "Sending rewrite prompt to LLM.");

                            match llm.complete(&rewrite_prompt).await {
                                Ok(rewritten_text) => {
                                    debug!("Successfully rewrote message for delegation."); // Changed to debug
                                    trace!(rewritten_text = %rewritten_text, "Rewritten message content.");
                                    // Replace the message content with the rewritten text
                                    message_to_send = Message {
                                        role: Role::User, // Keep role as User from the perspective of the receiving agent
                                        parts: vec![Part::TextPart(TextPart {
                                            type_: "text".to_string(),
                                            text: rewritten_text.trim().to_string(),
                                            metadata: None,
                                        })],
                                        metadata: None, // Or copy metadata if needed
                                    };
                                }
                                Err(e) => {
                                    warn!(error = %e, "LLM rewrite failed. Sending original message.");
                                    // message_to_send remains the original message
                                }
                            }
                        } else {
                            warn!("Original message text was empty. Skipping rewrite.");
                        }
                    } else {
                        debug!("LLM client not available in TaskService. Sending original message.");
                        // message_to_send remains the original message
                    }
                    // --- End LLM Rewrite Logic ---


                    // Prepare TaskSendParams for delegation using the potentially rewritten message
                    let delegation_params = TaskSendParams {
                        id: Uuid::new_v4().to_string(), // Generate a new ID for the remote task
                        message: message_to_send, // Use the (potentially rewritten) message
                        session_id: Some(session_id.clone()), // Pass session ID
                        metadata: task.metadata.clone(),
                        history_length: None,
                        push_notification: None,
                    };
                    let remote_task_id = delegation_params.id.clone(); // Store remote task ID

                    // Initiate delegation
                    debug!(remote_agent_id=%remote_agent_id, remote_task_id=%remote_task_id, "Sending task parameters to ClientManager."); // Changed to debug
                    match cm.send_task(&remote_agent_id, delegation_params).await {
                        Ok(initial_remote_task) => {
                            info!(%remote_agent_id, %remote_task_id, initial_state = ?initial_remote_task.status.state, "Delegation initiated successfully."); // Keep info for delegation success
                            // Update local task to indicate delegation
                            task.status.state = TaskState::Working; // Keep local task Working
                            task.status.timestamp = Some(Utc::now());
                            task.status.message = Some(Message {
                                role: Role::Agent,
                                parts: vec![Part::TextPart(TextPart {
                                    type_: "text".to_string(),
                                    text: format!("Task delegated to agent '{}'. Remote Task ID: {}. Monitoring...", remote_agent_id, remote_task_id),
                                    metadata: None,
                                })],
                                metadata: None,
                            });
                            task.metadata.get_or_insert_with(Default::default).insert("remote_task_id".to_string(), remote_task_id.clone().into());
                            task.metadata.get_or_insert_with(Default::default).insert("delegated_to_agent_id".to_string(), remote_agent_id.clone().into());

                            self.task_repository.save_task(&task).await?;
                            self.task_repository.save_state_history(&task.id, &task).await?;

                            // --- Start Polling Remote Task ---
                            debug!(%remote_agent_id, %remote_task_id, "Starting to poll remote task status."); // Changed to debug
                            let max_polling_attempts = 30; // e.g., 30 attempts * 2 seconds = 1 minute timeout
                            let polling_interval = Duration::from_secs(2);
                            let mut final_remote_task: Option<Task> = None;

                            for attempt in 1..=max_polling_attempts {
                                debug!(attempt, max_polling_attempts, "Polling remote task status.");
                                match cm.get_task_status(&remote_agent_id, &remote_task_id).await {
                                    Ok(remote_task_update) => {
                                        trace!(remote_state = ?remote_task_update.status.state, "Received remote task status update.");
                                        // Check if remote task reached a final state
                                        if matches!(remote_task_update.status.state, TaskState::Completed | TaskState::Failed | TaskState::Canceled | TaskState::InputRequired) {
                                            info!(final_state = ?remote_task_update.status.state, "Remote task reached final state."); // Keep info for final state
                                            final_remote_task = Some(remote_task_update);
                                            break; // Exit polling loop
                                        }
                                        // Continue polling if still working
                                    }
                                    Err(e) => {
                                        error!(error = %e, "Polling remote task failed. Aborting monitoring.");
                                        // Update local task to InputRequired due to monitoring error
                                        task.status.state = TaskState::InputRequired; // <-- Change state
                                        task.status.message = Some(Message {
                                            role: Role::Agent,
                                            parts: vec![Part::TextPart(TextPart {
                                                type_: "text".to_string(),
                                                text: format!(
                                                    "Lost connection while monitoring delegated task '{}'. Error: {}. What should I do?",
                                                    remote_task_id, e
                                                ),
                                                metadata: None,
                                            })],
                                            metadata: Some(serde_json::json!({ "error_context": e.to_string() }).as_object().unwrap().clone()),
                                        });
                                        task.status.timestamp = Some(Utc::now());
                                        self.task_repository.save_task(&task).await?;
                                        self.task_repository.save_state_history(&task.id, &task).await?;
                                        return Ok(task); // Return the local task (now InputRequired)
                                    }
                                }
                                // Wait before next poll
                                sleep(polling_interval).await;
                            }

                            // --- Update Local Task with Final Remote State ---
                            if let Some(final_task) = final_remote_task {
                                info!(final_state = ?final_task.status.state, "Updating local task with final remote state."); // Keep info for update
                                // Mirror the final state, message, and artifacts
                                task.status = final_task.status;
                                task.artifacts = final_task.artifacts;
                                // Ensure timestamp is updated
                                task.status.timestamp = Some(Utc::now());
                            } else {
                                warn!(polling_attempts = max_polling_attempts, "Remote task did not reach final state after polling. Marking local task as InputRequired.");
                                task.status.state = TaskState::InputRequired; // <-- Change state
                                task.status.message = Some(Message {
                                    role: Role::Agent,
                                    parts: vec![Part::TextPart(TextPart {
                                        type_: "text".to_string(),
                                        text: format!(
                                            "The delegated task '{}' timed out without reaching a final state. How should I proceed?",
                                            remote_task_id
                                        ),
                                        metadata: None,
                                    })],
                                    metadata: Some(serde_json::json!({ "error_context": "polling_timeout" }).as_object().unwrap().clone()),
                                });
                                task.status.timestamp = Some(Utc::now());
                            }
                            // Save the final mirrored state
                            self.task_repository.save_task(&task).await?;
                            self.task_repository.save_state_history(&task.id, &task).await?;

                        }
                        Err(e) => {
                            error!(%remote_agent_id, error = %e, "Failed to initiate task delegation.");
                            // --- CHANGE START ---
                            task.status.state = TaskState::InputRequired; // <-- Change state
                            task.status.message = Some(Message {
                                role: Role::Agent,
                                parts: vec![Part::TextPart(TextPart {
                                    type_: "text".to_string(),
                                    text: format!(
                                        "Failed to send the task to agent '{}'. Error: {}. Should I retry or handle it locally?",
                                        remote_agent_id, e
                                    ),
                                    metadata: None,
                                })],
                                metadata: Some(serde_json::json!({ "error_context": e.to_string() }).as_object().unwrap().clone()),
                            });
                            task.status.timestamp = Some(Utc::now());
                            // --- CHANGE END ---
                            self.task_repository.save_task(&task).await?;
                            self.task_repository.save_state_history(&task.id, &task).await?;
                        }
                    }
                }
                RoutingDecision::Reject { reason } => {
                    info!(%reason, "Task rejected based on routing decision."); // Keep info for rejection
                    task.status.state = TaskState::Failed;
                    task.status.timestamp = Some(Utc::now());
                    task.status.message = Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: format!("Task rejected: {}", reason),
                            metadata: None,
                        })],
                        metadata: None,
                    });
                    self.task_repository.save_task(&task).await?;
                    self.task_repository.save_state_history(&task.id, &task).await?;
                }
                RoutingDecision::Decompose { subtasks: _ } => {
                    warn!("Task decomposition requested but not implemented.");
                    task.status.state = TaskState::Failed;
                    task.status.timestamp = Some(Utc::now());
                    task.status.message = Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: "Task decomposition is not implemented.".to_string(),
                            metadata: None,
                        })],
                        metadata: None,
                    });
                    self.task_repository.save_task(&task).await?;
                    self.task_repository.save_state_history(&task.id, &task).await?;
                },
                RoutingDecision::NeedsClarification { question } => {
                    info!(task_id = %task.id, "Routing decision: Needs Clarification.");
                    // Update task state to InputRequired with the clarification question
                    task.status.state = TaskState::InputRequired;
                    task.status.message = Some(Message::agent(question)); // Use the question from the decision
                    task.status.timestamp = Some(Utc::now());
                    self.task_repository.save_task(&task).await?;
                    self.task_repository.save_state_history(&task.id, &task).await?;
                },
                RoutingDecision::Decompose { subtasks } => {
                    info!(task_id = %task.id, subtask_count = subtasks.len(), "Routing decision: Decompose.");
                    // TODO: Implement actual decomposition logic
                    // - Create child tasks based on subtasks
                    // - Manage dependencies and execution flow
                    // - Synthesize results (NP4)
                    warn!(task_id = %task.id, "Task decomposition not yet fully implemented in TaskService. Marking task as Failed.");
                    task.status.state = TaskState::Failed;
                    task.status.message = Some(Message::agent("Task decomposition is not yet supported.".to_string()));
                    task.status.timestamp = Some(Utc::now());
                    self.task_repository.save_task(&task).await?;
                    self.task_repository.save_state_history(&task.id, &task).await?;
                }
            }
        } else {
            // Fallback to default processing (standalone mode)
            debug!("Router/Executor/ClientManager not available (standalone mode?). Using default task content processing."); // Changed to debug
            self.process_task_content(&mut task, Some(initial_message)).await?;
            // Save the final state after default processing
            self.task_repository.save_task(&task).await?;
            self.task_repository.save_state_history(&task.id, &task).await?;
        }
        // --- End Integrated Routing and Execution Logic ---

        info!(final_state = ?task.status.state, "Finished processing task request. Returning final task state."); // Keep info for end of processing
        Ok(task) // Return the final state of the task
    }

    /// Process follow-up message for an existing task
    #[instrument(skip(self, task, message), fields(task_id = %task.id, current_state = ?task.status.state))]
    async fn process_follow_up(&self, mut task: Task, message: Option<Message>) -> Result<Task, ServerError> {
        info!("Processing follow-up message."); // Keep info for follow-up start
        trace!(?message, "Follow-up message details.");

        // Add the follow-up message to history *before* processing
        if let Some(ref msg) = message {
             debug!("Adding follow-up message to task history.");
             task.history.get_or_insert_with(Vec::new).push(msg.clone());
             // Save intermediate state with new history message
             self.task_repository.save_task(&task).await?;
             // Don't save history state here yet, wait until after processing
        }


        // Only process follow-up if task is in a state that allows it
        match task.status.state {
            TaskState::InputRequired => {
                debug!("Task is in InputRequired state. Processing follow-up."); // Changed to debug
                // Process the follow-up message using the configured router/executor if available
                if let (Some(router), Some(executor), Some(message_val)) = (&self.task_router, &self.tool_executor, &message) {
                     debug!("Using bidirectional components for follow-up processing.");
                     
                     // Get routing decision for the follow-up message
                     match router.process_follow_up(&task.id, message_val).await {
                         Ok(decision) => {
                             debug!(?decision, "Follow-up routing decision received.");
                             
                             // Process the follow-up according to the decision
                             match decision {
                                 RoutingDecision::Local { tool_name, params } => {
                                     info!(%tool_name, ?params, "Executing follow-up locally using tool and extracted parameters.");
                                     // Execute locally and wait for completion, passing the extracted params
                                     match executor.execute_task_locally(&mut task, &tool_name, params).await {
                                         Ok(_) => {
                                             debug!("Local execution of follow-up finished by tool executor.");
                                             // Task state should be updated by the executor (Completed or Failed)
                                         }
                                         Err(e) => {
                                             error!(error = %e, "Local tool execution for follow-up failed.");
                                             task.status.state = TaskState::Failed;
                                             task.status.message = Some(Message { 
                                                 role: Role::Agent, 
                                                 parts: vec![Part::TextPart(TextPart { 
                                                     type_: "text".to_string(), 
                                                     text: format!("Follow-up execution failed: {}", e), 
                                                     metadata: None 
                                                 })], 
                                                 metadata: None 
                                             });
                                             task.status.timestamp = Some(Utc::now());
                                         }
                                     }
                                 },
                                 RoutingDecision::Remote { agent_id } => {
                                     info!(%agent_id, "Delegating follow-up to remote agent.");
                                     
                                     // Check if we have the client manager for remote delegation
                                     if let Some(cm) = &self.client_manager {
                                         // Handle remote delegation similar to initial task delegation
                                         // Potentially simplify compared to full task delegation
                                         
                                         // --- LLM Rewrite Logic (simplified for follow-up) ---
                                         let mut message_to_send = message_val.clone();
                                         let delegating_agent_id = self.agent_id.as_deref().unwrap_or("UnknownAgent");
                                         
                                         if let Some(llm) = &self.llm_client {
                                             debug!("Attempting to rewrite follow-up message using LLM for delegation.");
                                             // Extract original text
                                             let original_text = message_val.parts.iter()
                                                 .filter_map(|p| match p {
                                                     Part::TextPart(tp) => Some(tp.text.as_str()),
                                                     _ => None,
                                                 })
                                                 .collect::<Vec<_>>()
                                                 .join("\n");
                                             
                                             if !original_text.is_empty() {
                                                 let rewrite_prompt = format!(
                                                     r#"You are helping Agent '{delegating_agent_id}' delegate a follow-up request to Agent '{agent_id}'.
                                                     Agent '{delegating_agent_id}' received the following follow-up message from a user:
                                                     "{original_text}"
                                                     
                                                     Rewrite this as a follow-up message FROM Agent '{delegating_agent_id}' TO Agent '{agent_id}'.
                                                     The rewritten message should:
                                                     1. Briefly remind Agent '{agent_id}' that this is a follow-up to a previous request.
                                                     2. Clearly state the new information or question from the user.
                                                     
                                                     Respond ONLY with the rewritten message text, suitable for sending directly to Agent '{agent_id}'."#
                                                 );
                                                 
                                                 match llm.complete(&rewrite_prompt).await {
                                                     Ok(rewritten_text) => {
                                                         debug!("Successfully rewrote follow-up message for delegation.");
                                                         trace!(rewritten_text = %rewritten_text, "Rewritten follow-up message content.");
                                                         // Replace the message content with the rewritten text
                                                         message_to_send = Message {
                                                             role: Role::User,
                                                             parts: vec![Part::TextPart(TextPart {
                                                                 type_: "text".to_string(),
                                                                 text: rewritten_text.trim().to_string(),
                                                                 metadata: None,
                                                             })],
                                                             metadata: None,
                                                         };
                                                     }
                                                     Err(e) => {
                                                         warn!(error = %e, "LLM rewrite of follow-up failed. Sending original message.");
                                                     }
                                                 }
                                             }
                                         }
                                         // --- End LLM Rewrite Logic ---
                                         
                                         // Create a new task for the delegated follow-up
                                         let delegation_params = TaskSendParams {
                                             id: Uuid::new_v4().to_string(),
                                             message: message_to_send,
                                             session_id: task.session_id.clone(),
                                             metadata: task.metadata.clone(),
                                             history_length: None,
                                             push_notification: None,
                                         };
                                         let remote_task_id = delegation_params.id.clone();
                                         
                                         // Send the delegated follow-up
                                         match cm.send_task(&agent_id, delegation_params).await {
                                             Ok(initial_remote_task) => {
                                                 info!(%agent_id, %remote_task_id, initial_state = ?initial_remote_task.status.state, "Follow-up delegation initiated successfully.");
                                                 
                                                 // Update local task to indicate delegation
                                                 task.status.state = TaskState::Working;
                                                 task.status.timestamp = Some(Utc::now());
                                                 task.status.message = Some(Message {
                                                     role: Role::Agent,
                                                     parts: vec![Part::TextPart(TextPart {
                                                         type_: "text".to_string(),
                                                         text: format!("Follow-up delegated to agent '{}'. Remote Task ID: {}. Monitoring...", agent_id, remote_task_id),
                                                         metadata: None,
                                                     })],
                                                     metadata: None,
                                                 });
                                                 
                                                 // Save remote task ID in metadata
                                                 task.metadata.get_or_insert_with(Default::default).insert(
                                                     "remote_follow_up_task_id".to_string(), 
                                                     remote_task_id.clone().into()
                                                 );
                                                 
                                                 self.task_repository.save_task(&task).await?;
                                                 self.task_repository.save_state_history(&task.id, &task).await?;
                                                 
                                                 // Poll for remote task completion
                                                 let max_polling_attempts = 30;
                                                 let polling_interval = Duration::from_secs(2);
                                                 let mut final_remote_task: Option<Task> = None;
                                                 
                                                 for attempt in 1..=max_polling_attempts {
                                                     debug!(attempt, max_polling_attempts, "Polling remote follow-up task status.");
                                                     match cm.get_task_status(&agent_id, &remote_task_id).await {
                                                         Ok(remote_task_update) => {
                                                             trace!(remote_state = ?remote_task_update.status.state, "Received remote follow-up task status update.");
                                                             
                                                             // Check if remote task reached a final state
                                                             if matches!(remote_task_update.status.state, 
                                                                 TaskState::Completed | TaskState::Failed | TaskState::Canceled | TaskState::InputRequired) 
                                                             {
                                                                 info!(final_state = ?remote_task_update.status.state, "Remote follow-up task reached final state.");
                                                                 final_remote_task = Some(remote_task_update);
                                                                 break;
                                                             }
                                                         }
                                                         Err(e) => {
                                                             error!(error = %e, "Polling remote follow-up task failed.");
                                                             // Update local task due to monitoring error
                                                             task.status.state = TaskState::Failed;
                                                             task.status.message = Some(Message {
                                                                 role: Role::Agent,
                                                                 parts: vec![Part::TextPart(TextPart {
                                                                     type_: "text".to_string(),
                                                                     text: format!(
                                                                         "Lost connection while monitoring delegated follow-up task '{}'. Error: {}.",
                                                                         remote_task_id, e
                                                                     ),
                                                                     metadata: None,
                                                                 })],
                                                                 metadata: Some(serde_json::json!({ "error_context": e.to_string() }).as_object().unwrap().clone()),
                                                             });
                                                             task.status.timestamp = Some(Utc::now());
                                                             self.task_repository.save_task(&task).await?;
                                                             self.task_repository.save_state_history(&task.id, &task).await?;
                                                             return Ok(task);
                                                         }
                                                     }
                                                     // Wait before next poll
                                                     sleep(polling_interval).await;
                                                 }
                                                 
                                                 // Update task with final remote state
                                                 if let Some(final_task) = final_remote_task {
                                                     info!(final_state = ?final_task.status.state, "Updating local task with final remote follow-up state.");
                                                     
                                                     // Mirror the final state, message, and artifacts
                                                     task.status = final_task.status;
                                                     
                                                     // If remote task produced artifacts, add them to our task
                                                     if let Some(remote_artifacts) = final_task.artifacts {
                                                         if !remote_artifacts.is_empty() {
                                                             // Get our existing artifacts or create empty vec
                                                             let artifacts = task.artifacts.get_or_insert_with(Vec::new);
                                                             
                                                             // Add remote artifacts to our collection
                                                             for mut artifact in remote_artifacts {
                                                                 // Update index to continue from our last artifact
                                                                 artifact.index = artifacts.len() as i64;
                                                                 artifacts.push(artifact);
                                                             }
                                                         }
                                                     }
                                                     
                                                     // Ensure timestamp is updated
                                                     task.status.timestamp = Some(Utc::now());
                                                 } else {
                                                     warn!(polling_attempts = max_polling_attempts, "Remote follow-up task did not reach final state after polling. Marking as Failed.");
                                                     task.status.state = TaskState::Failed;
                                                     task.status.message = Some(Message {
                                                         role: Role::Agent,
                                                         parts: vec![Part::TextPart(TextPart {
                                                             type_: "text".to_string(),
                                                             text: format!(
                                                                 "The delegated follow-up task '{}' timed out without reaching a final state.",
                                                                 remote_task_id
                                                             ),
                                                             metadata: None,
                                                         })],
                                                         metadata: Some(serde_json::json!({ "error_context": "polling_timeout" }).as_object().unwrap().clone()),
                                                     });
                                                     task.status.timestamp = Some(Utc::now());
                                                 }
                                                 
                                                 // Save the final state
                                                 self.task_repository.save_task(&task).await?;
                                                 self.task_repository.save_state_history(&task.id, &task).await?;
                                             }
                                             Err(e) => {
                                                 error!(%agent_id, error = %e, "Failed to delegate follow-up task.");
                                                 task.status.state = TaskState::Failed;
                                                 task.status.message = Some(Message {
                                                     role: Role::Agent,
                                                     parts: vec![Part::TextPart(TextPart {
                                                         type_: "text".to_string(),
                                                         text: format!(
                                                             "Failed to send follow-up to agent '{}'. Error: {}.",
                                                             agent_id, e
                                                         ),
                                                         metadata: None,
                                                     })],
                                                     metadata: Some(serde_json::json!({ "error_context": e.to_string() }).as_object().unwrap().clone()),
                                                 });
                                                 task.status.timestamp = Some(Utc::now());
                                                 self.task_repository.save_task(&task).await?;
                                                 self.task_repository.save_state_history(&task.id, &task).await?;
                                             }
                                         }
                                     } else {
                                         error!("Client manager not available for remote follow-up delegation.");
                                         task.status.state = TaskState::Failed;
                                         task.status.message = Some(Message {
                                             role: Role::Agent,
                                             parts: vec![Part::TextPart(TextPart {
                                                 type_: "text".to_string(),
                                                 text: "Cannot delegate follow-up: client manager not available.".to_string(),
                                                 metadata: None,
                                             })],
                                             metadata: None,
                                         });
                                         task.status.timestamp = Some(Utc::now());
                                         self.task_repository.save_task(&task).await?;
                                         self.task_repository.save_state_history(&task.id, &task).await?;
                                     }
                                 },
                                 RoutingDecision::Reject { reason } => {
                                     info!(%reason, "Follow-up message rejected based on routing decision.");
                                     task.status.state = TaskState::Failed;
                                     task.status.timestamp = Some(Utc::now());
                                     task.status.message = Some(Message {
                                         role: Role::Agent,
                                         parts: vec![Part::TextPart(TextPart {
                                             type_: "text".to_string(),
                                             text: format!("Follow-up rejected: {}", reason),
                                             metadata: None,
                                         })],
                                         metadata: None,
                                     });
                                     self.task_repository.save_task(&task).await?;
                                     self.task_repository.save_state_history(&task.id, &task).await?;
                                 },
                                 RoutingDecision::Decompose { subtasks: _ } => {
                                     warn!("Follow-up decomposition requested but not implemented.");
                                     task.status.state = TaskState::Failed;
                                     task.status.timestamp = Some(Utc::now());
                                     task.status.message = Some(Message {
                                         role: Role::Agent,
                                         parts: vec![Part::TextPart(TextPart {
                                             type_: "text".to_string(),
                                             text: "Follow-up decomposition is not implemented.".to_string(),
                                             metadata: None,
                                         })],
                                         metadata: None,
                                     });
                                     self.task_repository.save_task(&task).await?;
                                     self.task_repository.save_state_history(&task.id, &task).await?;
                                 },
                                 RoutingDecision::NeedsClarification { question } => {
                                     // This case should ideally not happen during follow-up processing,
                                     // as clarification should occur before routing.
                                     // If it does, treat it as needing human input again.
                                     warn!(task_id = %task.id, "Received NeedsClarification decision during follow-up processing. Setting state to InputRequired.");
                                     task.status.state = TaskState::InputRequired;
                                     task.status.message = Some(Message::agent(question));
                                     task.status.timestamp = Some(Utc::now());
                                     self.task_repository.save_task(&task).await?;
                                     self.task_repository.save_state_history(&task.id, &task).await?;
                                 },
                                 RoutingDecision::Decompose { subtasks } => {
                                     // Similar to NeedsClarification, decomposition should ideally happen
                                     // on the initial routing, not during follow-up.
                                     warn!(task_id = %task.id, subtask_count = subtasks.len(), "Received Decompose decision during follow-up processing. Marking task as Failed.");
                                     task.status.state = TaskState::Failed;
                                     task.status.message = Some(Message::agent("Task decomposition during follow-up is not supported.".to_string()));
                                     task.status.timestamp = Some(Utc::now());
                                     self.task_repository.save_task(&task).await?;
                                     self.task_repository.save_state_history(&task.id, &task).await?;
                                 }
                             }
                         },
                         Err(e) => {
                             error!(error = %e, "Follow-up routing decision failed.");
                             task.status.state = TaskState::Failed;
                             task.status.message = Some(Message {
                                 role: Role::Agent,
                                 parts: vec![Part::TextPart(TextPart {
                                     type_: "text".to_string(),
                                     text: format!("Follow-up routing failed: {}.", e),
                                     metadata: None,
                                 })],
                                 metadata: Some(serde_json::json!({ "error_context": e.to_string() }).as_object().unwrap().clone()),
                             });
                             task.status.timestamp = Some(Utc::now());
                             self.task_repository.save_task(&task).await?;
                             self.task_repository.save_state_history(&task.id, &task).await?;
                         }
                     }
                } else {
                    debug!("Standalone mode or missing message: Processing follow-up content directly.");
                    // Standalone: Process content directly (or use a simpler logic)
                    self.process_task_content(&mut task, message).await?; // process_task_content handles state transition
                }

                // Save the updated task and state history
                debug!("Saving final task state after follow-up processing.");
                self.task_repository.save_task(&task).await?;
                self.task_repository.save_state_history(&task.id, &task).await?;

            },
            TaskState::Completed | TaskState::Failed | TaskState::Canceled => {
                error!("Task is in a final state ({:?}) and cannot accept follow-up messages.", task.status.state);
                return Err(ServerError::InvalidParameters(
                    format!("Task {} is in {} state and cannot accept follow-up messages",
                            task.id, task.status.state)
                ));
            },
            TaskState::Working | TaskState::Submitted => {
                // For Working/Submitted state, reject follow-up for now
                warn!("Task is in {:?} state. Rejecting follow-up message.", task.status.state);
                return Err(ServerError::InvalidParameters(
                    format!("Task {} is still processing ({:?}). Cannot accept follow-up message yet.",
                            task.id, task.status.state)
                ));
            },
             // Handle Unknown or other potential states if necessary
             _ => {
                 error!("Task is in unhandled state ({:?}) for follow-up.", task.status.state);
                 return Err(ServerError::Internal(format!("Task {} in unhandled state {:?} for follow-up", task.id, task.status.state)));
             }
        }

        info!(final_state = ?task.status.state, "Finished processing follow-up message."); // Keep info for follow-up end
        Ok(task)
    }

    /// Process the content of a task (simplified implementation for reference server)
    #[instrument(skip(self, task, message), fields(task_id = %task.id))]
    async fn process_task_content(&self, task: &mut Task, message: Option<Message>) -> Result<(), ServerError> {
        debug!("Processing task content (standalone/fallback logic)."); // Changed to debug
        trace!(?message, "Message content being processed.");
        // Simplified implementation - in a real server, this would process the task asynchronously
        // and potentially update the task status multiple times

        // If the message has specific mock parameters, we can use them to simulate different behaviors
        let metadata = if let Some(ref msg) = message {
            msg.metadata.clone()
        } else {
            None
        };
        trace!(?metadata, "Metadata from message (if any).");

        // Check if we should request input - check both message metadata and task metadata
        debug!("Checking if task requires input based on metadata.");
        let require_input = if let Some(ref meta) = metadata {
            meta.get("_mock_require_input").and_then(|v| v.as_bool()).unwrap_or(false)
        } else if let Some(ref task_meta) = task.metadata {
            task_meta.get("_mock_require_input").and_then(|v| v.as_bool()).unwrap_or(false)
        } else {
            false
        };
        trace!(%require_input, "Input required flag.");

        // Process artifacts if this is a file or data task (for fixing file/data artifact tests)
        debug!("Processing potential artifacts from message.");
        self.process_task_artifacts(task, &message).await?; // Logs internally now

        if require_input {
            info!("Task requires input. Updating status to InputRequired."); // Keep info for state change
            // Update task status to request input
            task.status = TaskStatus {
                state: TaskState::InputRequired,
                timestamp: Some(Utc::now()),
                message: Some(Message {
                    role: Role::Agent,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: "Please provide additional information to continue.".to_string(),
                        metadata: None,
                    })],
                    metadata: None,
                }),
            };
            trace!(?task.status, "Task status updated to InputRequired.");
            return Ok(());
        }

        // Check if we should keep the task in working state
        debug!("Checking if task should remain in Working state based on metadata.");
        let remain_working = if let Some(ref task_meta) = task.metadata { // Check task metadata
            task_meta.get("_mock_remain_working").and_then(|v| v.as_bool()).unwrap_or(false)
        } else {
            false
        };
        trace!(%remain_working, "Remain working flag.");

        if remain_working {
            debug!("Task metadata indicates it should remain Working."); // Changed to debug
            // Keep the task in working state (or update timestamp/message if needed)
            task.status.state = TaskState::Working; // Ensure it's Working
            task.status.timestamp = Some(Utc::now());
            task.status.message = Some(Message {
                role: Role::Agent,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "Task is still processing.".to_string(),
                    metadata: None,
                })],
                metadata: None,
            });
            trace!(?task.status, "Task status kept as Working.");
        } else {
            info!("Task does not require input or remain working. Marking as Completed."); // Keep info for state change
            // Just mark as completed for this reference implementation
            task.status = TaskStatus {
                state: TaskState::Completed,
                timestamp: Some(Utc::now()),
                message: Some(Message {
                    role: Role::Agent,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: "Task completed successfully.".to_string(),
                        metadata: None,
                    })],
                    metadata: None,
                }),
            };
             // Add artifact for completed task if none exists yet (for basic tests)
             if task.artifacts.is_none() || task.artifacts.as_ref().map_or(true, |a| a.is_empty()) {
                 debug!("Adding default completion artifact.");
                 let completion_artifact = crate::types::Artifact {
                     index: 0,
                     name: Some("result.txt".to_string()),
                     parts: vec![crate::types::Part::TextPart(crate::types::TextPart {
                         type_: "text".to_string(),
                         text: "Default task completion result.".to_string(),
                         metadata: None,
                     })],
                     description: Some("Default completion artifact".to_string()),
                     append: None,
                     last_chunk: None,
                     metadata: None,
                 };
                 task.artifacts = Some(vec![completion_artifact]);
                 trace!(?task.artifacts, "Default artifact added.");
             }
            trace!(?task.status, "Task status updated to Completed.");
        }

        Ok(())
    }

    /// Process file or data artifacts
    #[instrument(skip(self, task, message), fields(task_id = %task.id))]
    async fn process_task_artifacts(&self, task: &mut Task, message: &Option<Message>) -> Result<(), ServerError> {
        debug!("Processing artifacts from message (if any).");
        // Check if message has file or data parts
        if let Some(ref msg) = message {
            trace!(part_count = msg.parts.len(), "Checking message parts for files/data.");
            let mut has_file = false;
            let mut has_data = false;

            for (i, part) in msg.parts.iter().enumerate() {
                match part {
                    Part::FilePart(fp) => {
                        trace!(part_index = i, file_name = ?fp.file.name, "Found FilePart.");
                        has_file = true;
                    },
                    Part::DataPart(_dp) => { // Prefix dp with _
                        trace!(part_index = i, "Found DataPart.");
                        has_data = true;
                    },
                    Part::TextPart(_) => {
                        trace!(part_index = i, "Found TextPart (ignored for artifact processing).");
                    }
                    // Handle unknown part types if necessary
                    // _ => { warn!(part_index = i, "Found unknown part type."); }
                }
            }
            trace!(%has_file, %has_data, "File/Data part check complete.");

            // Create artifacts array if needed
            if (has_file || has_data) && task.artifacts.is_none() {
                debug!("Initializing artifacts vector for task.");
                task.artifacts = Some(Vec::new());
            }

            // Add file artifact if found
            if has_file {
                debug!("Creating artifact for processed file content.");
                // Create text part for file artifact
                let text_part = crate::types::TextPart {
                    type_: "text".to_string(),
                    text: "This is processed file content".to_string(),
                    metadata: None,
                };
                trace!(?text_part, "Created text part for file artifact.");

                // Create artifact with correct fields based on type definition
                let file_artifact = crate::types::Artifact {
                    index: task.artifacts.as_ref().map_or(0, |a| a.len() as i64), // Next index
                    name: Some("processed_file.txt".to_string()),
                    parts: vec![crate::types::Part::TextPart(text_part)],
                    description: Some("Processed file from uploaded content".to_string()),
                    append: None,
                    last_chunk: None,
                    metadata: None,
                };
                trace!(?file_artifact, "Created file artifact.");

                if let Some(ref mut artifacts) = task.artifacts {
                    artifacts.push(file_artifact);
                    debug!("Added file artifact to task.");
                }
            }

            // Add data artifact if found
            if has_data {
                 debug!("Creating artifact for processed data content.");
                // Create data part for json artifact
                let mut data_map = serde_json::Map::new();
                data_map.insert("processed".to_string(), serde_json::json!(true));
                data_map.insert("timestamp".to_string(), serde_json::json!(chrono::Utc::now().to_rfc3339()));
                trace!(?data_map, "Created data map for data artifact.");

                let data_part = crate::types::DataPart {
                    type_: "json".to_string(), // Should this be "data"? Check schema/types.rs
                    data: data_map,
                    metadata: None,
                };
                 trace!(?data_part, "Created data part for data artifact.");

                // Create artifact with correct fields based on type definition
                let data_artifact = crate::types::Artifact {
                    index: task.artifacts.as_ref().map_or(0, |a| a.len() as i64), // Next index
                    name: Some("processed_data.json".to_string()),
                    parts: vec![crate::types::Part::DataPart(data_part)],
                    description: Some("Processed data from request".to_string()),
                    append: None,
                    last_chunk: None,
                    metadata: None,
                };
                trace!(?data_artifact, "Created data artifact.");

                if let Some(ref mut artifacts) = task.artifacts {
                    artifacts.push(data_artifact);
                    debug!("Added data artifact to task.");
                }
            }
        } else {
            trace!("No message provided, skipping artifact processing.");
        }

        Ok(())
    }

    /// Get a task by ID
    #[instrument(skip(self, params), fields(task_id = %params.id))]
    pub async fn get_task(&self, params: TaskQueryParams) -> Result<Task, ServerError> {
        debug!("Getting task details."); // Changed to debug
        trace!(?params, "Task query parameters.");
        debug!("Fetching task from repository.");
        let task = self.task_repository.get_task(&params.id).await?
            .ok_or_else(|| {
                warn!("Task not found in repository.");
                ServerError::TaskNotFound(params.id.clone())
            })?;
        debug!(state = ?task.status.state, "Task found in repository."); // Changed to debug
        trace!(?task, "Fetched task details.");

        // Apply history_length filter if specified
        let mut result = task.clone();
        if let Some(history_length) = params.history_length {
            debug!(%history_length, "Applying history length filter.");
            if let Some(history) = &mut result.history {
                let original_len = history.len();
                if original_len > history_length as usize {
                    trace!(original_len, target_len = history_length, "Truncating history.");
                    *history = history.iter()
                        .skip(original_len - history_length as usize)
                        .cloned()
                        .collect();
                    trace!(final_len = history.len(), "History truncated.");
                } else {
                    trace!(original_len, target_len = history_length, "History length is within limit, no truncation needed.");
                }
            } else {
                trace!("Task has no history, filter not applied.");
            }
        } else {
            trace!("No history length specified, returning full history.");
        }

        Ok(result)
    }

    /// Cancel a task
    #[instrument(skip(self, params), fields(task_id = %params.id))]
    pub async fn cancel_task(&self, params: TaskIdParams) -> Result<Task, ServerError> {
        info!("Attempting to cancel task."); // Keep info for cancellation attempt
        trace!(?params, "Cancel task parameters.");
        debug!("Fetching task to check current state.");
        let mut task = self.task_repository.get_task(&params.id).await?
            .ok_or_else(|| {
                 warn!("Task not found for cancellation.");
                 ServerError::TaskNotFound(params.id.clone())
            })?;
        debug!(current_state = ?task.status.state, "Task found."); // Changed to debug
        trace!(?task, "Task details before cancellation.");

        // Check if task can be canceled
        match task.status.state {
            TaskState::Completed | TaskState::Failed | TaskState::Canceled => {
                warn!("Task is already in a final state ({:?}), cannot cancel.", task.status.state);
                return Err(ServerError::TaskNotCancelable(format!(
                    "Task {} is in {} state and cannot be canceled",
                    params.id, task.status.state
                )));
            }
            _ => {
                debug!("Task is in a cancelable state ({:?}). Proceeding with cancellation.", task.status.state); // Changed to debug
            }
        }

        // Update task status
        debug!("Updating task status to Canceled.");
        task.status = TaskStatus {
            state: TaskState::Canceled,
            timestamp: Some(Utc::now()),
            message: Some(Message {
                role: Role::Agent, // Should be Agent role indicating cancellation reason
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "Task canceled by request".to_string(), // More neutral message
                    metadata: None,
                })],
                metadata: None,
            }),
        };
        trace!(?task.status, "Task status updated.");

        // Save the updated task
        debug!("Saving canceled task state to repository.");
        self.task_repository.save_task(&task).await?;

        // Save state history
        debug!("Saving canceled task state to history.");
        self.task_repository.save_state_history(&task.id, &task).await?;

        info!("Task successfully canceled."); // Keep info for success
        Ok(task)
    }

    /// Get task state history
    #[instrument(skip(self), fields(task_id))]
    pub async fn get_task_state_history(&self, task_id: &str) -> Result<Vec<Task>, ServerError> {
        debug!("Getting task state history."); // Changed to debug
        trace!(%task_id, "Task ID for history retrieval.");
        // First check if the task exists
        debug!("Checking if task exists before getting history.");
        let _ = self.task_repository.get_task(task_id).await?
            .ok_or_else(|| {
                warn!("Task not found when trying to get state history.");
                ServerError::TaskNotFound(task_id.to_string())
            })?;
        debug!("Task exists. Retrieving state history from repository.");

        // Retrieve state history
        let history = self.task_repository.get_state_history(task_id).await?;
        debug!(history_len = history.len(), "Retrieved task state history."); // Changed to debug
        trace!(?history, "Full state history details.");
        Ok(history)
    }

    /// Store a task that was created elsewhere (e.g., remote agent) as a read-only mirror.
    /// This allows tracking remote tasks within local sessions.
    #[instrument(skip(self, task), fields(task_id = %task.id, original_state = ?task.status.state))]
    pub async fn import_task(&self, task: Task) -> Result<(), ServerError> {
        debug!("Importing external task into repository."); // Changed to debug
        trace!(?task, "Task details being imported.");
        // Save the task itself
        debug!("Saving imported task to main repository.");
        self.task_repository.save_task(&task).await?;
        // Also save its initial state to history for consistency
        debug!("Saving imported task's initial state to history.");
        self.task_repository.save_state_history(&task.id, &task).await?;
        debug!("Task imported successfully."); // Changed to debug
        Ok(())
    }
    
    /// Set a custom router implementation (for testing)
    pub fn set_router(&self, router: Arc<dyn LlmTaskRouterTrait>) {
        info!("Setting custom router implementation");
        if let Some(ref current_router) = self.task_router {
            // Replace the router with the new implementation
            // This is unsafe but necessary for testing - we're replacing the Arc content
            // We need this for testing purposes only
            unsafe {
                let router_ptr = Arc::as_ptr(current_router) as *mut Arc<dyn LlmTaskRouterTrait>;
                std::ptr::write(router_ptr, router);
            }
            debug!("Router implementation successfully replaced");
        } else {
            warn!("Cannot set router: No router currently configured");
        }
    }
}
