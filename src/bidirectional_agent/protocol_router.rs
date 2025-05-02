// protocol_router.rs - A2A Protocol Compliant JSON-RPC Router
//
// This file provides a unified protocol router that ensures all task operations
// follow the standard A2A JSON-RPC method flow, even when executing locally.
// This maintains protocol compliance and consistency between local and remote execution.

use crate::types::{
    Task, TaskSendParams, TaskStatus, TaskState, Message, Role, Part, TextPart, 
    CancelTaskRequest, GetTaskRequest, TaskIdParams, SendTaskRequest,
};
use crate::bidirectional_agent::{
    error::AgentError,
    tool_executor::ToolExecutor,
    task_metadata::{MetadataManager, TaskOrigin},
    task_router::{self, LlmTaskRouterTrait},
};
use std::sync::Arc;
use serde_json::{json, Value, Map};
use uuid::Uuid;
use log::{debug, info, warn, error};
use async_trait::async_trait;

/// Repository trait abstracting the task data storage
#[async_trait]
pub trait TaskRepository: Send + Sync + 'static {
    /// Save a task to the repository
    async fn save_task(&self, task: &Task) -> Result<(), AgentError>;
    
    /// Get a task by ID
    async fn get_task(&self, id: &str) -> Result<Option<Task>, AgentError>;
    
    /// Save a task state to the history
    async fn save_state_history(&self, task_id: &str, task: &Task) -> Result<(), AgentError>;
    
    /// Get state history for a task
    async fn get_state_history(&self, task_id: &str) -> Result<Vec<Task>, AgentError>;
}

/// Represents a router method in the A2A protocol
pub enum ProtocolMethod {
    /// 'tasks/send' - Create or update a task
    TasksSend,
    
    /// 'tasks/get' - Get task status and details
    TasksGet,
    
    /// 'tasks/cancel' - Cancel a task
    TasksCancel,
    
    /// 'tasks/pushNotification/set' - Set push notification config
    TasksPushNotificationSet,
    
    /// 'tasks/pushNotification/get' - Get push notification config
    TasksPushNotificationGet,
    
    /// 'tasks/sendSubscribe' - Send a task and subscribe to updates (streaming)
    TasksSendSubscribe,
    
    /// 'tasks/resubscribe' - Resubscribe to a task (streaming)
    TasksResubscribe,
}

impl ProtocolMethod {
    /// Convert method to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::TasksSend => "tasks/send",
            Self::TasksGet => "tasks/get",
            Self::TasksCancel => "tasks/cancel",
            Self::TasksPushNotificationSet => "tasks/pushNotification/set",
            Self::TasksPushNotificationGet => "tasks/pushNotification/get",
            Self::TasksSendSubscribe => "tasks/sendSubscribe",
            Self::TasksResubscribe => "tasks/resubscribe",
        }
    }
    
    /// Parse a method string to ProtocolMethod enum
    pub fn from_str(method: &str) -> Option<Self> {
        match method {
            "tasks/send" => Some(Self::TasksSend),
            "tasks/get" => Some(Self::TasksGet),
            "tasks/cancel" => Some(Self::TasksCancel),
            "tasks/pushNotification/set" => Some(Self::TasksPushNotificationSet),
            "tasks/pushNotification/get" => Some(Self::TasksPushNotificationGet),
            "tasks/sendSubscribe" => Some(Self::TasksSendSubscribe),
            "tasks/resubscribe" => Some(Self::TasksResubscribe),
            _ => None,
        }
    }
}

/// Execution strategy for a task
pub enum ExecutionStrategy {
    /// Execute locally with the specified tool
    LocalExecution { tools: Vec<String> },
    
    /// Forward to a remote agent
    Delegation { agent_id: String },
    
    /// Decompose into multiple subtasks
    Decompose { subtask_definitions: Vec<SubtaskDefinition> },
    
    /// Reject with a reason
    Reject { reason: String },
}

/// Definition of a subtask for decomposition
pub struct SubtaskDefinition {
    /// Message content for the subtask
    pub message: String,
    
