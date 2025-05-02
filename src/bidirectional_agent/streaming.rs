//! Implementation of A2A-compliant Server-Sent Events (SSE) streaming
//! for the bidirectional agent.

use crate::bidirectional_agent::error::AgentError;
use crate::types::{Task, TaskStatusUpdateEvent, TaskArtifactUpdateEvent, Part, Artifact};
use futures_util::Stream;
use serde_json::{json, Value};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::sync::mpsc::error::SendError;
use tokio::time::sleep;
use tokio_stream::wrappers::ReceiverStream;
use std::fmt;

/// Type alias for the SSE stream output
pub type SseStream = Pin<Box<dyn Stream<Item = Result<String, StreamingError>> + Send>>;

/// Contains the types of streaming events that can be sent
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Task status update event
    Status {
        task_id: String,
        status: crate::types::TaskStatus,
        final_update: bool,
        metadata: Option<serde_json::Map<String, serde_json::Value>>,
    },
    /// Artifact update event
    Artifact {
        task_id: String,
        artifact: Artifact,
    },
    /// Error event
    Error {
        code: i64,
        message: String,
        data: Option<Value>,
    },
}

/// Errors that can occur during streaming
#[derive(Debug)]
pub enum StreamingError {
    /// JSON serialization error
    SerializationError(String),
    /// Channel send error
    SendError(String),
    /// General error
    Other(String),
}

impl fmt::Display for StreamingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamingError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            StreamingError::SendError(msg) => write!(f, "Send error: {}", msg),
            StreamingError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for StreamingError {}

impl From<serde_json::Error> for StreamingError {
    fn from(err: serde_json::Error) -> Self {
        StreamingError::SerializationError(err.to_string())
    }
}

impl<T> From<SendError<T>> for StreamingError {
    fn from(err: SendError<T>) -> Self {
        StreamingError::SendError(err.to_string())
    }
}

impl From<StreamingError> for AgentError {
    fn from(err: StreamingError) -> Self {
        match err {
            StreamingError::SerializationError(msg) => AgentError::SerializationError(msg),
            StreamingError::SendError(msg) => AgentError::Internal(format!("Streaming error: {}", msg)),
            StreamingError::Other(msg) => AgentError::Internal(format!("Streaming error: {}", msg)),
        }
    }
}

/// A formatter for Server-Sent Events
pub struct SseFormatter {
    request_id: Value,
}

impl SseFormatter {
    /// Create a new SSE formatter with the given request ID
    pub fn new(request_id: Value) -> Self {
        Self { request_id }
    }

    /// Format a streaming event as an SSE data event
    pub fn format_event(&self, event: &StreamEvent) -> Result<String, StreamingError> {
        match event {
            StreamEvent::Status { task_id, status, final_update, metadata } => {
                // Create TaskStatusUpdateEvent
                let update = TaskStatusUpdateEvent {
                    id: task_id.clone(),
                    status: status.clone(),
                    final_: *final_update,
                    metadata: metadata.clone(),
                };
                
                // Create JSON-RPC response
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": self.request_id.clone(),
                    "result": update
                });
                
                // Format as SSE data event
                Ok(format!("data: {}\n\n", serde_json::to_string(&response)?))
            },
            StreamEvent::Artifact { task_id, artifact } => {
                // Create TaskArtifactUpdateEvent
                let update = TaskArtifactUpdateEvent {
                    id: task_id.clone(),
                    artifact: artifact.clone()
                };
                
                // Create JSON-RPC response
                let response = json!({
                    "jsonrpc": "2.0", 
                    "id": self.request_id.clone(),
                    "result": update
                });
                
                // Format as SSE data event
                Ok(format!("data: {}\n\n", serde_json::to_string(&response)?))
            },
            StreamEvent::Error { code, message, data } => {
                // Create JSON-RPC error response
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": self.request_id.clone(),
                    "error": {
                        "code": code,
                        "message": message,
                        "data": data
                    }
                });
                
                // Format as SSE data event
                Ok(format!("data: {}\n\n", serde_json::to_string(&response)?))
            }
        }
    }

    /// Create a status event from a task
    pub fn create_status_event(&self, task: &Task, is_final: bool) -> StreamEvent {
        StreamEvent::Status {
            task_id: task.id.clone(),
            status: task.status.clone(),
            final_update: is_final,
            metadata: task.metadata.clone(),
        }
    }

    /// Create an artifact event
    pub fn create_artifact_event(&self, task_id: &str, artifact: Artifact) -> StreamEvent {
        StreamEvent::Artifact {
            task_id: task_id.to_string(),
            artifact,
        }
    }

    /// Create an error event
    pub fn create_error_event(&self, error: &AgentError) -> StreamEvent {
        StreamEvent::Error {
            code: error.code(),
            message: error.to_string(),
            data: Some(json!(format!("{:?}", error))),
        }
    }
}

