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
        }
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

// Task batch structure for the mock server
#[derive(Debug, Clone)]
struct MockBatch {
    id: String,
    name: Option<String>,
    created_at: DateTime<Utc>,
    task_ids: Vec<String>,
    metadata: Option<Map<String, Value>>,
}

impl MockBatch {
    fn new(id: &str, name: Option<String>, task_ids: Vec<String>, metadata: Option<Map<String, Value>>) -> Self {
        Self {
            id: id.to_string(),
            name,
            created_at: Utc::now(),
            task_ids,
            metadata,
        }
    }
    
    // Convert to JSON for API responses
    fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "created_at": self.created_at,
            "task_ids": self.task_ids,
            "metadata": self.metadata,
        })
    }
}

// File representation for the mock server
#[derive(Debug, Clone)]
struct MockFile {
    id: String,
    name: String,
    mime_type: String,
    content: String,  // base64 encoded
    size: usize,
    uploaded_at: DateTime<Utc>,
    task_id: Option<String>,
    metadata: Option<Map<String, Value>>,
}

impl MockFile {
    fn new(id: &str, name: &str, mime_type: &str, content: &str, task_id: Option<&str>, metadata: Option<Map<String, Value>>) -> Self {
        // Calculate approximate size from base64 content
        // Base64 increases size by ~33%, so we estimate original size
        let size = (content.len() * 3) / 4;
        
        Self {
            id: id.to_string(),
            name: name.to_string(),
            mime_type: mime_type.to_string(),
            content: content.to_string(),
            size,
            uploaded_at: Utc::now(),
            task_id: task_id.map(|id| id.to_string()),
            metadata,
        }
    }
    
    // Convert to JSON response for upload/list response
    fn to_upload_json(&self) -> Value {
        json!({
            "file_id": self.id,
            "uri": format!("files/{}", self.id),
            "name": self.name,
            "mime_type": self.mime_type,
            "size": self.size,
            "uploaded_at": self.uploaded_at,
        })
    }
    
    // Convert to JSON response for download response
    fn to_download_json(&self) -> Value {
        json!({
            "file_id": self.id,
            "name": self.name,
            "mime_type": self.mime_type,
            "bytes": self.content,
            "size": self.size,
        })
    }
}

// Global task storage
type TaskStorage = Arc<Mutex<HashMap<String, MockTask>>>;

// Global batch storage
type BatchStorage = Arc<Mutex<HashMap<String, MockBatch>>>;

// Global file storage
type FileStorage = Arc<Mutex<HashMap<String, MockFile>>>;

// Create a new task storage
fn create_task_storage() -> TaskStorage {
    Arc::new(Mutex::new(HashMap::new()))
}

// Create a new batch storage
fn create_batch_storage() -> BatchStorage {
    Arc::new(Mutex::new(HashMap::new()))
}

// Create a new file storage
fn create_file_storage() -> FileStorage {
    Arc::new(Mutex::new(HashMap::new()))
}