    /// Additional metadata for the subtask
    pub metadata: Option<Map<String, Value>>,
}

/// Strategy selector for determining how to handle tasks
#[async_trait]
pub trait ExecutionStrategySelector: Send + Sync + 'static {
    /// Determine the execution strategy for a task
    async fn select_strategy(&self, params: &TaskSendParams) -> Result<ExecutionStrategy, AgentError>;
}

/// Protocol-compliant router for A2A methods
pub struct ProtocolRouter {
    /// Task data repository
    task_repository: Arc<dyn TaskRepository>,
    
    /// Strategy selector for deciding task handling - using LlmTaskRouterTrait temporarily
    task_router: Arc<dyn LlmTaskRouterTrait>,
    
    /// Tool executor for local execution
    tool_executor: Arc<ToolExecutor>,
}

impl ProtocolRouter {
    /// Create a new protocol router
    pub fn new(
        task_repository: Arc<dyn TaskRepository>,
        task_router: Arc<dyn LlmTaskRouterTrait>,
        tool_executor: Arc<ToolExecutor>,
    ) -> Self {
        Self {
            task_repository,
            task_router,
            tool_executor,
        }
    }
    
    /// Process an A2A method call with the given method and parameters
    pub async fn handle_method(&self, method: ProtocolMethod, params: Value) -> Result<Value, AgentError> {
        match method {
            ProtocolMethod::TasksSend => {
                // Parse parameters as TaskSendParams
                let send_params = serde_json::from_value::<TaskSendParams>(params)
                    .map_err(|e| AgentError::InvalidRequest(format!("Invalid TaskSendParams: {}", e)))?;
                
                self.handle_tasks_send(send_params).await
            },
            ProtocolMethod::TasksGet => {
                // Parse parameters for get request
                let get_params = serde_json::from_value::<GetTaskRequest>(params)
                    .map_err(|e| AgentError::InvalidRequest(format!("Invalid GetTaskRequest: {}", e)))?;
                
                self.handle_tasks_get(&get_params.params.id).await
            },
            ProtocolMethod::TasksCancel => {
                // Parse parameters for cancel request
                let cancel_params = serde_json::from_value::<CancelTaskRequest>(params)
                    .map_err(|e| AgentError::InvalidRequest(format!("Invalid CancelTaskRequest: {}", e)))?;
                
                self.handle_tasks_cancel(&cancel_params.params.id).await
            },
            // Implement other methods as needed
            _ => Err(AgentError::UnsupportedOperation(format!(
                "Method '{}' not implemented", 
                method.as_str()
            ))),
        }
    }
    
    /// Handle the 'tasks/send' method
    async fn handle_tasks_send(&self, params: TaskSendParams) -> Result<Value, AgentError> {
        debug!("Handling tasks/send for task {}", params.id);
        
        // Check if task already exists
        let existing_task = self.task_repository.get_task(&params.id).await?;
        
        // Handle multi-turn conversations - check if this is a follow-up to an existing task
        if let Some(mut task) = existing_task {
            // This is an update to an existing task (multi-turn conversation)
            
            // Can only update if task is waiting for input
            if task.status.state != TaskState::InputRequired {
                return Err(AgentError::InvalidRequest(format!(
                    "Cannot update task {} because it is not in input-required state (current state: {:?})",
                    params.id, task.status.state
                )));
            }
            
            // Add the new message to history
            if task.history.is_none() {
                task.history = Some(vec![params.message.clone()]);
            } else if let Some(history) = &mut task.history {
                history.push(params.message.clone());
            }
            
            // Update status to working
            task.status = TaskStatus {
                state: TaskState::Working,
                timestamp: Some(chrono::Utc::now()),
                message: None,
            };
            
            // Save updated task
            self.task_repository.save_task(&task).await?;
            self.task_repository.save_state_history(&task.id, &task).await?;
            
            // Continue processing based on strategy
            return self.process_task_with_strategy(task).await;
        }
        
        // This is a new task - create it
        let mut task = Task {
            id: params.id.clone(),
            session_id: params.session_id.clone().or_else(|| Some(Uuid::new_v4().to_string())),
            status: TaskStatus {
                state: TaskState::Submitted,
                timestamp: Some(chrono::Utc::now()),
                message: None,
            },
            history: Some(vec![params.message.clone()]),
            artifacts: None,
            metadata: params.metadata.clone(),
        };
        
        // Initialize task metadata if not present
        if task.metadata.is_none() {
            task.metadata = Some(MetadataManager::create_metadata());
        }
        
        // Save initial task state
        self.task_repository.save_task(&task).await?;
        self.task_repository.save_state_history(&task.id, &task).await?;
        
        // Update status to working
        task.status = TaskStatus {
            state: TaskState::Working,
            timestamp: Some(chrono::Utc::now()),
            message: None,
        };
        
        // Save working state
        self.task_repository.save_task(&task).await?;
        self.task_repository.save_state_history(&task.id, &task).await?;
        
        // Process task based on strategy
        self.process_task_with_strategy(task).await
    }
    
