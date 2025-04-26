use hyper::{Body, Request, Response, Server, StatusCode};
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::service::{make_service_fn, service_fn};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use futures_util::stream::{self, StreamExt};
use serde_json::{json, Value, Map};
use crate::client::errors::{A2aError, error_codes};
use crate::types::{
    AgentCard, AgentSkill, AgentCapabilities, AgentAuthentication, 
    PushNotificationConfig, TaskPushNotificationConfig, AuthenticationInfo,
    Part, TextPart, FilePart, DataPart, FileContent, Artifact, Role, Message,
    TaskStatus, TaskState
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::{Utc, DateTime};

// Task information storage for the mock server
#[derive(Debug, Clone)]
struct MockTask {
    id: String,
    session_id: String,
    current_status: TaskStatus,
    state_history: Vec<TaskStatus>,
    artifacts: Vec<Artifact>,
    // State machine simulation configuration
    simulation_config: Option<TaskSimulationConfig>,
    // Channel for signaling when input is provided (for InputRequired state)
    input_received: bool,
    // Streaming configuration for dynamic streaming content
    streaming_config: Option<StreamingConfig>,
}

/// Configuration for simulating task state machine behavior
#[derive(Debug, Clone)]
struct TaskSimulationConfig {
    /// Duration of the task processing in milliseconds
    duration_ms: u64,
    /// Whether the task requires input to proceed
    require_input: bool,
    /// Whether the task should fail
    should_fail: bool,
    /// Failure message (if should_fail is true)
    fail_message: Option<String>,
    /// Timestamp when the task started processing
    start_time: DateTime<Utc>,
    /// Timestamp when the task is supposed to finish
    finish_time: Option<DateTime<Utc>>,
}

/// Configuration for dynamic streaming content
#[derive(Debug, Clone)]
struct StreamingConfig {
    /// Number of text chunks to generate
    text_chunks: u64,
    /// Types of artifacts to generate (text, data, file)
    artifact_types: Vec<String>,
    /// Delay between sending chunks in milliseconds
    chunk_delay_ms: u64,
    /// Final state of the stream (completed, failed)
    final_state: String,
}

impl MockTask {
    fn new(id: &str, session_id: &str) -> Self {
        // Create initial status
        let initial_status = TaskStatus {
            state: TaskState::Submitted,
            timestamp: Some(Utc::now()),
            message: None,
        };
        
        Self {
            id: id.to_string(),
            session_id: session_id.to_string(),
            current_status: initial_status.clone(),
            state_history: vec![initial_status],
            artifacts: Vec::new(),
            simulation_config: None,
            input_received: false,
            streaming_config: None,
        }
    }
    
    /// Create a task with simulation configuration
    fn with_simulation(id: &str, session_id: &str, duration_ms: u64, require_input: bool, should_fail: bool, fail_message: Option<String>) -> Self {
        let mut task = Self::new(id, session_id);
        
        let now = Utc::now();
        let simulation_config = TaskSimulationConfig {
            duration_ms,
            require_input,
            should_fail,
            fail_message,
            start_time: now,
            finish_time: None, // Will be calculated when state machine starts
        };
        
        task.simulation_config = Some(simulation_config);
        task
    }
    
    /// Create a task with streaming configuration
    fn with_streaming_config(id: &str, session_id: &str, text_chunks: u64, artifact_types: Vec<String>, chunk_delay_ms: u64, final_state: String) -> Self {
        let mut task = Self::new(id, session_id);
        
        let streaming_config = StreamingConfig {
            text_chunks,
            artifact_types,
            chunk_delay_ms,
            final_state,
        };
        
        task.streaming_config = Some(streaming_config);
        task
    }
    
    // Update the task's status, preserving history
    fn update_status(&mut self, new_state: TaskState, message: Option<Message>) {
        let new_status = TaskStatus {
            state: new_state,
            timestamp: Some(Utc::now()),
            message,
        };
        
        // Add current status to history before updating
        self.state_history.push(self.current_status.clone());
        
        // Update current status
        self.current_status = new_status;
    }
    
    // Add an artifact to the task
    fn add_artifact(&mut self, artifact: Artifact) {
        self.artifacts.push(artifact);
    }
    
    // Convert to JSON response
    fn to_json(&self, include_history: bool) -> Value {
        let mut task_json = json!({
            "id": self.id,
            "sessionId": self.session_id,
            "status": self.current_status,
            "artifacts": self.artifacts
        });
        
        // Only include state history if requested
        if include_history && !self.state_history.is_empty() {
            // Keep the history field as required by the client code
            if !self.state_history.is_empty() {
                // Create synthetic message history
                let mut messages = Vec::new();
                for status in &self.state_history {
                    // Add a message for each state
                    let role = if status.state == TaskState::Submitted {
                        Role::User
                    } else {
                        Role::Agent
                    };
                    
                    // Create default message parts if none exist
                    let text = match status.state {
                        TaskState::Submitted => "Initial user request",
                        TaskState::Working => "Working on your request...",
                        TaskState::InputRequired => "Need more information to proceed.",
                        TaskState::Completed => "Task completed successfully!",
                        TaskState::Canceled => "Task has been canceled.",
                        TaskState::Failed => "Task failed to complete.",
                        TaskState::Unknown => "Unknown state.",
                    };
                    
                    let message = if let Some(msg) = &status.message {
                        msg.clone()
                    } else {
                        // Create a default message
                        Message {
                            role,
                            parts: vec![Part::TextPart(TextPart {
                                type_: "text".to_string(),
                                text: text.to_string(),
                                metadata: None,
                            })],
                            metadata: None,
                        }
                    };
                    
                    messages.push(message);
                }
                task_json["history"] = json!(messages);
            }
        }
        
        task_json
    }
}

// Removed MockBatch struct and related functions

// Removed MockFile struct and related functions

// Global task storage
type TaskStorage = Arc<Mutex<HashMap<String, MockTask>>>;

// Removed BatchStorage type alias
// Removed FileStorage type alias

// Create a new task storage
fn create_task_storage() -> TaskStorage {
    Arc::new(Mutex::new(HashMap::new()))
}

// Removed create_batch_storage function
// Removed create_file_storage function

// Create agent card for the mock server
fn create_agent_card() -> AgentCard {
    create_agent_card_with_auth(true)
}

// Create agent card with configurable authentication requirement
fn create_agent_card_with_auth(require_auth: bool) -> AgentCard {
    let skill = AgentSkill {
        id: "test-skill-1".to_string(),
        name: "Echo".to_string(),
        description: Some("Echoes back any message sent".to_string()),
        tags: None,
        examples: None,
        input_modes: None,
        output_modes: None,
    };
    
    let capabilities = AgentCapabilities {
        streaming: true,
        push_notifications: true,
        state_transition_history: true,
    };
    
    // Configure authentication based on the require_auth parameter
    let authentication = if require_auth {
        // For authentication error tests, use strict auth schemes
        // Check if this is port 8097, which is used by auth error tests
        let schemes = if std::thread::current().name().unwrap_or("").contains("auth_test") {
            vec!["Bearer".to_string(), "ApiKey".to_string()]
        } else {
            // For regular tests, use "None" to include auth info in card but not actually require it
            vec!["None".to_string()]
        };
        
        Some(AgentAuthentication {
            schemes,
            credentials: None,
        })
    } else {
        None
    };
    
    AgentCard {
        name: "Mock A2A Server".to_string(),
        description: Some("A mock server for testing A2A protocol clients".to_string()),
        url: "http://localhost:8080".to_string(),
        provider: None,
        version: "0.1.0".to_string(),
        documentation_url: None,
        capabilities,
        authentication,
        default_input_modes: vec!["text/plain".to_string()],
        default_output_modes: vec!["text/plain".to_string()],
        skills: vec![skill],
    }
}

// Helper function to create standard error responses using A2aError
fn create_error_response(id: Option<&Value>, code: i64, message: &str, data: Option<Value>) -> Value {
    // Create a proper A2aError instance
    let a2a_error = A2aError::new(code, message, data.clone());
    
    // Create the standard JSON-RPC error response format
    let mut error = json!({
        "code": a2a_error.code,
        "message": a2a_error.message
    });
    
    if let Some(error_data) = data {
        if let Some(obj) = error.as_object_mut() {
            obj.insert("data".to_string(), error_data);
        }
    }
    
    json!({
        "jsonrpc": "2.0",
        "id": id.unwrap_or(&Value::Null),
        "error": error
    })
}

// Removed unused batch_storage and file_storage parameters
async fn handle_a2a_request(task_storage: TaskStorage, req: Request<Body>) -> Result<Response<Body>, Infallible> {
    handle_a2a_request_with_auth(task_storage, req, true).await
}

// Mock handlers for A2A endpoints with optional authentication
// Removed unused batch_storage and file_storage parameters
async fn handle_a2a_request_with_auth(task_storage: TaskStorage, mut req: Request<Body>, require_auth: bool) -> Result<Response<Body>, Infallible> {
    // Check if this is a request for agent card
    // Check for Accept header to see if client wants SSE
    let accept_header = req.headers().get("Accept")
                       .and_then(|h| h.to_str().ok())
                       .unwrap_or("");
    
    // Check if this is a request for agent card which doesn't need auth
    if req.uri().path() == "/.well-known/agent.json" {
        // Get the agent card
        let agent_card = create_agent_card();
        let json = serde_json::to_string(&agent_card).unwrap();
        return Ok(Response::new(Body::from(json)));
    }
                       
    // First get the agent card to check required auth
    let agent_card = create_agent_card_with_auth(require_auth);
    
    // Handle JSON-RPC requests - extract the body first but keep the headers
    let uri = req.uri().clone();
    let headers = req.headers().clone();
    let body_bytes = hyper::body::to_bytes(req.body_mut()).await.unwrap();
    
    // For testing purposes, extract the method from request body if available
    let method_name = match serde_json::from_slice::<Value>(&body_bytes) {
        Ok(json) => {
            let method = json.get("method").and_then(|m| m.as_str()).unwrap_or("");
            method.to_string()
        },
        Err(_) => "".to_string()
    };
    
    // For integration testing, make these methods bypass authentication
    // Removed "skills", "batches" from bypass list
    let bypass_auth_methods = ["state"];
    let should_bypass_auth = bypass_auth_methods.iter().any(|&m| method_name.contains(m));
    
    // Special handling for authentication test at port 8097
    let thread_name = std::thread::current().name().unwrap_or("").to_string();
    let is_auth_thread = thread_name.contains("auth_test");
    
    // Extract port from request URL (more reliable than environment variable)
    let req_url = req.uri().to_string();
    let is_auth_test = is_auth_thread || req_url.contains(":8097");
    
    println!("DEBUG: Thread name: {}, URL: {}, Is auth test: {}", thread_name, req_url, is_auth_test);
    
    // Check if endpoint requires authentication for auth test
    if is_auth_test && agent_card.authentication.is_some() {
        // Skip auth checks for agent card endpoint
        if req.uri().path() == "/.well-known/agent.json" {
            // Allow agent card access without auth
        } else {
            // Special handling - require authentication for the auth test
            let mut auth_valid = false;
            
            println!("AUTH TEST: Checking authentication headers");
            
            // Check for bearer auth
            let auth_header = headers.get("Authorization");
            if let Some(header) = auth_header {
                let header_str = header.to_str().unwrap_or("");
                println!("AUTH TEST: Found Authorization header: {}", header_str);
                if header_str.starts_with("Bearer ") {
                    auth_valid = true;
                }
            } else {
                println!("AUTH TEST: No Authorization header found");
            }
            
            // Check for API key auth
            if !auth_valid {
                let api_key = headers.get("X-API-Key");
                if let Some(key) = api_key {
                    println!("AUTH TEST: Found X-API-Key header: {}", key.to_str().unwrap_or(""));
                    auth_valid = true;
                } else {
                    println!("AUTH TEST: No X-API-Key header found");
                }
            }
            
            // If no valid auth was found, return unauthorized error
            if !auth_valid {
                println!("AUTH TEST: No valid authentication found, returning 401");
                
                // Always return 401 for unauthorized requests in auth test mode
                return Ok(Response::builder()
                    .status(401)
                    .header("content-type", "application/json")
                    .body(Body::from(create_error_response(
                        None,
                        error_codes::ERROR_INVALID_REQUEST,
                        "Unauthorized request",
                        None
                    ).to_string()))
                    .unwrap());
            }
        }
    }
    
    // Parse the request body as JSON
    let request: Value = match serde_json::from_slice(&body_bytes) {
        Ok(req) => req,
        Err(e) => {
            let error_response = create_error_response(
                None,
                error_codes::ERROR_PARSE,
                &format!("Invalid JSON payload: {}", e),
                Some(json!({
                    "help": "Please ensure that your request contains valid JSON",
                    "example": {
                        "jsonrpc": "2.0",
                        "id": "request-id",
                        "method": "tasks/send",
                        "params": {
                            "message": {
                                "role": "user",
                                "parts": [{"type": "text", "text": "Hello world"}]
                            }
                        }
                    }
                }))
            );
            let json = serde_json::to_string(&error_response).unwrap();
            return Ok(Response::new(Body::from(json)));
        }
    };
    
    // Extract message content for tasks/send methods to help respond appropriately
    let message_opt = if request.get("method").and_then(|m| m.as_str()).unwrap_or("") == "tasks/send" {
        request.get("params")
            .and_then(|p| p.get("message"))
            .cloned()
    } else {
        None
    };
    
    // Check method to determine response
    if let Some(method) = request.get("method").and_then(|m| m.as_str()) {
        match method {
            // Removed non-standard "auth/validate" endpoint
            "tasks/send" => {
                // Apply configurable delay if specified
                let params = request.get("params");
                apply_mock_delay(&params).await;
                
                // Explicitly check for ID in direct params first
                let task_id = params
                    .and_then(|p| p.get("id"))
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
                    // Check ID in metadata if not found directly
                    .or_else(|| params
                        .and_then(|p| p.get("metadata"))
                        .and_then(|m| m.get("id"))
                        .and_then(|id| id.as_str())
                        .map(|s| s.to_string()))
                    // Generate a random ID if not found anywhere
                    .unwrap_or_else(|| format!("mock-task-{}", chrono::Utc::now().timestamp_millis()));
                    
                // Print the task ID we're using for debugging
                println!("📋 Using task ID: {}", task_id);
                
                let session_id = params
                    .and_then(|p| p.get("sessionId"))
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("mock-session-{}", chrono::Utc::now().timestamp_millis()));
                
                // Check if this is a follow-up message for an existing task in InputRequired state
                let is_followup = {
                    let storage = task_storage.lock().unwrap();
                    if let Some(existing_task) = storage.get(&task_id) {
                        existing_task.current_status.state == TaskState::InputRequired
                    } else {
                        false
                    }
                };
                
                // If this is a follow-up message, mark the task as having received input
                if is_followup {
                    // Update the task to indicate input has been received
                    // Also immediately transition to Working state for better test behavior
                    {
                        let mut storage = task_storage.lock().unwrap();
                        if let Some(existing_task) = storage.get_mut(&task_id) {
                            // Mark input as received
                            existing_task.input_received = true;
                            println!("📬 Received follow-up input for task {}", task_id);
                            
                            // Immediately transition to Working state
                            let working_message = Message {
                                role: Role::Agent,
                                parts: vec![Part::TextPart(TextPart {
                                    type_: "text".to_string(),
                                    text: "Thanks for the additional information. Continuing processing...".to_string(),
                                    metadata: None,
                                })],
                                metadata: None,
                            };
                            
                            existing_task.update_status(TaskState::Working, Some(working_message));
                            println!("📝 Task {} transitioned to Working state after receiving input", task_id);
                        }
                    }
                    
                    // Get the updated task to return
                    let task_json = {
                        let storage = task_storage.lock().unwrap();
                        if let Some(task) = storage.get(&task_id) {
                            // Don't include history in the follow-up response
                            task.to_json(false)
                        } else {
                            // This shouldn't happen
                            json!({
                                "id": task_id,
                                "sessionId": session_id,
                                "status": {
                                    "state": "working",
                                    "timestamp": chrono::Utc::now().to_rfc3339()
                                }
                            })
                        }
                    };
                    
                    // Create a response with the updated task state
                    let response = json!({
                        "jsonrpc": "2.0",
                        "id": request.get("id"),
                        "result": task_json
                    });
                    
                    let json = serde_json::to_string(&response).unwrap();
                    return Ok(Response::new(Body::from(json)));
                }
                
                // This is a new task - check for simulation config in the metadata
                let simulation_config = parse_task_simulation_config(&params);
                
                // Create response artifacts based on message content
                let artifacts = create_response_artifacts(&message_opt);
                
                // Create and store the task
                let mut task = match &simulation_config {
                    Some((duration_ms, require_input, should_fail, fail_message)) => {
                        // Create a task with simulation configuration
                        // Clone the fail_message to avoid ownership issues
                        let fail_message_clone = fail_message.clone();
                        
                        let task = MockTask::with_simulation(
                            &task_id,
                            &session_id,
                            *duration_ms,
                            *require_input,
                            *should_fail,
                            fail_message_clone
                        );
                        
                        // Store the task
                        {
                            let mut storage = task_storage.lock().unwrap();
                            
                            // Add artifacts to the task
                            let mut task_with_artifacts = task.clone();
                            for artifact in artifacts {
                                task_with_artifacts.add_artifact(artifact);
                            }
                            
                            storage.insert(task_id.clone(), task_with_artifacts.clone());
                            task_with_artifacts
                        }
                    },
                    None => {
                        // No simulation - create a regular task that completes immediately
                        let mut task = MockTask::new(&task_id, &session_id);
                        
                        // Store the task with immediate state transitions
                        {
                            let mut storage = task_storage.lock().unwrap();
                            
                            // Update with working state
                            task.update_status(TaskState::Working, None);
                            
                            // Add artifacts
                            for artifact in artifacts {
                                task.add_artifact(artifact);
                            }
                            
                            // Update to completed state
                            let completed_message = Message {
                                role: Role::Agent,
                                parts: vec![Part::TextPart(TextPart {
                                    type_: "text".to_string(),
                                    text: "Task completed successfully!".to_string(),
                                    metadata: None,
                                })],
                                metadata: None,
                            };
                            task.update_status(TaskState::Completed, Some(completed_message));
                            
                            // Store the task
                            storage.insert(task_id.clone(), task.clone());
                            task
                        }
                    }
                };
                
                // For simulated tasks, spawn a background task to manage the lifecycle
                if let Some((duration_ms, require_input, should_fail, fail_message)) = &simulation_config {
                    // Clone task_id and task_storage for the spawned task
                    let task_id_clone = task_id.clone();
                    let storage_clone = task_storage.clone();
                    let duration_ms_clone = *duration_ms;
                    let require_input_clone = *require_input;
                    let should_fail_clone = *should_fail;
                    let fail_message_clone = fail_message.clone();
                    
                    // Spawn the lifecycle simulation task
                    tokio::spawn(async move {
                        simulate_task_lifecycle(
                            task_id_clone,
                            storage_clone,
                            duration_ms_clone,
                            require_input_clone,
                            should_fail_clone,
                            fail_message_clone
                        ).await;
                    });
                }
                
                // Create a SendTaskResponse with a Task - initially in Submitted state
                let response = if simulation_config.is_some() {
                    // For simulated tasks, return just the minimal task in Submitted state
                    json!({
                        "jsonrpc": "2.0",
                        "id": request.get("id"),
                        "result": {
                            "id": task_id,
                            "sessionId": session_id,
                            "status": {
                                "state": "submitted",
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            },
                            "artifacts": task.artifacts
                        }
                    })
                } else {
                    // For non-simulated tasks, return the complete task (already completed)
                    json!({
                        "jsonrpc": "2.0",
                        "id": request.get("id"),
                        "result": task.to_json(false)
                    })
                };
                
                let json = serde_json::to_string(&response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "tasks/get" => {
                // Apply configurable delay if specified
                let params = request.get("params");
                apply_mock_delay(&params).await;
                
                // Extract task ID from request params
                let task_id_opt = params.and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let task_id = match task_id_opt {
                    Some(id) => id.to_string(), // Clone the string to avoid borrowing request
                    None => {
                        let error = create_error_response(
                            request.get("id"),
                            error_codes::ERROR_INVALID_PARAMS,
                            "Invalid parameters: missing required 'id' parameter for task",
                            None
                        );
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Check if we should include history
                let history_length = request.get("params")
                    .and_then(|p| p.get("historyLength"))
                    .and_then(|h| h.as_i64());
                
                // Include full history if historyLength is null or not specified
                let include_history = history_length.is_none() || history_length.unwrap_or(0) > 0;
                
                // Check if task exists in our storage
                let task_response = {
                    let storage = task_storage.lock().unwrap();
                    if let Some(task) = storage.get(&task_id) {
                        // Return the task with history if requested
                        json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "result": task.to_json(include_history)
                        })
                    } else {
                        // Return task not found error with specific details
                        create_error_response(
                            request.get("id"),
                            error_codes::ERROR_TASK_NOT_FOUND,
                            &format!("Task not found: {}", task_id),
                            None
                        )
                    }
                };
                
                let json = serde_json::to_string(&task_response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "tasks/cancel" => {
                // Apply configurable delay if specified
                let params = request.get("params");
                apply_mock_delay(&params).await;
                
                // Extract task ID from request params
                let task_id_opt = params.and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let task_id = match task_id_opt {
                    Some(id) => id.to_string(), // Clone the string to avoid borrowing request
                    None => {
                        let error = create_error_response(
                            request.get("id"),
                            error_codes::ERROR_INVALID_PARAMS,
                            "Invalid parameters: missing required 'id' parameter for task",
                            None
                        );
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Update task state to canceled if it exists
                let task_json = {
                    let mut storage = task_storage.lock().unwrap();
                    if let Some(task) = storage.get_mut(&task_id) {
                        // Special handling for error_handling tests
                        // If we're in a testing context and the task is completed, we need
                        // to check if this is a test specifically for the "not cancelable" error
                        
                        if task.current_status.state == TaskState::Completed && 
                           (task_id.starts_with("test-task-not-cancelable") || 
                           request.get("params").and_then(|p| p.get("test_error")).is_some()) {
                            // For test cases that specifically test error handling, return the expected error
                            // Provide more detailed message that includes the task state
                            return Ok(Response::new(Body::from(create_error_response(
                                request.get("id"),
                                error_codes::ERROR_TASK_NOT_CANCELABLE,
                                &format!("Task cannot be canceled: {} is in {} state", 
                                    task_id, task.current_status.state),
                                None
                            ).to_string())));
                        }
                        
                        // For regular tests, allow canceling any task
                        // Note: In a real implementation, you would typically check cancelable state
                        /*
                        if task.current_status.state == TaskState::Completed || 
                           task.current_status.state == TaskState::Failed ||
                           task.current_status.state == TaskState::Canceled {
                            // Return error - task cannot be canceled
                            return Ok(Response::new(Body::from(create_error_response(
                                request.get("id"),
                                error_codes::ERROR_TASK_NOT_CANCELABLE,
                                &format!("Task cannot be canceled: {} is in {} state", 
                                    task_id, task.current_status.state),
                                None
                            ).to_string())));
                        }
                        */
                        
                        // Update with canceled state
                        let canceled_message = Message {
                            role: Role::Agent,
                            parts: vec![Part::TextPart(TextPart {
                                type_: "text".to_string(),
                                text: "Task canceled by user request".to_string(),
                                metadata: None,
                            })],
                            metadata: None,
                        };
                        task.update_status(TaskState::Canceled, Some(canceled_message));
                        
                        // Return updated task
                        task.to_json(false) // Don't include history in cancel response
                    } else {
                        // Return task not found error using specific A2A error
                        return Ok(Response::new(Body::from(create_error_response(
                            request.get("id"),
                            error_codes::ERROR_TASK_NOT_FOUND,
                            &format!("Task not found: {}", task_id),
                            None
                        ).to_string())));
                    }
                };
                
                // Return success response
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": task_json
                });
                
                let json = serde_json::to_string(&response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "tasks/sendSubscribe" => {
                // Apply configurable delay if specified for the initial response
                let params = request.get("params");
                apply_mock_delay(&params).await;
                
                // Parse streaming configuration if specified
                let streaming_config = parse_streaming_config(&params);
                
                // Extract the configuration parameters or use defaults
                let (text_chunks, artifact_types, chunk_delay_ms, final_state) = streaming_config
                    .unwrap_or((5, vec!["text".to_string(), "data".to_string(), "file".to_string()], 300, "completed".to_string()));
                
                // This is a streaming endpoint, respond with SSE
                let id = request.get("id").unwrap_or(&json!(null)).clone();
                let task_id = format!("stream-task-{}", chrono::Utc::now().timestamp_millis());
                let session_id = format!("stream-session-{}", chrono::Utc::now().timestamp_millis());
                
                // Create a new task for this streaming request with streaming configuration
                let mut streaming_task = MockTask::with_streaming_config(
                    &task_id, 
                    &session_id, 
                    text_chunks, 
                    artifact_types.clone(), 
                    chunk_delay_ms, 
                    final_state.clone()
                );
                
                // Create a streaming channel
                let (tx, rx) = mpsc::channel::<String>(32);
                
                // Create a clone of task_storage for the spawned task
                let storage_clone = task_storage.clone();
                
                // Clone values for the spawned task
                let chunk_delay = chunk_delay_ms;
                let task_types = artifact_types.clone();
                let num_text_chunks = text_chunks;
                let final_task_state = final_state.clone();
                
                // Spawn a task to generate streaming events
                tokio::spawn(async move {
                    // Update task status to working
                    streaming_task.update_status(TaskState::Working, None);
                    
                    // Create initial working status update
                    let status_update = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "id": task_id,
                            "sessionId": session_id,
                            "status": streaming_task.current_status,
                            "final": false
                        }
                    });
                    
                    // Store task in the global task storage
                    {
                        let mut storage = storage_clone.lock().unwrap();
                        storage.insert(task_id.clone(), streaming_task.clone());
                    }
                    
                    // Send status update
                    let _ = tx.send(format!("data: {}\n\n", status_update.to_string())).await;
                    sleep(Duration::from_millis(chunk_delay)).await;
                    
                    // Keep track of the total artifacts generated
                    let mut artifact_count = 0;
                    
                    // Generate text chunks if configured
                    if task_types.contains(&"text".to_string()) {
                        // Generate text chunks based on configuration
                        for i in 0..num_text_chunks {
                            let is_last = i == num_text_chunks - 1;
                            let append = i > 0;
                            
                            // Generate and send text artifact
                            let (artifact, artifact_update, _) = generate_mock_text_artifact(
                                &task_id, i as usize, is_last, append
                            );
                            
                            // Save artifact to task
                            {
                                let mut storage = storage_clone.lock().unwrap();
                                if let Some(task) = storage.get_mut(&task_id) {
                                    task.add_artifact(artifact);
                                }
                            }
                            
                            // Send artifact update
                            let _ = tx.send(format!("data: {}\n\n", artifact_update.to_string())).await;
                            sleep(Duration::from_millis(chunk_delay)).await;
                            
                            artifact_count += 1;
                        }
                    }
                    
                    // Generate data artifact if configured
                    if task_types.contains(&"data".to_string()) {
                        // Generate and send data artifact
                        let (artifact, artifact_update, _) = generate_mock_data_artifact(
                            &task_id, artifact_count as usize, true
                        );
                        
                        // Save artifact to task
                        {
                            let mut storage = storage_clone.lock().unwrap();
                            if let Some(task) = storage.get_mut(&task_id) {
                                task.add_artifact(artifact);
                            }
                        }
                        
                        // Send artifact update
                        let _ = tx.send(format!("data: {}\n\n", artifact_update.to_string())).await;
                        sleep(Duration::from_millis(chunk_delay)).await;
                        
                        artifact_count += 1;
                    }
                    
                    // Generate file artifact if configured
                    if task_types.contains(&"file".to_string()) {
                        // Generate and send file artifact
                        let (artifact, artifact_update, _) = generate_mock_file_artifact(
                            &task_id, artifact_count as usize, true
                        );
                        
                        // Save artifact to task
                        {
                            let mut storage = storage_clone.lock().unwrap();
                            if let Some(task) = storage.get_mut(&task_id) {
                                task.add_artifact(artifact);
                            }
                        }
                        
                        // Send artifact update
                        let _ = tx.send(format!("data: {}\n\n", artifact_update.to_string())).await;
                        sleep(Duration::from_millis(chunk_delay)).await;
                        
                        artifact_count += 1;
                    }
                    
                    // Final delay before sending status update
                    sleep(Duration::from_millis(chunk_delay)).await;
                    
                    // Determine final state based on configuration
                    let final_state_str = match final_task_state.as_str() {
                        "failed" => {
                            // Update task status to failed
                            {
                                let mut storage = storage_clone.lock().unwrap();
                                if let Some(task) = storage.get_mut(&task_id) {
                                    let message = Message {
                                        role: Role::Agent,
                                        parts: vec![Part::TextPart(TextPart {
                                            type_: "text".to_string(),
                                            text: "Task failed due to configured mock_stream_final_state".to_string(),
                                            metadata: None,
                                        })],
                                        metadata: None,
                                    };
                                    task.update_status(TaskState::Failed, Some(message));
                                }
                            }
                            "failed"
                        },
                        _ => {
                            // Update task status to completed (default)
                            {
                                let mut storage = storage_clone.lock().unwrap();
                                if let Some(task) = storage.get_mut(&task_id) {
                                    task.update_status(TaskState::Completed, None);
                                }
                            }
                            "completed"
                        }
                    };
                    
                    // Final status update with configured state
                    let final_update = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "id": task_id,
                            "sessionId": session_id,
                            "status": {
                                "state": final_state_str,
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            },
                            "final": true
                        }
                    });
                    
                    // Send final status update
                    let _ = tx.send(format!("data: {}\n\n", final_update.to_string())).await;
                });
                
                // Create a streaming response body by mapping the receiver to Result<String, Infallible>
                let mapped_stream = tokio_stream::wrappers::ReceiverStream::new(rx)
                    .map(|chunk| Ok::<_, Infallible>(chunk));
                
                // Wrap the mapped stream
                let stream_body = Body::wrap_stream(mapped_stream);
                
                // Create a Server-Sent Events response
                let mut response = Response::new(stream_body);
                response.headers_mut().insert(
                    CONTENT_TYPE, 
                    HeaderValue::from_static("text/event-stream")
                );
                
                return Ok(response);
            },
            "tasks/resubscribe" => {
                // Apply configurable delay if specified for the initial response
                let params = request.get("params");
                apply_mock_delay(&params).await;
                
                // Parse streaming configuration if specified
                let streaming_config = parse_streaming_config(&params);
                
                // Extract the configuration parameters or use defaults
                let (text_chunks, artifact_types, chunk_delay_ms, final_state) = streaming_config
                    .unwrap_or((3, vec!["text".to_string()], 300, "completed".to_string()));
                
                // Extract task ID from request params
                let task_id_opt = params.and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let task_id = match task_id_opt {
                    Some(id) => id.to_string(), // Clone the string to avoid borrowing request
                    None => {
                        let error = create_error_response(
                            request.get("id"),
                            error_codes::ERROR_INVALID_PARAMS,
                            "Invalid parameters: missing required 'id' parameter for task",
                            None
                        );
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Check if the task exists
                let task_exists = {
                    let storage = task_storage.lock().unwrap();
                    storage.contains_key(&task_id)
                };
                
                if !task_exists {
                    // Return task not found error for resubscribe
                    let error = create_error_response(
                        request.get("id"),
                        error_codes::ERROR_TASK_NOT_FOUND,
                        &format!("Task not found: {}", task_id),
                        None
                    );
                    let json = serde_json::to_string(&error).unwrap();
                    return Ok(Response::new(Body::from(json)));
                }
                
                let id = request.get("id").unwrap_or(&json!(null)).clone();
                
                // Create a streaming channel
                let (tx, rx) = mpsc::channel::<String>(32);
                
                // Clone values for the spawned task
                let chunk_delay = chunk_delay_ms;
                let task_types = artifact_types.clone();
                let num_text_chunks = text_chunks;
                let final_task_state = final_state.clone();
                
                // Spawn a task to generate streaming events for resubscribe
                tokio::spawn(async move {
                    // Initial status update - pick up where we left off
                    let status_update = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "id": task_id,
                            "sessionId": "stream-session-1",
                            "status": {
                                "state": "working",
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            },
                            "final": false
                        }
                    });
                    
                    // Send status update
                    let _ = tx.send(format!("data: {}\n\n", status_update.to_string())).await;
                    sleep(Duration::from_millis(chunk_delay)).await;
                    
                    // Keep track of the total artifacts generated
                    let mut artifact_count = 0;
                    
                    // Generate text chunks if configured (for resubscribe always use append=true)
                    if task_types.contains(&"text".to_string()) {
                        // For resubscribe, generate fewer chunks but always with append=true
                        // to simulate continuation
                        for i in 0..num_text_chunks {
                            let is_last = i == num_text_chunks - 1;
                            
                            // Generate a resubscription text part
                            let text_part = TextPart {
                                type_: "text".to_string(),
                                text: format!("Continued chunk {} after resubscribe for task {}", i, task_id),
                                metadata: None,
                            };
                            
                            let artifact = Artifact {
                                parts: vec![Part::TextPart(text_part)],
                                index: i as i64,
                                append: Some(true), // Always append for resubscribe
                                name: Some("text_response".to_string()),
                                description: None,
                                last_chunk: Some(is_last),
                                metadata: None,
                            };
                            
                            let artifact_update = json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "id": task_id,
                                    "artifact": artifact
                                }
                            });
                            
                            // Send artifact content chunk
                            let _ = tx.send(format!("data: {}\n\n", artifact_update.to_string())).await;
                            sleep(Duration::from_millis(chunk_delay)).await;
                            
                            artifact_count += 1;
                        }
                    }
                    
                    // Generate data artifact if configured
                    if task_types.contains(&"data".to_string()) {
                        // Generate and send data artifact with resubscribe info
                        let mut data_map = serde_json::Map::new();
                        data_map.insert("type".to_string(), json!("resubscribe_data"));
                        data_map.insert("timestamp".to_string(), json!(chrono::Utc::now().to_rfc3339()));
                        data_map.insert("metrics".to_string(), json!({
                            "resubscribe_count": 1,
                            "task_id": task_id,
                            "continued_at": chrono::Utc::now().to_rfc3339()
                        }));
                        
                        let data_part = DataPart {
                            type_: "data".to_string(),
                            data: data_map,
                            metadata: None,
                        };
                        
                        let artifact = Artifact {
                            parts: vec![Part::DataPart(data_part)],
                            index: artifact_count as i64,
                            append: None,
                            name: Some("resubscribe_metrics".to_string()),
                            description: Some("Resubscription metrics".to_string()),
                            last_chunk: Some(true),
                            metadata: None,
                        };
                        
                        let artifact_update = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "id": task_id,
                                "artifact": artifact
                            }
                        });
                        
                        // Send artifact content chunk
                        let _ = tx.send(format!("data: {}\n\n", artifact_update.to_string())).await;
                        sleep(Duration::from_millis(chunk_delay)).await;
                        
                        artifact_count += 1;
                    }
                    
                    // Determine final state based on configuration
                    let final_state_str = match final_task_state.as_str() {
                        "failed" => "failed",
                        _ => "completed"
                    };
                    
                    // Final status update
                    let final_update = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "id": task_id,
                            "sessionId": "stream-session-1",
                            "status": {
                                "state": final_state_str,
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            },
                            "final": true
                        }
                    });
                    
                    // Send final status update
                    let _ = tx.send(format!("data: {}\n\n", final_update.to_string())).await;
                });
                
                // Create a streaming response body by mapping the receiver to Result<String, Infallible>
                let mapped_stream = tokio_stream::wrappers::ReceiverStream::new(rx)
                    .map(|chunk| Ok::<_, Infallible>(chunk));
                
                // Wrap the mapped stream
                let stream_body = Body::wrap_stream(mapped_stream);
                
                // Create a Server-Sent Events response
                let mut response = Response::new(stream_body);
                response.headers_mut().insert(
                    CONTENT_TYPE, 
                    HeaderValue::from_static("text/event-stream")
                );
                
                return Ok(response);
            },
            "tasks/pushNotification/set" => {
                // Extract task ID and push notification config from request params
                let task_id_opt = request.get("params").and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let task_id = match task_id_opt {
                    Some(id) => id.to_string(),
                    None => {
                        let error = create_error_response(
                            request.get("id"),
                            error_codes::ERROR_INVALID_PARAMS,
                            "Invalid parameters: missing required 'id' parameter for task",
                            None
                        );
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Check if the task exists
                let task_exists = {
                    let storage = task_storage.lock().unwrap();
                    storage.contains_key(&task_id)
                };
                
                if !task_exists {
                    // Return task not found error
                    let error = create_error_response(
                        request.get("id"),
                        error_codes::ERROR_TASK_NOT_FOUND,
                        &format!("Task not found: {}", task_id),
                        None
                    );
                    let json = serde_json::to_string(&error).unwrap();
                    return Ok(Response::new(Body::from(json)));
                }
                
                // Check for the push notification config
                let push_config = match request.get("params").and_then(|p| p.get("pushNotificationConfig")) {
                    Some(config) => config,
                    None => {
                        let error = create_error_response(
                            request.get("id"),
                            error_codes::ERROR_INVALID_PARAMS,
                            "Invalid parameters: missing required 'pushNotificationConfig' parameter",
                            None
                        );
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Verify the webhook URL
                let webhook_url = match push_config.get("url").and_then(|u| u.as_str()) {
                    Some(url) => url,
                    None => {
                        let error = create_error_response(
                            request.get("id"),
                            error_codes::ERROR_INVALID_PARAMS,
                            "Invalid parameters: missing required 'url' parameter in pushNotificationConfig",
                            None
                        );
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // In a real implementation, we would verify the webhook by sending a challenge
                // But for the mock server, we'll just log it
                println!("Setting push notification webhook for task {}: {}", task_id, webhook_url);
                
                // Return success response
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "id": task_id
                    }
                });
                
                let json = serde_json::to_string(&response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "tasks/pushNotification/get" => {
                // Extract task ID from request params
                let task_id_opt = request.get("params").and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let task_id = match task_id_opt {
                    Some(id) => id.to_string(),
                    None => {
                        let error = create_error_response(
                            request.get("id"),
                            error_codes::ERROR_INVALID_PARAMS,
                            "Invalid parameters: missing required 'id' parameter for task",
                            None
                        );
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Check if the task exists
                let task_exists = {
                    let storage = task_storage.lock().unwrap();
                    storage.contains_key(&task_id)
                };
                
                if !task_exists {
                    // Return task not found error
                    let error = create_error_response(
                        request.get("id"),
                        error_codes::ERROR_TASK_NOT_FOUND,
                        &format!("Task not found: {}", task_id),
                        None
                    );
                    let json = serde_json::to_string(&error).unwrap();
                    return Ok(Response::new(Body::from(json)));
                }
                
                // Create a mock response using the proper types
                let auth_info = AuthenticationInfo {
                    schemes: vec!["Bearer".to_string()],
                    credentials: None,
                    extra: serde_json::Map::new(),
                };
                
                let push_config = PushNotificationConfig {
                    url: "https://example.com/webhook".to_string(),
                    authentication: Some(auth_info),
                    token: Some("mock-token-123".to_string()),
                };
                
                let task_push_config = TaskPushNotificationConfig {
                    id: task_id,
                    push_notification_config: push_config,
                };
                
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": task_push_config
                });
                
                let json = serde_json::to_string(&response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            // Removed non-standard Batch operations: batches/create, batches/get, batches/cancel
            // Removed non-standard Skills operations: skills/list, skills/get, skills/invoke
            // Removed non-standard File operations: files/upload, files/download, files/list
            // Fallback for unhandled methods
            _ => {
                // Return method not found error with more context
                let error_response = create_error_response(
                    request.get("id"),
                    error_codes::ERROR_METHOD_NOT_FOUND,
                    &format!("Method not found: {}", method),
                    Some(json!({
                        "available_methods": [
                            "tasks/send", "tasks/get", "tasks/cancel", "tasks/sendSubscribe",
                            "tasks/resubscribe", "tasks/pushNotification/set", "tasks/pushNotification/get"
                            // Removed non-standard methods from the list
                        ]
                    }))
                );
                
                let json = serde_json::to_string(&error_response).unwrap();
                return Ok(Response::new(Body::from(json)));
            }
        }
    }
    
    // Return invalid request error with more detailed information
    let error_response = create_error_response(
        None,
        error_codes::ERROR_INVALID_REQUEST,
        "Request payload validation error: missing or invalid 'method' field",
        Some(json!({
            "help": "JSON-RPC requests must include a valid 'method' field",
            "example": {
                "jsonrpc": "2.0",
                "id": "request-id",
                "method": "tasks/send",
                "params": {}
            }
        }))
    );
    
    let json = serde_json::to_string(&error_response).unwrap();
    Ok(Response::new(Body::from(json)))
}

/// Helper function that parses and applies a mock delay from request metadata
/// 
/// This function enables configurable delays to simulate network latency and server processing time.
/// It looks for the "_mock_delay_ms" field in the request's metadata and applies the specified delay.
/// 
/// Example metadata:
/// ```json
/// {
///   "metadata": {
///     "_mock_delay_ms": 2000  // 2-second delay
///   }
/// }
/// ```
/// 
/// For streaming operations, there's also support for "_mock_chunk_delay_ms" which controls 
/// the delay between streamed chunks (used in sendSubscribe and resubscribe methods).
async fn apply_mock_delay(params: &Option<&Value>) {
    if let Some(params_value) = params {
        if let Some(metadata) = params_value.get("metadata").and_then(|m| m.as_object()) {
            // Check for the _mock_delay_ms field in metadata
            if let Some(delay_value) = metadata.get("_mock_delay_ms") {
                if let Some(delay_ms) = delay_value.as_u64() {
                    println!("⏱️ Applying mock delay of {}ms", delay_ms);
                    // Cap the delay at a reasonable maximum (e.g., 10 seconds)
                    let capped_delay = std::cmp::min(delay_ms, 10000);
                    sleep(Duration::from_millis(capped_delay)).await;
                }
            }
        }
    }
}

/// Parse task simulation configuration from request metadata
/// 
/// This function extracts configuration for simulating realistic task state machine behaviors:
/// 
/// Examples:
/// ```json
/// {
///   "metadata": {
///     "_mock_duration_ms": 5000,       // Task takes 5 seconds to complete
///     "_mock_require_input": true,     // Task will require additional input
///     "_mock_fail": true,              // Task will fail instead of completing
///     "_mock_fail_message": "Custom failure message"  // Optional failure message
///   }
/// }
/// ```
fn parse_task_simulation_config(params: &Option<&Value>) -> Option<(u64, bool, bool, Option<String>)> {
    if let Some(params_value) = params {
        if let Some(metadata) = params_value.get("metadata").and_then(|m| m.as_object()) {
            // Default duration is 3 seconds if not specified but other simulation params are present
            let default_duration = 3000;
            
            // Extract duration (with default)
            let duration_ms = metadata.get("_mock_duration_ms")
                .and_then(|d| d.as_u64())
                .unwrap_or(default_duration);
                
            // Cap duration at a reasonable maximum (e.g., 60 seconds)
            let capped_duration = std::cmp::min(duration_ms, 60000);
            
            // Extract input requirement
            let require_input = metadata.get("_mock_require_input")
                .and_then(|r| r.as_bool())
                .unwrap_or(false);
                
            // Extract failure simulation
            let should_fail = metadata.get("_mock_fail")
                .and_then(|f| f.as_bool())
                .unwrap_or(false);
                
            // Extract custom failure message
            let fail_message = metadata.get("_mock_fail_message")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string());
                
            // If any simulation parameter is present, return the configuration
            if metadata.contains_key("_mock_duration_ms") || 
               metadata.contains_key("_mock_require_input") || 
               metadata.contains_key("_mock_fail") {
                return Some((capped_duration, require_input, should_fail, fail_message));
            }
        }
    }
    None
}

/// Parse streaming configuration from request metadata
/// 
/// This function extracts dynamic streaming content configuration from request metadata:
/// - text_chunks: Number of text chunks to stream (default: 5)
/// - artifact_types: Types of artifacts to include (default: ["text", "data", "file"])
/// - chunk_delay_ms: Delay between chunks in ms (default: 300)
/// - final_state: Final state of the task (default: "completed")
fn parse_streaming_config(params: &Option<&Value>) -> Option<(u64, Vec<String>, u64, String)> {
    if let Some(params_value) = params {
        if let Some(metadata) = params_value.get("metadata").and_then(|m| m.as_object()) {
            // Extract streaming-related parameters
            let has_streaming_config = metadata.contains_key("_mock_stream_text_chunks") || 
                                      metadata.contains_key("_mock_stream_artifact_types") || 
                                      metadata.contains_key("_mock_stream_chunk_delay_ms") || 
                                      metadata.contains_key("_mock_stream_final_state");
                                      
            if !has_streaming_config {
                return None;
            }
                                      
            // Extract text_chunks parameter (default to 5 if not provided)
            let text_chunks = metadata.get("_mock_stream_text_chunks")
                .and_then(|c| c.as_u64())
                .unwrap_or(5);
            
            // Extract artifact_types parameter (default to all types if not provided)
            let artifact_types = metadata.get("_mock_stream_artifact_types")
                .and_then(|t| t.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_else(|| vec!["text".to_string(), "data".to_string(), "file".to_string()]);
            
            // Extract chunk_delay_ms parameter (default to 300ms if not provided)
            let chunk_delay_ms = metadata.get("_mock_stream_chunk_delay_ms")
                .and_then(|d| d.as_u64())
                .unwrap_or(300);
            
            // Extract final_state parameter (default to "completed" if not provided)
            let final_state = metadata.get("_mock_stream_final_state")
                .and_then(|s| s.as_str())
                .unwrap_or("completed")
                .to_string();
            
            return Some((text_chunks, artifact_types, chunk_delay_ms, final_state));
        }
    }
    
    None
}

/// Generate a text artifact for streaming
fn generate_mock_text_artifact(task_id: &str, index: usize, is_last: bool, append: bool) -> (Artifact, Value, u64) {
    let text_part = TextPart {
        type_: "text".to_string(),
        text: format!("This is mock text chunk {} for task {}", index, task_id),
        metadata: None,
    };
    
    let artifact = Artifact {
        parts: vec![Part::TextPart(text_part)],
        index: index as i64,
        append: Some(append),
        name: Some("text_response".to_string()),
        description: None,
        last_chunk: Some(is_last),
        metadata: None,
    };
    
    // Create JSON-RPC response with this artifact
    let id = rand::random::<u64>();
    let artifact_update = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "id": task_id,
            "artifact": artifact.clone()
        }
    });
    
    (artifact, artifact_update, id)
}

/// Generate a data artifact for streaming
fn generate_mock_data_artifact(task_id: &str, index: usize, is_last: bool) -> (Artifact, Value, u64) {
    let mut data_map = serde_json::Map::new();
    data_map.insert("type".to_string(), json!("mock_data"));
    data_map.insert("timestamp".to_string(), json!(chrono::Utc::now().to_rfc3339()));
    data_map.insert("metrics".to_string(), json!({
        "chunk_number": index,
        "task_id": task_id,
        "processing_time": (index as f64) * 1.25
    }));
    
    let data_part = DataPart {
        type_: "data".to_string(),
        data: data_map,
        metadata: None,
    };
    
    let artifact = Artifact {
        parts: vec![Part::DataPart(data_part)],
        index: index as i64,
        append: None,
        name: Some(format!("data_artifact_{}", index)),
        description: Some(format!("Mock data artifact #{}", index)),
        last_chunk: Some(is_last),
        metadata: None,
    };
    
    // Create JSON-RPC response with this artifact
    let id = rand::random::<u64>();
    let artifact_update = json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "id": task_id,
            "artifact": artifact.clone()
        }
    });
    
    (artifact, artifact_update, id)
}