// Mock handlers for A2A endpoints
async fn handle_a2a_request(task_storage: TaskStorage, batch_storage: BatchStorage, file_storage: FileStorage, req: Request<Body>) -> Result<Response<Body>, Infallible> {
    // Check if this is a request for agent card
    // Check for Accept header to see if client wants SSE
    let accept_header = req.headers().get("Accept")
                       .and_then(|h| h.to_str().ok())
                       .unwrap_or("");
                       
    if req.uri().path() == "/.well-known/agent.json" {
        // Create the agent card using the proper types
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
        
        let authentication = AgentAuthentication {
            schemes: vec!["None".to_string()],
            credentials: None,
        };
        
        let agent_card = AgentCard {
            name: "Mock A2A Server".to_string(),
            description: Some("A mock server for testing A2A protocol clients".to_string()),
            url: "http://localhost:8080".to_string(),
            provider: None,
            version: "0.1.0".to_string(),
            documentation_url: None,
            capabilities,
            authentication: Some(authentication),
            default_input_modes: vec!["text/plain".to_string()],
            default_output_modes: vec!["text/plain".to_string()],
            skills: vec![skill],
        };
        
        let json = serde_json::to_string(&agent_card).unwrap();
        return Ok(Response::new(Body::from(json)));
    }
    
    // Handle JSON-RPC requests
    let body_bytes = hyper::body::to_bytes(req.into_body()).await.unwrap();
    let request: Value = match serde_json::from_slice(&body_bytes) {
        Ok(req) => req,
        Err(e) => {
            let error_response = json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {
                    "code": -32700,
                    "message": format!("Parse error: {}", e),
                }
            });
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
            "tasks/send" => {
                // Generate a new task ID or extract from params if provided
                let task_id = request.get("params")
                    .and_then(|p| p.get("id"))
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("mock-task-{}", chrono::Utc::now().timestamp_millis()));
                
                let session_id = request.get("params")
                    .and_then(|p| p.get("sessionId"))
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("mock-session-{}", chrono::Utc::now().timestamp_millis()));
                
                // Create a new task record with initial "submitted" status
                let task = MockTask::new(&task_id, &session_id);
                
                // Create response artifacts based on message content
                let artifacts = create_response_artifacts(&message_opt);
                
                // Store the task in our task storage
                {
                    let mut storage = task_storage.lock().unwrap();
                    
                    // If task already exists, just update it, otherwise insert new
                    if let Some(existing_task) = storage.get_mut(&task_id) {
                        // Update with working state
                        existing_task.update_status(TaskState::Working, None);
                        // Add artifacts
                        for artifact in artifacts {
                            existing_task.add_artifact(artifact);
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
                        existing_task.update_status(TaskState::Completed, Some(completed_message));
                    } else {
                        // Insert new task
                        let mut new_task = task;
                        // Update with working state
                        new_task.update_status(TaskState::Working, None);
                        // Add artifacts
                        for artifact in artifacts {
                            new_task.add_artifact(artifact);
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
                        new_task.update_status(TaskState::Completed, Some(completed_message));
                        
                        // Store new task
                        storage.insert(task_id.clone(), new_task);
                    }
                }
                
                // Get the task to return (includes all updates)
                let task_json = {
                    let storage = task_storage.lock().unwrap();
                    if let Some(task) = storage.get(&task_id) {
                        // Don't include history in the initial response
                        task.to_json(false)
                    } else {
                        // This shouldn't happen, but just in case
                        json!({
                            "id": task_id,
                            "sessionId": session_id,
                            "status": {
                                "state": "completed",
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            }
                        })
                    }
                };
                
                // Create a SendTaskResponse with a Task
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": task_json
                });
                
                let json = serde_json::to_string(&response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "tasks/get" => {
                // Extract task ID from request params
                let task_id_opt = request.get("params").and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let task_id = match task_id_opt {
                    Some(id) => id.to_string(), // Clone the string to avoid borrowing request
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing task id"
                            }
                        });
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
                        // If task doesn't exist in our storage, create a mock response
                        
                        // Create a complete mock task with status history
                        let mut mock_task = MockTask::new(&task_id, &format!("mock-session-{}", 
                            chrono::Utc::now().timestamp_millis()));
                        
                        // Create some sample artifacts
                        let text_part = TextPart {
                            type_: "text".to_string(),
                            text: "This is a sample text response from the A2A server".to_string(),
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
                        
                        // Create a sample file artifact
                        let file_content = FileContent {
                            bytes: Some(BASE64.encode("Sample file content from mock server")),
                            uri: None,
                            mime_type: Some("text/plain".to_string()),
                            name: Some("result.txt".to_string()),
                        };
                        
                        let file_part = FilePart {
                            type_: "file".to_string(),
                            file: file_content,
                            metadata: None,
                        };
                        
                        let file_artifact = Artifact {
                            parts: vec![Part::FilePart(file_part)],
                            index: 1,
                            name: Some("file_output".to_string()),
                            description: Some("Sample file output".to_string()),
                            append: None,
                            last_chunk: None,
                            metadata: None,
                        };
                        
                        // Add artifacts
                        mock_task.add_artifact(text_artifact);
                        mock_task.add_artifact(file_artifact);
                        
                        // Add status transitions
                        let five_min_ago = Utc::now() - chrono::Duration::minutes(5);
                        let ten_min_ago = Utc::now() - chrono::Duration::minutes(10);
                        
                        // Set timestamps on the history entries to be in the past
                        mock_task.state_history[0].timestamp = Some(ten_min_ago);
                        
                        // Add working state
                        mock_task.update_status(TaskState::Working, None);
                        mock_task.state_history.last_mut().unwrap().timestamp = Some(five_min_ago);
                        
                        // Add completed state
                        let completed_message = Message {
                            role: Role::Agent,
                            parts: vec![Part::TextPart(TextPart {
                                type_: "text".to_string(),
                                text: "Task completed successfully!".to_string(),
                                metadata: None,
                            })],
                            metadata: None,
                        };
                        mock_task.update_status(TaskState::Completed, Some(completed_message));
                        
                        // Return mock task response
                        json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "result": mock_task.to_json(include_history)
                        })
                    }
                };
                
                let json = serde_json::to_string(&task_response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "tasks/cancel" => {
                // Extract task ID from request params
                let task_id_opt = request.get("params").and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let task_id = match task_id_opt {
                    Some(id) => id.to_string(), // Clone the string to avoid borrowing request
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing task id"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Update task state to canceled if it exists
                let task_json = {
                    let mut storage = task_storage.lock().unwrap();
                    if let Some(task) = storage.get_mut(&task_id) {
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
                        // If task doesn't exist, create a simple canceled task
                        let mut mock_task = MockTask::new(&task_id, &format!("mock-session-{}", 
                            chrono::Utc::now().timestamp_millis()));
                            
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
                        mock_task.update_status(TaskState::Canceled, Some(canceled_message));
                        
                        // Add to storage
                        let task_json = mock_task.to_json(false);
                        storage.insert(task_id.clone(), mock_task);
                        task_json
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
                // This is a streaming endpoint, respond with SSE
                let id = request.get("id").unwrap_or(&json!(null)).clone();
                let task_id = format!("stream-task-{}", chrono::Utc::now().timestamp_millis());
                let session_id = format!("stream-session-{}", chrono::Utc::now().timestamp_millis());
                
                // Create a new task for this streaming request
                let mut streaming_task = MockTask::new(&task_id, &session_id);
                
                // Create a streaming channel
                let (tx, rx) = mpsc::channel::<String>(32);
                
                // Create a clone of task_storage for the spawned task
                let storage_clone = task_storage.clone();
                
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
                    sleep(Duration::from_millis(500)).await;
                    
                    // Simulate content streaming with different types of artifacts
                    // First, send text parts
                    let content_parts = vec![
                        "This is ", "the first ", "part of ", "the streaming ", 
                        "response from ", "the mock A2A server."
                    ];
                    
                    for (i, part) in content_parts.iter().enumerate() {
                        let is_last = i == content_parts.len() - 1;
                        
                        // Create a proper artifact using the types
                        let text_part = TextPart {
                            type_: "text".to_string(),
                            text: part.to_string(),
                            metadata: None,
                        };
                        
                        let artifact = Artifact {
                            parts: vec![Part::TextPart(text_part)],
                            index: 0,
                            append: Some(i > 0),
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
                        sleep(Duration::from_millis(300)).await;
                    }
                    
                    // Then, send a structured data artifact
                    let mut data_map = serde_json::Map::new();
                    data_map.insert("type".to_string(), json!("result_data"));
                    data_map.insert("timestamp".to_string(), json!(chrono::Utc::now().to_rfc3339()));
                    data_map.insert("metrics".to_string(), json!({
                        "tokens": 150,
                        "processing_time": 1.25
                    }));
                    
                    let data_part = DataPart {
                        type_: "data".to_string(),
                        data: data_map,
                        metadata: None,
                    };
                    
                    let data_artifact = Artifact {
                        parts: vec![Part::DataPart(data_part)],
                        index: 1,
                        append: None,
                        name: Some("result_metrics".to_string()),
                        description: Some("Performance metrics".to_string()),
                        last_chunk: Some(true),
                        metadata: None,
                    };
                    
                    let data_update = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "id": task_id,
                            "artifact": data_artifact
                        }
                    });
                    
                    // Send data artifact
                    let _ = tx.send(format!("data: {}\n\n", data_update.to_string())).await;
                    sleep(Duration::from_millis(300)).await;
                    
                    // Finally, send a simple file artifact with base64 content
                    let file_content = FileContent {
                        bytes: Some("SGVsbG8sIHRoaXMgaXMgYSBzaW1wbGUgZmlsZSBhcnRpZmFjdCBmcm9tIHRoZSBtb2NrIHNlcnZlciE=".to_string()), // "Hello, this is a simple file artifact from the mock server!"
                        uri: None,
                        mime_type: Some("text/plain".to_string()),
                        name: Some("result.txt".to_string()),
                    };
                    
                    let file_part = FilePart {
                        type_: "file".to_string(),
                        file: file_content,
                        metadata: None,
                    };
                    
                    let file_artifact = Artifact {
                        parts: vec![Part::FilePart(file_part)],
                        index: 2,
                        append: None,
                        name: Some("output_file".to_string()),
                        description: Some("Text output file".to_string()),
                        last_chunk: Some(true),
                        metadata: None,
                    };
                    
                    let file_update = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "id": task_id,
                            "artifact": file_artifact
                        }
                    });
                    
                    // Send file artifact
                    let _ = tx.send(format!("data: {}\n\n", file_update.to_string())).await;
                    sleep(Duration::from_millis(300)).await;
                    
                    // Final completed status update
                    let final_update = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "id": task_id,
                            "sessionId": "stream-session-1",
                            "status": {
                                "state": "completed",
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
                // Extract task ID from request params
                let task_id_opt = request.get("params").and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let task_id = match task_id_opt {
                    Some(id) => id.to_string(), // Clone the string to avoid borrowing request
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing task id"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                let id = request.get("id").unwrap_or(&json!(null)).clone();
                
                // Create a streaming channel
                let (tx, rx) = mpsc::channel::<String>(32);
                
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
                    sleep(Duration::from_millis(300)).await;
                    
                    // Send some continuation content
                    let content = "Continuing from where we left off... here's some more content!";
                    
                    let text_part = TextPart {
                        type_: "text".to_string(),
                        text: content.to_string(),
                        metadata: None,
                    };
                    
                    let artifact = Artifact {
                        parts: vec![Part::TextPart(text_part)],
                        index: 0,
                        append: Some(true),
                        name: Some("text_response".to_string()),
                        description: None,
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
                    
                    // Send artifact content
                    let _ = tx.send(format!("data: {}\n\n", artifact_update.to_string())).await;
                    sleep(Duration::from_millis(300)).await;
                    
                    // Final completed status update
                    let final_update = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "id": task_id,
                            "sessionId": "stream-session-1",
                            "status": {
                                "state": "completed",
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
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing task id"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Check for the push notification config
                let push_config = match request.get("params").and_then(|p| p.get("pushNotificationConfig")) {
                    Some(config) => config,
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing push notification config"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Verify the webhook URL
                let webhook_url = match push_config.get("url").and_then(|u| u.as_str()) {
                    Some(url) => url,
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing webhook URL in push notification config"
                            }
                        });
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
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing task id"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
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
            // Batch operations
            "batches/create" => {
                // Extract batch data from request
                let batch_opt = request.get("params").and_then(|p| p.get("batch"));
                let batch_data = match batch_opt {
                    Some(data) => data,
                    None => {
                        // If batch data is missing, return error
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing batch data"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Extract batch ID and task IDs
                let batch_id = batch_data.get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| "mock-batch-id");
                
                let name = batch_data.get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                
                let task_ids = batch_data.get("task_ids")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                           .filter_map(|v| v.as_str())
                           .map(|s| s.to_string())
                           .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(|| vec![]);
                
                let metadata = batch_data.get("metadata")
                    .and_then(|v| v.as_object().cloned());
                
                // Create new batch
                let batch = MockBatch::new(batch_id, name, task_ids, metadata);
                
                // Store the batch
                {
                    let mut storage = batch_storage.lock().unwrap();
                    storage.insert(batch_id.to_string(), batch.clone());
                }
                
                // Return batch data
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": batch.to_json()
                });
                
                let json = serde_json::to_string(&response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "batches/get" => {
                // Extract batch ID from request params
                let batch_id_opt = request.get("params").and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let batch_id = match batch_id_opt {
                    Some(id) => id.to_string(),
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing batch id"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Check if should include task details
                let include_tasks = request.get("params")
                    .and_then(|p| p.get("include_tasks"))
                    .and_then(|i| i.as_bool())
                    .unwrap_or(false);
                
                // Get the batch
                let batch_response = {
                    let storage = batch_storage.lock().unwrap();
                    if let Some(batch) = storage.get(&batch_id) {
                        // If include_tasks is true, also fetch task details
                        let mut batch_json = batch.to_json();
                        
                        if include_tasks {
                            let task_storage_lock = task_storage.lock().unwrap();
                            let tasks_json = batch.task_ids.iter()
                                .filter_map(|id| task_storage_lock.get(id))
                                .map(|task| task.to_json(true)) // Include history
                                .collect::<Vec<_>>();
                            
                            batch_json["tasks"] = json!(tasks_json);
                        }
                        
                        json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "result": batch_json
                        })
                    } else {
                        // Create a mock batch if it doesn't exist
                        let mock_batch = MockBatch::new(
                            &batch_id,
                            Some(format!("Mock Batch {}", batch_id)),
                            vec![
                                format!("task-in-batch-1-{}", chrono::Utc::now().timestamp_millis()),
                                format!("task-in-batch-2-{}", chrono::Utc::now().timestamp_millis()),
                            ],
                            None
                        );
                        
                        json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "result": mock_batch.to_json()
                        })
                    }
                };
                
                let json = serde_json::to_string(&batch_response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            // Skills operations
            "skills/list" => {
                // Get optional filter tags
                let tags_opt = request.get("params")
                    .and_then(|p| p.get("tags"))
                    .and_then(|t| t.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(String::from)
                            .collect::<Vec<String>>()
                    });
                
                // Create mock skills list
                let mut skills = vec![
                    AgentSkill {
                        id: "test-skill-1".to_string(),
                        name: "Echo".to_string(),
                        description: Some("Echoes back any message sent".to_string()),
                        tags: Some(vec!["basic".to_string(), "text".to_string()]),
                        examples: Some(vec!["Echo this message".to_string()]),
                        input_modes: Some(vec!["text/plain".to_string()]),
                        output_modes: Some(vec!["text/plain".to_string()]),
                    },
                    AgentSkill {
                        id: "test-skill-2".to_string(),
                        name: "Summarize".to_string(),
                        description: Some("Summarizes long text content".to_string()),
                        tags: Some(vec!["text".to_string(), "analysis".to_string()]),
                        examples: Some(vec!["Summarize this article".to_string()]),
                        input_modes: Some(vec!["text/plain".to_string(), "text/html".to_string()]),
                        output_modes: Some(vec!["text/plain".to_string()]),
                    },
                    AgentSkill {
                        id: "test-skill-3".to_string(),
                        name: "Image Generation".to_string(),
                        description: Some("Creates images from text descriptions".to_string()),
                        tags: Some(vec!["image".to_string(), "creative".to_string()]),
                        examples: Some(vec!["Generate an image of a sunset over mountains".to_string()]),
                        input_modes: Some(vec!["text/plain".to_string()]),
                        output_modes: Some(vec!["image/png".to_string(), "image/jpeg".to_string()]),
                    }
                ];
                
                // Filter skills if tags provided
                if let Some(tags) = tags_opt {
                    skills = skills.into_iter()
                        .filter(|skill| {
                            if let Some(skill_tags) = &skill.tags {
                                // Check if any of the skill's tags match the filter tags
                                skill_tags.iter().any(|tag| tags.contains(tag))
                            } else {
                                // If the skill has no tags and we're filtering by tags, exclude it
                                false
                            }
                        })
                        .collect();
                }
                
                // Create the response
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "skills": skills,
                        "metadata": {
                            "total_count": skills.len()
                        }
                    }
                });
                
                let json = serde_json::to_string(&response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "skills/get" => {
                // Extract skill ID from request params
                let skill_id_opt = request.get("params").and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let skill_id = match skill_id_opt {
                    Some(id) => id.to_string(),
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing skill id"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Return the appropriate skill based on ID
                let skill = match skill_id.as_str() {
                    "test-skill-1" => AgentSkill {
                        id: "test-skill-1".to_string(),
                        name: "Echo".to_string(),
                        description: Some("Echoes back any message sent".to_string()),
                        tags: Some(vec!["basic".to_string(), "text".to_string()]),
                        examples: Some(vec!["Echo this message".to_string()]),
                        input_modes: Some(vec!["text/plain".to_string()]),
                        output_modes: Some(vec!["text/plain".to_string()]),
                    },
                    "test-skill-2" => AgentSkill {
                        id: "test-skill-2".to_string(),
                        name: "Summarize".to_string(),
                        description: Some("Summarizes long text content".to_string()),
                        tags: Some(vec!["text".to_string(), "analysis".to_string()]),
                        examples: Some(vec!["Summarize this article".to_string()]),
                        input_modes: Some(vec!["text/plain".to_string(), "text/html".to_string()]),
                        output_modes: Some(vec!["text/plain".to_string()]),
                    },
                    "test-skill-3" => AgentSkill {
                        id: "test-skill-3".to_string(),
                        name: "Image Generation".to_string(),
                        description: Some("Creates images from text descriptions".to_string()),
                        tags: Some(vec!["image".to_string(), "creative".to_string()]),
                        examples: Some(vec!["Generate an image of a sunset over mountains".to_string()]),
                        input_modes: Some(vec!["text/plain".to_string()]),
                        output_modes: Some(vec!["image/png".to_string(), "image/jpeg".to_string()]),
                    },
                    _ => {
                        // For unknown skill IDs, create a generic skill
                        AgentSkill {
                            id: skill_id.clone(),
                            name: format!("Unknown Skill ({})", skill_id),
                            description: Some("This skill is not recognized".to_string()),
                            tags: None,
                            examples: None,
                            input_modes: None,
                            output_modes: None,
                        }
                    }
                };
                
                // Create the response
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "skill": skill,
                        "metadata": {
                            "timestamp": Utc::now().to_rfc3339()
                        }
                    }
                });
                
                let json = serde_json::to_string(&response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "skills/invoke" => {
                // Extract skill ID and message from request params
                let skill_id_opt = request.get("params").and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let skill_id = match skill_id_opt {
                    Some(id) => id.to_string(),
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing skill id"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Make sure message exists
                if !request.get("params").and_then(|p| p.get("message")).is_some() {
                    let error = json!({
                        "jsonrpc": "2.0",
                        "id": request.get("id"),
                        "error": {
                            "code": -32602,
                            "message": "Invalid parameters: missing message"
                        }
                    });
                    let json = serde_json::to_string(&error).unwrap();
                    return Ok(Response::new(Body::from(json)));
                }
                
                // Create a new task ID
                let task_id = format!("skill-task-{}", chrono::Utc::now().timestamp_millis());
                
                // Get session ID if provided, otherwise generate one
                let session_id = request.get("params")
                    .and_then(|p| p.get("session_id"))
                    .and_then(|id| id.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("skill-session-{}", chrono::Utc::now().timestamp_millis()));
                
                // Create a new task with the skill execution
                let mut task = MockTask::new(&task_id, &session_id);
                
                // Update with working state
                task.update_status(TaskState::Working, None);
                
                // Create response based on skill ID
                let response_text = match skill_id.as_str() {
                    "test-skill-1" => {
                        // Echo skill - echo back the message content
                        let user_message = request.get("params")
                            .and_then(|p| p.get("message"))
                            .and_then(|m| m.get("parts"))
                            .and_then(|p| p.as_array())
                            .and_then(|parts| parts.first())
                            .and_then(|part| part.get("text"))
                            .and_then(|t| t.as_str())
                            .unwrap_or("Empty message");
                        
                        format!("Echo skill response: {}", user_message)
                    },
                    "test-skill-2" => {
                        // Summarize skill - return a mock summary
                        "Summary: This is a simulated summary of the content provided. The mock summarization skill extracts key points and condenses them into a concise format for easier comprehension.".to_string()
                    },
                    "test-skill-3" => {
                        // Image generation skill - describe the image that would be generated
                        "Image Generation: A vivid image has been created based on your description. In a real implementation, this would return an actual image file.".to_string()
                    },
                    _ => {
                        format!("Unknown skill '{}' - this is a simulated response for demonstration purposes.", skill_id)
                    }
                };
                
                // Create an artifact with the response
                let text_part = TextPart {
                    type_: "text".to_string(),
                    text: response_text.to_string(),
                    metadata: None,
                };
                
                let artifact = Artifact {
                    parts: vec![Part::TextPart(text_part)],
                    index: 0,
                    name: Some(format!("{}_response", skill_id)),
                    description: Some(format!("Response from skill: {}", skill_id)),
                    append: None,
                    last_chunk: None,
                    metadata: None,
                };
                
                // Add the artifact to the task
                task.add_artifact(artifact);
                
                // Update to completed state
                let completed_message = Message {
                    role: Role::Agent,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: format!("Skill '{}' executed successfully", skill_id),
                        metadata: None,
                    })],
                    metadata: None,
                };
                task.update_status(TaskState::Completed, Some(completed_message));
                
                // Store the task
                {
                    let mut storage = task_storage.lock().unwrap();
                    storage.insert(task_id.clone(), task.clone());
                }
                
                // Return the task
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": task.to_json(false)
                });
                
                let json = serde_json::to_string(&response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            // File Operations
            "files/upload" => {
                // Extract file data from request
                let file_data = match request.get("params").and_then(|p| p.get("file")) {
                    Some(data) => data,
                    None => {
                        // Return invalid parameters error
                        let error_response = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing file data",
                            }
                        });
                        
                        let json = serde_json::to_string(&error_response).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Extract file details
                let name = file_data.get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unnamed.file")
                    .to_string();
                    
                let mime_type = file_data.get("mimeType")
                    .and_then(|m| m.as_str())
                    .unwrap_or("application/octet-stream")
                    .to_string();
                    
                let content = file_data.get("bytes")
                    .and_then(|b| b.as_str())
                    .unwrap_or("")
                    .to_string();
                    
                // Extract optional metadata
                let metadata = request.get("params")
                    .and_then(|p| p.get("metadata"))
                    .and_then(|m| m.as_object().cloned());
                    
                // Extract optional task ID from metadata
                let task_id = metadata.as_ref()
                    .and_then(|m| m.get("taskId"))
                    .and_then(|t| t.as_str());
                    
                // Generate a file ID
                let file_id = format!("file-{}", uuid::Uuid::new_v4());
                
                // Create new file record
                let file = MockFile::new(
                    &file_id,
                    &name,
                    &mime_type,
                    &content,
                    task_id,
                    metadata.clone()
                );
                
                // Add file to storage
                let mut files = file_storage.lock().unwrap();
                files.insert(file_id.clone(), file.clone());
                
                // Create response
                let upload_response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": file.to_upload_json()
                });
                
                let json = serde_json::to_string(&upload_response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "files/download" => {
                // Extract file ID from request
                let file_id = match request.get("params").and_then(|p| p.get("fileId")).and_then(|id| id.as_str()) {
                    Some(id) => id,
                    None => {
                        // Return invalid parameters error
                        let error_response = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing fileId",
                            }
                        });
                        
                        let json = serde_json::to_string(&error_response).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Retrieve file from storage
                let files = file_storage.lock().unwrap();
                
                // Look up the file
                let file = match files.get(file_id) {
                    Some(f) => f.clone(),
                    None => {
                        // Return file not found error
                        let error_response = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32001,
                                "message": "File not found",
                            }
                        });
                        
                        let json = serde_json::to_string(&error_response).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Create response
                let download_response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": file.to_download_json()
                });
                
                let json = serde_json::to_string(&download_response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "files/list" => {
                // Extract optional task ID from request
                let task_id = request.get("params")
                    .and_then(|p| p.get("taskId"))
                    .and_then(|id| id.as_str());
                    
                // Get files from storage
                let files = file_storage.lock().unwrap();
                
                // Filter files by task ID if provided
                let filtered_files: Vec<Value> = files.values()
                    .filter(|file| {
                        if let Some(tid) = task_id {
                            file.task_id.as_ref().map_or(false, |id| id == tid)
                        } else {
                            true
                        }
                    })
                    .map(|file| file.to_upload_json())
                    .collect();
                    
                // Create response
                let list_response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "files": filtered_files
                    }
                });
                
                let json = serde_json::to_string(&list_response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            // Fallback for unhandled methods
            _ => {
                // Return method not found error
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "error": {
                        "code": -32601,
                        "message": "Method not found",
                    }
                });
                
                let json = serde_json::to_string(&error_response).unwrap();
                return Ok(Response::new(Body::from(json)));
            }
        }
    }
    
    // Return invalid request error
    let error_response = json!({
        "jsonrpc": "2.0",
        "id": null,
        "error": {
            "code": -32600,
            "message": "Invalid request",
        }
    });
    
    let json = serde_json::to_string(&error_response).unwrap();
    Ok(Response::new(Body::from(json)))
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

pub fn start_mock_server(port: u16) {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        
        // Create shared task storage
        let task_storage = create_task_storage();
        
        // Create shared batch storage
        let batch_storage = create_batch_storage();
        
        // Create shared file storage
        let file_storage = create_file_storage();
        
        // Clone storages for the make_service closure
        let ts = task_storage.clone();
        let bs = batch_storage.clone();
        let fs = file_storage.clone();
        
        let make_svc = make_service_fn(move |_conn| {
            // Clone storages for each service function
            let ts_clone = ts.clone();
            let bs_clone = bs.clone();
            let fs_clone = fs.clone();
            
            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    // Clone storages for each request
                    let ts_req = ts_clone.clone();
                    let bs_req = bs_clone.clone();
                    let fs_req = fs_clone.clone();
                    handle_a2a_request(ts_req, bs_req, fs_req, req)
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