/// Handles streaming tasks and sends events through the SSE channel
pub struct StreamingTaskHandler {
    tx: Sender<Result<String, StreamingError>>,
    formatter: Arc<SseFormatter>,
    task_id: String,
}

impl StreamingTaskHandler {
    /// Create a new streaming task handler
    pub fn new(request_id: Value, task_id: String) -> (Self, SseStream) {
        let (tx, rx) = mpsc::channel(32);
        let formatter = Arc::new(SseFormatter::new(request_id));
        
        let handler = Self {
            tx: tx.clone(),
            formatter,
            task_id,
        };
        
        // Convert receiver to a Stream
        let stream = Box::pin(ReceiverStream::new(rx)) as SseStream;
        
        (handler, stream)
    }
    
    /// Send a task status update event
    pub async fn send_status(&self, task: &Task, is_final: bool) -> Result<(), StreamingError> {
        let event = self.formatter.create_status_event(task, is_final);
        let message = self.formatter.format_event(&event)?;
        Ok(self.tx.send(Ok(message)).await?)
    }
    
    /// Send an artifact update event
    pub async fn send_artifact(&self, artifact: Artifact) -> Result<(), StreamingError> {
        let event = self.formatter.create_artifact_event(&self.task_id, artifact);
        let message = self.formatter.format_event(&event)?;
        Ok(self.tx.send(Ok(message)).await?)
    }
    
    /// Send an error event
    pub async fn send_error(&self, error: &AgentError) -> Result<(), StreamingError> {
        let event = self.formatter.create_error_event(error);
        let message = self.formatter.format_event(&event)?;
        Ok(self.tx.send(Ok(message)).await?)
    }