// Removed unused generate_mock_file_artifact function

/// Asynchronously simulate a task's lifecycle with different states
/// 
/// This function runs in the background and updates the task's state according to
/// the simulation configuration. It handles:
/// 
/// 1. Transitioning from Submitted to Working
/// 2. Waiting for a portion of the specified duration
/// 3. Potentially transitioning to InputRequired
/// 4. Waiting for input or remaining duration
/// 5. Transitioning to Completed, Failed, or remaining in InputRequired
/// 
async fn simulate_task_lifecycle(
    task_id: String, 
    task_storage: TaskStorage,
    duration_ms: u64,
    require_input: bool, 
    should_fail: bool,
    fail_message: Option<String>
) {
    println!("🔄 Starting task lifecycle simulation for task {}", task_id);
    println!("   Duration: {}ms, Require input: {}, Should fail: {}", 
             duration_ms, require_input, should_fail);
    
    // Calculate middle point for state transition (half the duration)
    let half_duration = duration_ms / 2;
    
    // Update to Working state
    {
        let mut storage = task_storage.lock().unwrap();
        if let Some(task) = storage.get_mut(&task_id) {
            // Create a working message
            let working_message = Message {
                role: Role::Agent,
                parts: vec![Part::TextPart(TextPart {
                    type_: "text".to_string(),
                    text: "Working on your request...".to_string(),
                    metadata: None,
                })],
                metadata: None,
            };
            
            // Update to working state
            task.update_status(TaskState::Working, Some(working_message));
            println!("📝 Task {} transitioned to Working state", task_id);
        } else {
            println!("⚠️ Task {} not found, cannot simulate lifecycle", task_id);
            return;
        }
    }
    
    // Wait for half the duration
    sleep(Duration::from_millis(half_duration)).await;
    
    // If input is required, update state to InputRequired and wait for input
    if require_input {
        {
            let mut storage = task_storage.lock().unwrap();
            if let Some(task) = storage.get_mut(&task_id) {
                // Create an input required message
                let input_message = Message {
                    role: Role::Agent,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: "I need more information to proceed. Please provide additional details.".to_string(),
                        metadata: None,
                    })],
                    metadata: None,
                };
                
                // Update to input required state
                task.update_status(TaskState::InputRequired, Some(input_message));
                println!("📝 Task {} transitioned to InputRequired state", task_id);
            }
        }
        
        // Wait for input or timeout (the remaining half of the duration)
        let timeout_duration = Duration::from_millis(half_duration);
        let wait_until = Utc::now() + chrono::Duration::from_std(timeout_duration).unwrap();
        
        // Loop until timeout or input received
        loop {
            // Check if timeout has elapsed
            if Utc::now() > wait_until {
                println!("⏱️ Timeout waiting for input for task {}", task_id);
                break;
            }
            
            // Check if input was received
            let input_received = {
                let storage = task_storage.lock().unwrap();
                if let Some(task) = storage.get(&task_id) {
                    task.input_received
                } else {
                    false
                }
            };
            
            if input_received {
                println!("📨 Input received for task {}, continuing processing", task_id);
                
                // Update status to Working again
                {
                    let mut storage = task_storage.lock().unwrap();
                    if let Some(task) = storage.get_mut(&task_id) {
                        // Create a working message
                        let working_message = Message {
                            role: Role::Agent,
                            parts: vec![Part::TextPart(TextPart {
                                type_: "text".to_string(),
                                text: "Thanks for the additional information. Continuing processing...".to_string(),
                                metadata: None,
                            })],
                            metadata: None,
                        };
                        
                        // Verify the task is still in InputRequired state before updating
                        if task.current_status.state == TaskState::InputRequired {
                            // Update to working state
                            task.update_status(TaskState::Working, Some(working_message));
                            println!("📝 Task {} updated to Working state in simulation", task_id);
                        } else {
                            println!("ℹ️ Task {} already updated, current state: {:?}", 
                                     task_id, task.current_status.state);
                        }
                    }
                }
                
                // Reduce waiting time after input received to speed up tests
                let remaining_time = std::cmp::min(half_duration, 1000); // Use at most 1 second
                sleep(Duration::from_millis(remaining_time)).await;
                break;
            }
            
            // Sleep briefly before checking again
            sleep(Duration::from_millis(100)).await;
        }
    } else {
        // No input required, sleep for remaining duration
        sleep(Duration::from_millis(half_duration)).await;
    }
    
    // Final state transition (Completed or Failed)
    {
        let mut storage = task_storage.lock().unwrap();
        if let Some(task) = storage.get_mut(&task_id) {
            // Skip if task was manually canceled during simulation
            if task.current_status.state == TaskState::Canceled {
                println!("🛑 Task {} was canceled, not updating final state", task_id);
                return;
            }
            
            if should_fail {
                // Create a failure message
                let fail_text = fail_message.unwrap_or_else(|| "Task failed to complete.".to_string());
                let fail_message = Message {
                    role: Role::Agent,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: fail_text,
                        metadata: None,
                    })],
                    metadata: None,
                };
                
                // Update to failed state
                task.update_status(TaskState::Failed, Some(fail_message));
                println!("❌ Task {} transitioned to Failed state", task_id);
            } else {
                // Only update to completed if no input is required or input was received
                if !require_input || task.input_received {
                    // Create a completion message
                    let completion_message = Message {
                        role: Role::Agent,
                        parts: vec![Part::TextPart(TextPart {
                            type_: "text".to_string(),
                            text: "Task completed successfully!".to_string(),
                            metadata: None,
                        })],
                        metadata: None,
                    };
                    
                    // Update to completed state
                    task.update_status(TaskState::Completed, Some(completion_message));
                    println!("✅ Task {} transitioned to Completed state", task_id);
                }
                // If input is required but not received, leave in InputRequired state
            }
        }
    }
}

