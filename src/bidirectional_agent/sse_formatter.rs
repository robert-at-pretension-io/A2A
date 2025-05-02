// sse_formatter.rs - A2A Protocol Compliant Server-Sent Events Formatter
//
// This file provides utilities for formatting Server-Sent Events (SSE) according
// to the A2A protocol specification, ensuring correct event formatting for
// streaming task updates.

use crate::types::{
    Task, TaskStatus, Artifact, JsonrpcMessage, SendTaskStreamingResponse,
    TaskStatusUpdateEvent, TaskArtifactUpdateEvent, SendTaskStreamingResponseResult,
};
use serde_json::Value;
use chrono::Utc;
use bytes::Bytes;
use std::time::Duration;
use futures::stream::{Stream, StreamExt};
use futures::channel::mpsc::{self, Sender, Receiver};
use std::pin::Pin;

/// Type alias for SSE event stream
pub type SseEventStream = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;

/// Represents different types of streaming events
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Task status update
    Status {
        /// Task ID
        task_id: String,
        /// Task status
        status: TaskStatus,
        /// True if this is the final update
        final_update: bool,
    },
    
    /// Artifact update
    Artifact {
        /// Task ID
        task_id: String,
        /// Artifact
        artifact: Artifact,
    },
    
    /// Task completion
    Complete {
        /// The completed task
        task: Task,
    },
    
    /// Error event
    Error {
        /// Error code
        code: i64,
        /// Error message
        message: String,
        /// Optional additional data
        data: Option<Value>,
    },
    
    /// Keep-alive event (empty but maintains connection)
    KeepAlive,
}

/// Formats SSE events according to the A2A protocol specification
pub struct SseFormatter {
    /// JSON-RPC request ID for this stream
    request_id: Option<Value>,
}

impl SseFormatter {
    /// Create a new SSE formatter with the given request ID
    pub fn new(request_id: Option<Value>) -> Self {
        Self { request_id }
    }
    
    /// Format a stream event as an SSE event
    pub fn format_event(&self, event: &StreamEvent) -> String {
        // Format the event data according to the A2A protocol
        match event {
            StreamEvent::Status { task_id, status, final_update } => {
                // Create TaskStatusUpdateEvent
                let update = TaskStatusUpdateEvent {
                    id: task_id.clone(),
                    status: status.clone(),
                    final_: *final_update,
                    metadata: None,
                };
                
                // Create SendTaskStreamingResponse
                let response = SendTaskStreamingResponse {
                    jsonrpc: "2.0".to_string(),
                    id: self.request_id.clone().map(|v| v.into()),
                    result: SendTaskStreamingResponseResult::Variant0(update),
                    error: None,
                };
                
                // Format as SSE data event
                self.format_data_event(&response)
            },
            StreamEvent::Artifact { task_id, artifact } => {
                // Create TaskArtifactUpdateEvent
                let update = TaskArtifactUpdateEvent {
                    id: task_id.clone(),
                    artifact: artifact.clone(),
                    metadata: None,
                };
                
                // Create SendTaskStreamingResponse
                let response = SendTaskStreamingResponse {
                    jsonrpc: "2.0".to_string(),
                    id: self.request_id.clone().map(|v| v.into()),
                    result: SendTaskStreamingResponseResult::Variant1(update),
                    error: None,
                };
                
                // Format as SSE data event
                self.format_data_event(&response)
            },
            StreamEvent::Complete { task } => {
                // Create final status update
                let update = TaskStatusUpdateEvent {
                    id: task.id.clone(),
                    status: task.status.clone(),
                    final_: true,
                    metadata: None,
                };
                
                // Create SendTaskStreamingResponse
                let response = SendTaskStreamingResponse {
                    jsonrpc: "2.0".to_string(),
                    id: self.request_id.clone().map(|v| v.into()),
                    result: SendTaskStreamingResponseResult::Variant0(update),
                    error: None,
                };
                
                // Format as SSE data event
                self.format_data_event(&response)
            },
            StreamEvent::Error { code, message, data } => {
                // Create error response
                let response = SendTaskStreamingResponse {
                    jsonrpc: "2.0".to_string(),
                    id: self.request_id.clone().map(|v| v.into()),
                    result: SendTaskStreamingResponseResult::Variant2,
                    error: Some(serde_json::json!({
                        "code": code,
                        "message": message,
                        "data": data
                    })),
                };
                
                // Format as SSE data event
                self.format_data_event(&response)
            },
            StreamEvent::KeepAlive => {
                // Format as SSE comment for keep-alive
                String::from(": keep-alive\n\n")
            },
        }
    }
    