    /// Handle streaming for a task with support for generating multiple artifacts
    /// over time and streaming them back to the client
    pub async fn handle_streaming_task(
        &self,
        task: &Task, 
        task_repository: Arc<dyn crate::server::repositories::task_repository::TaskRepository + Send + Sync>,
    ) -> Result<(), AgentError> {
        // Clone the task ID for the closure
        let task_id = self.task_id.clone();
        let tx = self.tx.clone();
        let formatter = self.formatter.clone();
        
        // Send initial status event
        self.send_status(task, false).await?;
        
        // Launch a task to handle the streaming
        tokio::spawn(async move {
            // Check if the task requires input (for InputRequired state)
            let requires_input = if let Some(meta) = &task.metadata {
                meta.get("_mock_require_input").and_then(|v| v.as_bool()).unwrap_or(false)
            } else {
                false
            };
            
            // Check if there's a specified delay between chunks
            let chunk_delay = if let Some(meta) = &task.metadata {
                meta.get("_mock_chunk_delay_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(500)
            } else {
                500 // Default delay
            };
            
            // Check if there's a specified number of text chunks
            let num_chunks = if let Some(meta) = &task.metadata {
                meta.get("_mock_stream_text_chunks")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            } else {
                0
            };
            
            // Check if there's a specific final state
            let final_state = if let Some(meta) = &task.metadata {
                meta.get("_mock_stream_final_state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("completed")
            } else {
                "completed"
            };
            
            // If the task requires input, send an InputRequired state
            if requires_input {
                let mut task_clone = task.clone();
                task_clone.status.state = crate::types::TaskState::InputRequired;
                
                // Create a message if one doesn't exist
                if task_clone.status.message.is_none() {
                    task_clone.status.message = Some(crate::types::Message {
                        role: crate::types::Role::Agent,
                        parts: vec![Part::TextPart(crate::types::TextPart {
                            type_: "text".to_string(),
                            text: "Please provide additional information to continue.".to_string(),
                            metadata: None,
                        })],
                        metadata: None,
                    });
                }
                
                // Send InputRequired status
                let event = formatter.create_status_event(&task_clone, false);
                if let Ok(message) = formatter.format_event(&event) {
                    let _ = tx.send(Ok(message)).await;
                }
                
                // Update task status in repository
                let _ = task_repository.save_task(&task_clone).await;
                let _ = task_repository.save_state_history(&task_id, &task_clone).await;
                
                // Wait for a long time (in a real implementation, this would wait for client input)
                sleep(Duration::from_secs(300)).await;
            }
            
            // Generate stream chunks if specified
            if num_chunks > 0 {
                for i in 1..=num_chunks {
                    // Create text artifact
                    let artifact = Artifact {
                        parts: vec![Part::TextPart(crate::types::TextPart {
                            type_: "text".to_string(), 
                            text: format!("Streaming content chunk {}", i),
                            metadata: None,
                        })],
                        index: i as i64 - 1,
                        name: Some(format!("chunk_{}", i)),
                        description: Some("Generated streaming content".to_string()),
                        append: if i > 1 { Some(true) } else { None },
                        last_chunk: if i == num_chunks { Some(true) } else { Some(false) },
                        metadata: None,
                    };
                    
                    // Send artifact update
                    let event = formatter.create_artifact_event(&task_id, artifact.clone());
                    if let Ok(message) = formatter.format_event(&event) {
                        let _ = tx.send(Ok(message)).await;
                    }
                    
                    // Add to task artifacts in repository
                    if let Ok(mut task) = task_repository.get_task(&task_id).await {
                        if let Some(mut task) = task {
                            if task.artifacts.is_none() {
                                task.artifacts = Some(vec![]);
                            }
                            if let Some(artifacts) = &mut task.artifacts {
                                artifacts.push(artifact);
                            }
                            let _ = task_repository.save_task(&task).await;
                        }
                    }
                    
                    // Wait before sending next chunk
                    sleep(Duration::from_millis(chunk_delay)).await;
                }
            }
            
            // Send final status update
            if let Ok(mut task) = task_repository.get_task(&task_id).await {
                if let Some(task) = &mut task {
                    // Update the task status based on the final_state parameter
                    match final_state {
                        "completed" => {
                            task.status.state = crate::types::TaskState::Completed;
                        },
                        "failed" => {
                            task.status.state = crate::types::TaskState::Failed;
                        },
                        "canceled" => {
                            task.status.state = crate::types::TaskState::Canceled;
                        },
                        _ => {
                            task.status.state = crate::types::TaskState::Completed;
                        }
                    }
                    
                    // Update the task status in the repository
                    let _ = task_repository.save_task(&task).await;
                    let _ = task_repository.save_state_history(&task_id, &task).await;
                    
                    // Send final status update
                    let event = formatter.create_status_event(&task, true);
                    if let Ok(message) = formatter.format_event(&event) {
                        let _ = tx.send(Ok(message)).await;
                    }
                }
            }
        });
        
        Ok(())
    }
    
    /// Handle resubscription to an existing task
    pub async fn handle_resubscribe(
        &self,
        task: &Task,
        task_repository: Arc<dyn crate::server::repositories::task_repository::TaskRepository + Send + Sync>,
    ) -> Result<(), AgentError> {
        // Clone necessary variables for the async task
        let task_id = self.task_id.clone();
        let tx = self.tx.clone();
        let formatter = self.formatter.clone();
        let task_clone = task.clone();
        
        // Send initial status update
        self.send_status(task, false).await?;
        
        // Check if the task is already in a final state
        let is_final = matches!(
            task.status.state,
            crate::types::TaskState::Completed | 
            crate::types::TaskState::Failed | 
            crate::types::TaskState::Canceled
        );
        
        // If task is already in a final state, just send a final update
        if is_final {
            // Send the final status event after a small delay
            tokio::spawn(async move {
                sleep(Duration::from_millis(100)).await;
                
                let event = formatter.create_status_event(&task_clone, true);
                if let Ok(message) = formatter.format_event(&event) {
                    let _ = tx.send(Ok(message)).await;
                }
            });
            
            return Ok(());
        }
        
        // For tasks that are still in progress, handle streaming similar to a new task
        tokio::spawn(async move {
            // Check if there's a specified delay between chunks
            let chunk_delay = if let Some(meta) = &task_clone.metadata {
                meta.get("_mock_chunk_delay_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(500)
            } else {
                500 // Default delay
            };
            
            // Send any existing artifacts
            if let Some(artifacts) = &task_clone.artifacts {
                for artifact in artifacts {
                    let event = formatter.create_artifact_event(&task_id, artifact.clone());
                    if let Ok(message) = formatter.format_event(&event) {
                        let _ = tx.send(Ok(message)).await;
                        sleep(Duration::from_millis(chunk_delay)).await;
                    }
                }
            }
            
            // Check if there's a specified final state
            let final_state = if let Some(meta) = &task_clone.metadata {
                meta.get("_mock_stream_final_state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("completed")
            } else {
                "completed"
            };
            
            // Update and send final state
            if let Ok(mut task) = task_repository.get_task(&task_id).await {
                if let Some(task) = &mut task {
                    match final_state {
                        "completed" => {
                            task.status.state = crate::types::TaskState::Completed;
                        },
                        "failed" => {
                            task.status.state = crate::types::TaskState::Failed;
                        },
                        "canceled" => {
                            task.status.state = crate::types::TaskState::Canceled;
                        },
                        _ => {
                            task.status.state = crate::types::TaskState::Completed;
                        }
                    }
                    
                    // Update the task status in the repository
                    let _ = task_repository.save_task(&task).await;
                    let _ = task_repository.save_state_history(&task_id, &task).await;
                    
                    // Send final status update
                    let event = formatter.create_status_event(&task, true);
                    if let Ok(message) = formatter.format_event(&event) {
                        let _ = tx.send(Ok(message)).await;
                    }
                }
            }
        });
        
        Ok(())
    }
}

/// A Stream implementation that can be used to send SSE events
pub struct StreamingResponseStream {
    receiver: Mutex<ReceiverStream<Result<String, StreamingError>>>,
}

impl StreamingResponseStream {
    /// Create a new streaming response stream
    pub fn new(rx: Receiver<Result<String, StreamingError>>) -> Self {
        Self {
            receiver: Mutex::new(ReceiverStream::new(rx)),
        }
    }
}

impl Stream for StreamingResponseStream {
    type Item = Result<String, StreamingError>;
    
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut receiver = self.receiver.lock().unwrap();
        Pin::new(&mut *receiver).poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TaskStatus, TaskState, Message, Role, TextPart};
    use futures_util::StreamExt;
    use serde_json::json;
    use std::sync::Arc;
    use tokio::time::timeout;
    use tokio_test::block_on;
    use chrono::Utc;
    use std::time::Duration;
    
    // Mock task repository for testing
    struct MockTaskRepository {
        task: Arc<Mutex<Option<Task>>>,
    }
    
    impl MockTaskRepository {
        fn new(task: Task) -> Self {
            Self {
                task: Arc::new(Mutex::new(Some(task))),
            }
        }
    }
    
    #[async_trait::async_trait]
    impl crate::server::repositories::task_repository::TaskRepository for MockTaskRepository {
        async fn get_task(&self, _task_id: &str) -> Result<Option<Task>, crate::server::ServerError> {
            let task = self.task.lock().unwrap().clone();
            Ok(task)
        }
        
        async fn save_task(&self, task: &Task) -> Result<(), crate::server::ServerError> {
            let mut t = self.task.lock().unwrap();
            *t = Some(task.clone());
            Ok(())
        }
        
        async fn save_state_history(&self, _task_id: &str, _task: &Task) -> Result<(), crate::server::ServerError> {
            Ok(())
        }
        
        async fn get_task_state_history(&self, _task_id: &str) -> Result<Vec<Task>, crate::server::ServerError> {
            Ok(vec![])
        }
        
        async fn delete_task(&self, _task_id: &str) -> Result<(), crate::server::ServerError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_streaming_task_handler() {
        // Create a test task
        let task = Task {
            id: "test-task-123".to_string(),
            session_id: Some("session-1".to_string()),
            status: TaskStatus {
                state: TaskState::Working,
                timestamp: Some(Utc::now()),
                message: Some(Message {
                    role: Role::Agent,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: "Working on your request...".to_string(),
                        metadata: None,
                    })],
                    metadata: None,
                }),
            },
            artifacts: None,
            metadata: Some(json!({
                "_mock_stream_text_chunks": 2,
                "_mock_chunk_delay_ms": 50
            }).as_object().unwrap().clone()),
        };
        
        // Create mock task repository
        let repo = Arc::new(MockTaskRepository::new(task.clone()));
        
        // Create streaming handler
        let (handler, mut stream) = StreamingTaskHandler::new(json!(1), task.id.clone());
        
        // Start streaming
        handler.handle_streaming_task(&task, repo.clone()).await.unwrap();
        
        // Collect streaming events with a timeout
        let mut events = Vec::new();
        while let Ok(Some(result)) = timeout(Duration::from_millis(500), stream.next()).await {
            if let Ok(event) = result {
                events.push(event);
            } else {
                break;
            }
        }
        
        // Should have at least 3 events (initial status, 2 chunks)
        assert!(events.len() >= 3, "Expected at least 3 events, got {}", events.len());
        
        // First event should be the status update
        let first_event = &events[0];
        assert!(first_event.contains("\"id\":\"test-task-123\""));
        assert!(first_event.contains("\"final\":false"));
        
        // Second and third events should be artifact updates
        let second_event = &events[1];
        assert!(second_event.contains("\"artifact\""));
        assert!(second_event.contains("Streaming content chunk 1"));
        
        let third_event = &events[2];
        assert!(third_event.contains("\"artifact\""));
        assert!(third_event.contains("Streaming content chunk 2"));
        
        // Last event should be the final status update
        if events.len() >= 4 {
            let final_event = &events[events.len() - 1];
            assert!(final_event.contains("\"final\":true"));
        }
    }
    
    #[tokio::test]
    async fn test_resubscribe_handler() {
        // Create a completed task
        let task = Task {
            id: "completed-task".to_string(),
            session_id: Some("session-1".to_string()),
            status: TaskStatus {
                state: TaskState::Completed,
                timestamp: Some(Utc::now()),
                message: Some(Message {
                    role: Role::Agent,
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: "Task is complete".to_string(),
                        metadata: None,
                    })],
                    metadata: None,
                }),
            },
            artifacts: Some(vec![
                Artifact {
                    parts: vec![Part::TextPart(TextPart {
                        type_: "text".to_string(),
                        text: "Result data".to_string(),
                        metadata: None,
                    })],
                    index: 0,
                    name: Some("result".to_string()),
                    description: Some("Task result".to_string()),
                    append: None,
                    last_chunk: Some(true),
                    metadata: None,
                }
            ]),
            metadata: None,
        };
        
        // Create mock task repository
        let repo = Arc::new(MockTaskRepository::new(task.clone()));
        
        // Create streaming handler
        let (handler, mut stream) = StreamingTaskHandler::new(json!(2), task.id.clone());
        
        // Start resubscribe handler
        handler.handle_resubscribe(&task, repo.clone()).await.unwrap();
        
        // Collect streaming events with a timeout
        let mut events = Vec::new();
        while let Ok(Some(result)) = timeout(Duration::from_millis(500), stream.next()).await {
            if let Ok(event) = result {
                events.push(event);
            } else {
                break;
            }
        }
        
        // Should have 2 events (initial status, final status)
        assert!(events.len() >= 2, "Expected at least 2 events, got {}", events.len());
        
        // First event should be the status update
        let first_event = &events[0];
        assert!(first_event.contains("\"id\":\"completed-task\""));
        
        // Last event should be the final status update
        let last_event = &events[events.len() - 1];
        assert!(last_event.contains("\"final\":true"));
        assert!(last_event.contains("\"state\":\"completed\""));
    }
}