// Helper function to create response artifacts based on message content
fn create_response_artifacts(message_opt: &Option<Value>) -> Vec<Artifact> {
    let mut artifacts = Vec::new();
    
    // Create a default text artifact
    let text_part = TextPart {
        type_: "text".to_string(),
        text: "This is a response from the A2A mock server".to_string(),
        metadata: None,
    };
    
    let text_artifact = Artifact {
        parts: vec![Part::TextPart(text_part)],
        index: 0,
        name: Some("text_response".to_string()),
        description: Some("Text output".to_string()),
        append: None,
        last_chunk: None,
        metadata: None,
    };
    
    artifacts.push(text_artifact);
    
    // If we have a message with specific content, create more tailored artifacts
    if let Some(message) = message_opt {
        // Check if the message contains file parts
        if let Some(parts) = message.get("parts").and_then(|p| p.as_array()) {
            let has_file = parts.iter().any(|p| p.get("type").and_then(|t| t.as_str()) == Some("file"));
            let has_data = parts.iter().any(|p| p.get("type").and_then(|t| t.as_str()) == Some("data"));
            
            // If the message had a file, respond with a file artifact
            if has_file {
                let file_content = FileContent {
                    bytes: Some(BASE64.encode("This is a response file from the mock server")),
                    uri: None,
                    mime_type: Some("text/plain".to_string()),
                    name: Some("response.txt".to_string()),
                };
                
                let file_part = FilePart {
                    type_: "file".to_string(),
                    file: file_content,
                    metadata: None,
                };
                
                let file_artifact = Artifact {
                    parts: vec![Part::FilePart(file_part)],
                    index: artifacts.len() as i64,
                    name: Some("file_response".to_string()),
                    description: Some("File output".to_string()),
                    append: None,
                    last_chunk: None,
                    metadata: None,
                };
                
                artifacts.push(file_artifact);
            }
            
            // If the message had structured data, respond with data artifact
            if has_data {
                let mut data_map = serde_json::Map::new();
                data_map.insert("type".to_string(), json!("response_data"));
                data_map.insert("timestamp".to_string(), json!(chrono::Utc::now().to_rfc3339()));
                data_map.insert("status".to_string(), json!("success"));
                
                let data_part = DataPart {
                    type_: "data".to_string(),
                    data: data_map,
                    metadata: None,
                };
                
                let data_artifact = Artifact {
                    parts: vec![Part::DataPart(data_part)],
                    index: artifacts.len() as i64,
                    name: Some("data_response".to_string()),
                    description: Some("Structured data output".to_string()),
                    append: None,
                    last_chunk: None,
                    metadata: None,
                };
                
                artifacts.push(data_artifact);
            }
        }
    }
    
    artifacts
}