    /// Format a serializable object as an SSE data event
    fn format_data_event<T: serde::Serialize>(&self, data: &T) -> String {
        let json_str = match serde_json::to_string(data) {
            Ok(s) => s,
            Err(e) => {
                // Fallback for serialization errors
                format!("{{\"jsonrpc\":\"2.0\",\"error\":{{\"code\":-32603,\"message\":\"Serialization error: {}\"}}}}", e)
            }
        };
        
        // Format as proper SSE data event
        format!("data: {}\n\n", json_str)
    }
    
    /// Create an SSE stream from a channel of stream events
    pub fn create_stream(
        request_id: Option<Value>,
        mut event_rx: Receiver<StreamEvent>,
    ) -> SseEventStream {
        let formatter = Self::new(request_id);
        
        // Create a stream from the receiver that formats events
        let stream = async_stream::stream! {
            while let Some(event) = event_rx.next().await {
                let event_str = formatter.format_event(&event);
                yield Ok(Bytes::from(event_str));
            }
        };
        
        Box::pin(stream)
    }
    
    /// Create a stream event channel
    pub fn create_channel() -> (Sender<StreamEvent>, Receiver<StreamEvent>) {
        mpsc::channel(100) // Buffer of 100 events
    }
    
    /// Start a keep-alive task that sends periodic events
    pub async fn start_keep_alive(tx: Sender<StreamEvent>, interval: Duration) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(interval);
            loop {
                interval.tick().await;
                // If send fails, the receiver is closed, so break the loop
                if tx.clone().send(StreamEvent::KeepAlive).await.is_err() {
                    break;
                }
            }
        });
    }
}

/// Provides a high-level streaming handler for tasks
pub struct StreamingTaskHandler {
    /// Stream event sender
    sender: Sender<StreamEvent>,
    /// JSON-RPC request ID
    request_id: Option<Value>,
    /// Task ID being streamed
    task_id: String,
}

impl StreamingTaskHandler {
    /// Create a new streaming task handler
    pub fn new(sender: Sender<StreamEvent>, request_id: Option<Value>, task_id: String) -> Self {
        Self { sender, request_id, task_id }
    }
    
    /// Send a status update for the task
    pub async fn send_status(&self, status: TaskStatus, final_update: bool) -> Result<(), String> {
        self.sender.clone()
            .send(StreamEvent::Status {
                task_id: self.task_id.clone(),
                status,
                final_update,
            })
            .await
            .map_err(|e| format!("Failed to send status update: {}", e))
    }
    
    /// Send an artifact update for the task
    pub async fn send_artifact(&self, artifact: Artifact) -> Result<(), String> {
        self.sender.clone()
            .send(StreamEvent::Artifact {
                task_id: self.task_id.clone(),
                artifact,
            })
            .await
            .map_err(|e| format!("Failed to send artifact update: {}", e))
    }
    
    /// Send a completion event for the task
    pub async fn send_completion(&self, task: Task) -> Result<(), String> {
        self.sender.clone()
            .send(StreamEvent::Complete { task })
            .await
            .map_err(|e| format!("Failed to send completion event: {}", e))
    }
    
    /// Send an error event
    pub async fn send_error(&self, code: i64, message: &str, data: Option<Value>) -> Result<(), String> {
        self.sender.clone()
            .send(StreamEvent::Error {
                code,
                message: message.to_string(),
                data,
            })
            .await
            .map_err(|e| format!("Failed to send error event: {}", e))
    }
    
    /// Create an SSE stream from a task processing iterator
    /// 
    /// This is a convenience method for creating a stream from a task
    /// that will have its status and artifacts updated over time.
    pub fn create_task_stream(
        request_id: Option<Value>,
        task_id: String,
    ) -> (Self, SseEventStream) {
        // Create channel
        let (tx, rx) = SseFormatter::create_channel();
        
        // Create stream
        let stream = SseFormatter::create_stream(request_id.clone(), rx);
        
        // Create handler
        let handler = Self::new(tx, request_id, task_id);
        
        (handler, stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Part, TextPart, Role};
    use futures::executor::block_on;
    use futures::StreamExt;
    
    #[test]
    fn test_format_status_event() {
        // Create a formatter
        let formatter = SseFormatter::new(Some(serde_json::json!(1)));
        
        // Create a status update event
        let status = TaskStatus {
            state: crate::types::TaskState::Working,
            timestamp: Some(Utc::now()),
            message: None,
        };
        
        let event = StreamEvent::Status {
            task_id: "test-task".to_string(),
            status,
            final_update: false,
        };
        
        // Format the event
        let event_str = formatter.format_event(&event);
        
        // Verify format follows SSE spec
        assert!(event_str.starts_with("data: "));
        assert!(event_str.ends_with("\n\n"));
        
        // Verify content is valid JSON
        let json_str = event_str.strip_prefix("data: ").unwrap().strip_suffix("\n\n").unwrap();
        let json: Value = serde_json::from_str(json_str).expect("Should be valid JSON");
        
        // Verify JSON structure follows A2A protocol
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 1);
        assert_eq!(json["result"]["id"], "test-task");
        assert_eq!(json["result"]["status"]["state"], "working");
        assert_eq!(json["result"]["final"], false);
    }
    