    /// Process a task based on the determined strategy
    async fn process_task_with_strategy(&self, mut task: Task) -> Result<Value, AgentError> {
        // Create task params for strategy selection
        let task_params = TaskSendParams {
            id: task.id.clone(),
            message: task.history.as_ref()
                .and_then(|h| h.last().cloned())
                .ok_or_else(|| AgentError::InvalidRequest("Task has no message history".to_string()))?,
            session_id: task.session_id.clone(),
            metadata: task.metadata.clone(),
            history_length: None,
            push_notification: None,
        };
        
        // Determine execution strategy
        // Instead of using strategy_selector, use the task_router to decide
        let routing_decision = self.task_router.decide(&task_params).await?;
        
        // Map from RoutingDecision to ExecutionStrategy
        let strategy = match routing_decision {
            task_router::RoutingDecision::Local { tool_names } => {
                ExecutionStrategy::LocalExecution { tools: tool_names }
            },
            task_router::RoutingDecision::Remote { agent_id } => {
                ExecutionStrategy::Delegation { agent_id }
            },
            task_router::RoutingDecision::Reject { reason } => {
                ExecutionStrategy::Reject { reason }
            },
            task_router::RoutingDecision::Decompose { subtasks } => {
                // Convert SubtaskDefinition formats
                let proto_subtasks = subtasks.into_iter()
                    .map(|subtask| SubtaskDefinition {
                        message: subtask.input_message,
                        metadata: subtask.metadata,
                    })
                    .collect();
                
                ExecutionStrategy::Decompose { subtask_definitions: proto_subtasks }
            }
        };
        
        match strategy {
            ExecutionStrategy::LocalExecution { tools } => {
                // Set origin as local
                if let Some(metadata) = &mut task.metadata {
                    MetadataManager::set_task_origin(metadata, TaskOrigin::Local);
                }
                
                // Execute locally with the specified tool(s)
                self.tool_executor.execute_task_locally(&mut task, &tools).await?;
                
                // Save updated task and history
                self.task_repository.save_task(&task).await?;
                self.task_repository.save_state_history(&task.id, &task).await?;
                
                // Return task as JSON value
                Ok(serde_json::to_value(task).unwrap_or(json!({})))
            },
            ExecutionStrategy::Delegation { agent_id } => {
                // Handle remote execution
                // In a real implementation, this would delegate to the ClientManager
                // Using placeholder for now
                Err(AgentError::UnsupportedOperation(
                    format!("Remote execution to agent {} not implemented in this example", agent_id)
                ))
            },
            ExecutionStrategy::Decompose { subtask_definitions } => {
                // Handle decomposition
                // In a real implementation, this would create subtasks
                // Using placeholder for now
                Err(AgentError::UnsupportedOperation(
                    "Task decomposition not implemented in this example".to_string()
                ))
            },
            ExecutionStrategy::Reject { reason } => {
                // Update task status to failed with the rejection reason
                task.status = TaskStatus {
                    state: TaskState::Failed,
                    timestamp: Some(chrono::Utc::now()),
                    message: Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: reason,
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
                
                // Save rejected task
                self.task_repository.save_task(&task).await?;
                self.task_repository.save_state_history(&task.id, &task).await?;
                
                // Return task as JSON value
                Ok(serde_json::to_value(task).unwrap_or(json!({})))
            },
        }
    }
    
    /// Handle the 'tasks/get' method
    async fn handle_tasks_get(&self, task_id: &str) -> Result<Value, AgentError> {
        debug!("Handling tasks/get for task {}", task_id);
        
        // Get task from repository
        let task = self.task_repository.get_task(task_id).await?
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))?;
        