// Start the mock server with authentication required
pub fn start_mock_server(port: u16) {
    // Default to requiring authentication
    start_mock_server_with_auth(port, true);
}

// Start the mock server with configurable authentication requirements
pub fn start_mock_server_with_auth(port: u16, require_auth: bool) {
    // Set the port in an environment variable so the request handler can access it
    std::env::set_var("SERVER_PORT", port.to_string());
    
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        
        // Create shared task storage
        let task_storage = create_task_storage();
        
        // Removed batch_storage creation
        // Removed file_storage creation
        
        // Clone storages for the make_service closure
        let ts = task_storage.clone();
        // Removed bs clone
        // Removed fs clone
        
        // Set whether authentication is required
        let auth_required = require_auth;
        
        let make_svc = make_service_fn(move |_conn| {
            // Clone storages for each service function
            let ts_clone = ts.clone();
            // Removed bs_clone
            // Removed fs_clone
            let auth_req = auth_required;
            
            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    // Clone storages for each request
                    let ts_req = ts_clone.clone();
                    // Removed bs_req
                    // Removed fs_req
                    // Removed unused bs_req and fs_req from handle_a2a_request_with_auth call
                    handle_a2a_request_with_auth(ts_req, req, auth_req)
                }))
            }
        });
        
        let server = Server::bind(&addr).serve(make_svc);
        
        println!("📡 Mock A2A server running at http://{}", addr);
        println!("Press Ctrl+C to stop the server...");
        
        if let Err(e) = server.await {
            eprintln!("Server error: {}", e);
        }
    });
}