    #[test]
    fn test_format_artifact_event() {
        // Create a formatter
        let formatter = SseFormatter::new(Some(serde_json::json!("abc123")));
        
        // Create an artifact
        let artifact = Artifact {
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: "Test artifact content".to_string(),
                metadata: None,
            })],
            index: 0,
            name: Some("test-artifact".to_string()),
            description: None,
            append: None,
            last_chunk: None,
            metadata: None,
        };
        
        // Create an artifact update event
        let event = StreamEvent::Artifact {
            task_id: "test-task".to_string(),
            artifact,
        };
        
        // Format the event
        let event_str = formatter.format_event(&event);
        
        // Verify format follows SSE spec
        assert!(event_str.starts_with("data: "));
        assert!(event_str.ends_with("\n\n"));
        
        // Verify content is valid JSON
        let json_str = event_str.strip_prefix("data: ").unwrap().strip_suffix("\n\n").unwrap();
        let json: Value = serde_json::from_str(json_str).expect("Should be valid JSON");
        
        // Verify JSON structure follows A2A protocol
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], "abc123");
        assert_eq!(json["result"]["id"], "test-task");
        assert_eq!(json["result"]["artifact"]["index"], 0);
        assert_eq!(json["result"]["artifact"]["name"], "test-artifact");
        let part = &json["result"]["artifact"]["parts"][0];
        assert_eq!(part["type_"], "text");
        assert_eq!(part["text"], "Test artifact content");
    }
    
    #[test]
    fn test_format_error_event() {
        // Create a formatter
        let formatter = SseFormatter::new(Some(serde_json::json!(42)));
        
        // Create an error event
        let event = StreamEvent::Error {
            code: -32001,
            message: "Task not found".to_string(),
            data: Some(serde_json::json!({"task_id": "missing-task"})),
        };
        
        // Format the event
        let event_str = formatter.format_event(&event);
        
        // Verify format follows SSE spec
        assert!(event_str.starts_with("data: "));
        assert!(event_str.ends_with("\n\n"));
        
        // Verify content is valid JSON
        let json_str = event_str.strip_prefix("data: ").unwrap().strip_suffix("\n\n").unwrap();
        let json: Value = serde_json::from_str(json_str).expect("Should be valid JSON");
        
        // Verify JSON structure follows A2A protocol
        assert_eq!(json["jsonrpc"], "2.0");
        assert_eq!(json["id"], 42);
        assert_eq!(json["error"]["code"], -32001);
        assert_eq!(json["error"]["message"], "Task not found");
        assert_eq!(json["error"]["data"]["task_id"], "missing-task");
    }
    
    #[test]
    fn test_format_keep_alive_event() {
        // Create a formatter
        let formatter = SseFormatter::new(Some(serde_json::json!(1)));
        
        // Create a keep-alive event
        let event = StreamEvent::KeepAlive;
        
        // Format the event
        let event_str = formatter.format_event(&event);
        
        // Verify format follows SSE spec for comments
        assert_eq!(event_str, ": keep-alive\n\n");
    }
    
    #[tokio::test]
    async fn test_streaming_task_handler() {
        // Create a task handler with stream
        let (handler, mut stream) = StreamingTaskHandler::create_task_stream(
            Some(serde_json::json!(99)),
            "stream-test-task".to_string(),
        );
        
        // Send a status update
        let status = TaskStatus {
            state: crate::types::TaskState::Working,
            timestamp: Some(Utc::now()),
            message: None,
        };
        handler.send_status(status, false).await.unwrap();
        
        // Read from the stream
        let event_bytes = stream.next().await.unwrap().unwrap();
        let event_str = String::from_utf8(event_bytes.to_vec()).unwrap();
        
        // Verify the event
        assert!(event_str.starts_with("data: "));
        assert!(event_str.ends_with("\n\n"));
        
        // Verify JSON content
        let json_str = event_str.strip_prefix("data: ").unwrap().strip_suffix("\n\n").unwrap();
        let json: Value = serde_json::from_str(json_str).expect("Should be valid JSON");
        assert_eq!(json["result"]["id"], "stream-test-task");
        assert_eq!(json["result"]["status"]["state"], "working");
    }
}