        // Return task as JSON value
        Ok(serde_json::to_value(task).unwrap_or(json!({})))
    }
    
    /// Handle the 'tasks/cancel' method
    async fn handle_tasks_cancel(&self, task_id: &str) -> Result<Value, AgentError> {
        debug!("Handling tasks/cancel for task {}", task_id);
        
        // Get task from repository
        let mut task = self.task_repository.get_task(task_id).await?
            .ok_or_else(|| AgentError::TaskNotFound(task_id.to_string()))?;
        
        // Check if task can be canceled (not in a final state)
        match task.status.state {
            TaskState::Completed | TaskState::Failed | TaskState::Canceled => {
                return Err(AgentError::TaskNotCancelable(format!(
                    "Task {} is already in final state: {:?}", 
                    task_id, task.status.state
                )));
            }
            _ => {
                // Update status to canceled
                task.status = TaskStatus {
                    state: TaskState::Canceled,
                    timestamp: Some(chrono::Utc::now()),
                    message: Some(Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: "Task canceled by request".to_string(),
                            metadata: None,
                        })],
                        metadata: None,
                    }),
                };
                
                // Save canceled task
                self.task_repository.save_task(&task).await?;
                self.task_repository.save_state_history(&task.id, &task).await?;
                
                // Return task as JSON value
                Ok(serde_json::to_value(task).unwrap_or(json!({})))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    
    // Mock implementations for testing
    
    /// Mock task repository
    struct MockTaskRepository {
        tasks: Mutex<HashMap<String, Task>>,
        history: Mutex<HashMap<String, Vec<Task>>>,
    }
    
    impl MockTaskRepository {
        fn new() -> Self {
            Self {
                tasks: Mutex::new(HashMap::new()),
                history: Mutex::new(HashMap::new()),
            }
        }
    }
    
    #[async_trait]
    impl TaskRepository for MockTaskRepository {
        async fn save_task(&self, task: &Task) -> Result<(), AgentError> {
            let mut tasks = self.tasks.lock().unwrap();
            tasks.insert(task.id.clone(), task.clone());
            Ok(())
        }
        
        async fn get_task(&self, id: &str) -> Result<Option<Task>, AgentError> {
            let tasks = self.tasks.lock().unwrap();
            Ok(tasks.get(id).cloned())
        }
        
        async fn save_state_history(&self, task_id: &str, task: &Task) -> Result<(), AgentError> {
            let mut history = self.history.lock().unwrap();
            let task_history = history.entry(task_id.to_string()).or_insert_with(Vec::new);
            task_history.push(task.clone());
            Ok(())
        }
        
        async fn get_state_history(&self, task_id: &str) -> Result<Vec<Task>, AgentError> {
            let history = self.history.lock().unwrap();
            Ok(history.get(task_id).cloned().unwrap_or_default())
        }
    }
    
    /// Mock strategy selector that always chooses local execution
    struct MockStrategySelector;
    
    #[async_trait]
    impl ExecutionStrategySelector for MockStrategySelector {
        async fn select_strategy(&self, _params: &TaskSendParams) -> Result<ExecutionStrategy, AgentError> {
            Ok(ExecutionStrategy::LocalExecution {
                tools: vec!["mock_tool".to_string()],
            })
        }
    }
    
    // Tests for ProtocolRouter
    
    #[tokio::test]
    async fn test_handle_tasks_send_new_task() {
        // Create mock components
        let repository = Arc::new(MockTaskRepository::new());
        let selector = Arc::new(MockStrategySelector);
        
        // Create mock tool executor
        // In a real test, you would use a mock ToolExecutor implementation
        let tool_executor = Arc::new(ToolExecutor::new(
            Arc::new(crate::bidirectional_agent::agent_directory::AgentDirectory::new(&Default::default()).await.unwrap()),
        ));
        
        // Create protocol router
        let router = ProtocolRouter::new(repository.clone(), selector, tool_executor);
        
        // Create task params
        let task_id = "test-task-1";
        let params = TaskSendParams {
            id: task_id.to_string(),
            message: Message {
                role: Role::User,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "Test message".to_string(),
                    metadata: None,
                })],
                metadata: None,
            },
            session_id: None,
            metadata: None,
            history_length: None,
            push_notification: None,
        };
        
        // Handle tasks/send method
        let result = router.handle_tasks_send(params).await;
        
        // Verify task was created
        assert!(result.is_ok());
        
        // Check task in repository
        let task = repository.get_task(task_id).await.unwrap().unwrap();
        assert_eq!(task.id, task_id);
        
        // Check task history was saved
        let history = repository.get_state_history(task_id).await.unwrap();
        assert!(!history.is_empty());
    }
    
    #[tokio::test]
    async fn test_handle_tasks_get() {
        // Create mock components
        let repository = Arc::new(MockTaskRepository::new());
        let selector = Arc::new(MockStrategySelector);
        
        // Create mock tool executor
        let tool_executor = Arc::new(ToolExecutor::new(
            Arc::new(crate::bidirectional_agent::agent_directory::AgentDirectory::new(&Default::default()).await.unwrap()),
        ));
        
        // Create protocol router
        let router = ProtocolRouter::new(repository.clone(), selector, tool_executor);
        
        // Create a task in the repository
        let task_id = "test-task-2";
        let task = Task {
            id: task_id.to_string(),
            session_id: Some("test-session".to_string()),
            status: TaskStatus {
                state: TaskState::Completed,
                timestamp: Some(chrono::Utc::now()),
                message: None,
            },
            history: None,
            artifacts: None,
            metadata: None,
        };
        repository.save_task(&task).await.unwrap();
        
        // Test the get method
        let result = router.handle_tasks_get(task_id).await;
        
        // Verify result
        assert!(result.is_ok());
        let result_json = result.unwrap();
        let result_task: Task = serde_json::from_value(result_json).unwrap();
        assert_eq!(result_task.id, task_id);
        assert_eq!(result_task.status.state, TaskState::Completed);
    }
    
    #[tokio::test]
    async fn test_handle_tasks_cancel() {
        // Create mock components
        let repository = Arc::new(MockTaskRepository::new());
        let selector = Arc::new(MockStrategySelector);
        
        // Create mock tool executor
        let tool_executor = Arc::new(ToolExecutor::new(
            Arc::new(crate::bidirectional_agent::agent_directory::AgentDirectory::new(&Default::default()).await.unwrap()),
        ));
        
        // Create protocol router
        let router = ProtocolRouter::new(repository.clone(), selector, tool_executor);
        
        // Create a task in the repository
        let task_id = "test-task-3";
        let task = Task {
            id: task_id.to_string(),
            session_id: Some("test-session".to_string()),
            status: TaskStatus {
                state: TaskState::Working,
                timestamp: Some(chrono::Utc::now()),
                message: None,
            },
            history: None,
            artifacts: None,
            metadata: None,
        };
        repository.save_task(&task).await.unwrap();
        
        // Test the cancel method
        let result = router.handle_tasks_cancel(task_id).await;
        
        // Verify result
        assert!(result.is_ok());
        
        // Check task was updated
        let updated_task = repository.get_task(task_id).await.unwrap().unwrap();
        assert_eq!(updated_task.status.state, TaskState::Canceled);
    }
}