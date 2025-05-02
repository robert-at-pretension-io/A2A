use std::error::Error;
use std::pin::Pin;
use bytes::Bytes;
use futures_util::stream::{Stream, StreamExt};
use futures_util::TryStreamExt;
use eventsource_stream::Eventsource;
use crate::types::{Part, TextPart};
use serde_json::{json, Value};

use crate::client::A2aClient;
use crate::client::errors::{ClientError, A2aError}; 
use crate::client::error_handling::{ErrorCompatibility, self};
use crate::types::{Message, Task, Artifact, TaskSendParams, TaskQueryParams};

/// Response from a streaming task operation
#[derive(Debug, Clone)]
pub enum StreamingResponse {
    /// Task status update
    Status(Task),
    /// New content artifact
    Artifact(Artifact),
    /// Final response (stream end)
    Final(Task),
}

/// Type alias for a streaming response stream (using ClientError)
pub type StreamingResponseStream = Pin<Box<dyn Stream<Item = Result<StreamingResponse, ClientError>> + Send>>;

impl A2aClient {
    /// Send a task with streaming response enabled via SSE (typed error version)
    pub async fn send_task_subscribe_typed(&mut self, text: &str) -> Result<StreamingResponseStream, ClientError> {
        // Call with empty metadata
        self.send_task_subscribe_with_metadata_typed(text, &json!({})).await
    }

    /// Send a task with streaming response enabled via SSE (backward compatible)
    pub async fn send_task_subscribe(&mut self, text: &str) -> Result<StreamingResponseStream, Box<dyn Error>> {
        match self.send_task_subscribe_typed(text).await {
            Ok(val) => Ok(val),
            Err(err) => Err(Box::new(err))
        }
    }

    /// Send a task with streaming response enabled via SSE and optional metadata as JSON string (backward compatible)
    /// 
    /// This method is kept for backward compatibility.
    /// 
    /// # Arguments
    /// * `text` - The message text to send 
    /// * `metadata_json` - Optional JSON string containing metadata
    ///
    /// # Examples
    /// ```
    /// // Stream with 1-second delay between chunks
    /// let stream = client.send_task_subscribe_with_metadata_str(
    ///     "Stream with slow delivery",
    ///     Some(r#"{"_mock_chunk_delay_ms": 1000}"#)
    /// ).await?;
    /// ```
    pub async fn send_task_subscribe_with_metadata_str(&mut self, text: &str, metadata_json: Option<&str>) -> Result<StreamingResponseStream, Box<dyn Error>> {
        // Parse metadata if provided
        let metadata = if let Some(meta_str) = metadata_json {
            match serde_json::from_str(meta_str) {
                Ok(parsed) => parsed,
                Err(e) => return Err(format!("Failed to parse metadata JSON: {}", e).into())
            }
        } else {
            json!({})
        };
        
        self.send_task_subscribe_with_metadata(text, &metadata).await
    }

    /// Send a task with streaming response enabled via SSE and metadata (typed error version)
    /// 
    /// Metadata can include testing parameters like:
    /// - `_mock_delay_ms`: Simulates initial request delay
    /// - `_mock_chunk_delay_ms`: Controls delay between streamed chunks
    /// - `_mock_stream_text_chunks`: Number of text chunks to generate
    /// - `_mock_stream_artifact_types`: Types of artifacts to generate (text, data, file)
    /// - `_mock_stream_final_state`: Final state of the stream (completed, failed)
    /// 
    /// # Arguments
    /// * `text` - The message text to send 
    /// * `metadata` - JSON Value containing metadata
    ///
    /// # Examples
    /// ```
    /// // Stream with dynamic content configuration
    /// let stream = client.send_task_subscribe_with_metadata_typed(
    ///     "Stream with dynamic configuration",
    ///     &json!({
    ///         "_mock_stream_text_chunks": 3,
    ///         "_mock_stream_artifact_types": ["text", "data"],
    ///         "_mock_stream_final_state": "completed"
    ///     })
    /// ).await?;
    /// ```
    pub async fn send_task_subscribe_with_metadata_typed(&mut self, text: &str, metadata: &serde_json::Value) -> Result<StreamingResponseStream, ClientError> {
        // Create the message with the text content
        let message = self.create_text_message(text);
        
        // Use the provided metadata
        // Convert Value to Map if needed
        let metadata_map = match metadata {
            serde_json::Value::Object(map) => Some(map.clone()),
            _ => {
                // Convert other Value types to a Map with a "_data" key
                let mut map = serde_json::Map::new();
                map.insert("_data".to_string(), metadata.clone());
                Some(map)
            }
        };
        
        // Create request parameters using the proper TaskSendParams type
        let params = TaskSendParams {
            id: uuid::Uuid::new_v4().to_string(),
            message: message,
            history_length: None,
            metadata: metadata_map,
            push_notification: None,
            session_id: None,
        };
        
        // Build the SSE request
        self.send_streaming_request_typed("tasks/sendSubscribe", serde_json::to_value(params)?).await
    }

    /// Send a task with streaming response enabled via SSE and metadata (backward compatible)
    pub async fn send_task_subscribe_with_metadata(&mut self, text: &str, metadata: &serde_json::Value) -> Result<StreamingResponseStream, Box<dyn Error>> {
        match self.send_task_subscribe_with_metadata_typed(text, metadata).await {
            Ok(val) => Ok(val),
            Err(err) => Err(Box::new(err))
        }
    }

    /// Resubscribe to an existing task's streaming updates (typed error version)
    pub async fn resubscribe_task_typed(&mut self, task_id: &str) -> Result<StreamingResponseStream, ClientError> {
        // Call with no metadata
        self.resubscribe_task_with_metadata_typed(task_id, &json!({})).await
    }

    /// Resubscribe to an existing task's streaming updates (backward compatible)
    pub async fn resubscribe_task(&mut self, task_id: &str) -> Result<StreamingResponseStream, Box<dyn Error>> {
        match self.resubscribe_task_typed(task_id).await {
            Ok(val) => Ok(val),
            Err(err) => Err(Box::new(err))
        }
    }

    /// Resubscribe to a task's streaming updates with metadata (typed error version)
    /// 
    /// Metadata can include testing parameters like:
    /// - `_mock_stream_text_chunks`: Number of text chunks to generate
    /// - `_mock_stream_artifact_types`: Types of artifacts to generate (text, data, file)
    /// - `_mock_stream_chunk_delay_ms`: Delay between chunks in milliseconds
    /// - `_mock_stream_final_state`: Final state of the stream (completed, failed)
    ///
    /// # Arguments
    /// * `task_id` - The ID of the task to resubscribe to
    /// * `metadata` - JSON value containing metadata
    pub async fn resubscribe_task_with_metadata_typed(&mut self, task_id: &str, metadata: &serde_json::Value) -> Result<StreamingResponseStream, ClientError> {
        // Create request parameters using the proper TaskQueryParams type
        // Convert Value to Map if needed
        let metadata_map = match metadata {
            serde_json::Value::Object(map) => Some(map.clone()),
            _ => {
                // Convert other Value types to a Map with a "_data" key
                let mut map = serde_json::Map::new();
                map.insert("_data".to_string(), metadata.clone());
                Some(map)
            }
        };
        
        let params = TaskQueryParams {
            id: task_id.to_string(),
            history_length: None,
            metadata: metadata_map
        };

        // Build the SSE request
        self.send_streaming_request_typed("tasks/resubscribe", serde_json::to_value(params)?).await
    }

    /// Resubscribe to a task's streaming updates with metadata (backward compatible)
    pub async fn resubscribe_task_with_metadata(&mut self, task_id: &str, metadata: &serde_json::Value) -> Result<StreamingResponseStream, Box<dyn Error>> {
        match self.resubscribe_task_with_metadata_typed(task_id, metadata).await {
            Ok(val) => Ok(val),
            Err(err) => Err(Box::new(err))
        }
    }

    /// Send a streaming request and return a stream of responses (typed error version)
    pub async fn send_streaming_request_typed(&mut self, method: &str, params: Value) -> Result<StreamingResponseStream, ClientError> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_request_id(),
            "method": method,
            "params": params
        });
        
        let mut http_request = self.http_client
            .post(&self.base_url)
            .header("Accept", "text/event-stream")
            .json(&request);
            
        if let (Some(header), Some(value)) = (&self.auth_header, &self.auth_value) {
            http_request = http_request.header(header, value);
        }
        
        // Send the request and get a streaming response
        let response = http_request.send().await?;

        if !response.status().is_success() {
            return Err(ClientError::ReqwestError { msg: format!("Request failed with status: {}", response.status()), status_code: Some(response.status().as_u16()) });
        }

        // Get the response as a byte stream
        let byte_stream = response.bytes_stream();
        
        // Convert to an SSE stream
        let event_stream = byte_stream
            .eventsource()
            .map_err(|e| ClientError::Other(format!("SSE stream error: {}", e))); // Convert SSE error to ClientError

        // Transform the SSE events to StreamingResponse objects
        let streaming_response = event_stream.map(|event_result| {
            match event_result {
                Ok(event) => {
                    // Parse the event data as JSON
                    match serde_json::from_str::<Value>(&event.data) {
                        Ok(json_data) => {
                            // Check for JSON-RPC errors
                            if let Some(error) = json_data.get("error") {
                                let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
                                let message = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
                                let data = error.get("data").cloned();
                                // Return ClientError::A2aError
                                return Err(ClientError::A2aError(A2aError::new(code, message, data)));
                            }

                            // Get the result field
                            if let Some(result) = json_data.get("result") {
                                // Check if this is an artifact update
                                if let Some(artifact) = result.get("artifact") {
                                    match serde_json::from_value::<Artifact>(artifact.clone()) {
                                        Ok(artifact_obj) => Ok(StreamingResponse::Artifact(artifact_obj)),
                                        Err(e) => Err(ClientError::JsonError(format!("Failed to parse artifact: {}", e))) // Corrected error type
                                    }
                                }
                                // Check if this has a final flag
                                else if let Some(is_final) = result.get("final").and_then(|f| f.as_bool()) {
                                    if is_final {
                                        match serde_json::from_value::<Task>(result.clone()) {
                                            Ok(task) => Ok(StreamingResponse::Final(task)),
                                            Err(e) => Err(ClientError::JsonError(format!("Failed to parse final task: {}", e)))
                                        }
                                    } else {
                                        // This is a regular status update
                                        match serde_json::from_value::<Task>(result.clone()) {
                                            Ok(task) => Ok(StreamingResponse::Status(task)),
                                            Err(e) => Err(ClientError::JsonError(format!("Failed to parse task: {}", e)))
                                        }
                                    }
                                } 
                                // Default to a status update if no specific flags
                                else {
                                    match serde_json::from_value::<Task>(result.clone()) {
                                        Ok(task) => Ok(StreamingResponse::Status(task)),
                                        Err(e) => Err(ClientError::JsonError(format!("Failed to parse task: {}", e)))
                                    }
                                }
                            } else {
                                Err(ClientError::Other("Invalid JSON-RPC response: missing 'result' field".to_string()))
                            }
                        },
                        Err(e) => Err(ClientError::JsonError(format!("Failed to parse event data as JSON: {}", e))),
                    }
                },
                Err(e) => Err(e), // Propagate the ClientError from map_err
            }
        });

        // Return the boxed stream
        Ok(Box::pin(streaming_response) as StreamingResponseStream)
    }
    
    /// Creates a text message with the given content
    pub fn create_text_message(&self, text: &str) -> Message {
        use crate::types::{TextPart, Part, Role};
        
        let text_part = TextPart {
            type_: "text".to_string(),
            text: text.to_string(),
            metadata: None,
        };
        
        Message {
            role: Role::User,
            parts: vec![Part::TextPart(text_part)],
            metadata: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use tokio::test;
    use futures_util::StreamExt;
    use std::time::Duration;
    
    #[test]
    async fn test_send_task_subscribe() {
        // Create a realistic SSE stream response
        let sse_response = vec![
            "data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"id\":\"task-123\",\"status\":{\"state\":\"working\",\"timestamp\":\"2025-04-19T12:00:00Z\"},\"final\":false}}\n\n",
            "data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"artifact\":{\"parts\":[{\"type\":\"text\",\"text\":\"Streaming content 1\"}],\"index\":0,\"append\":false,\"lastChunk\":false}}}\n\n",
            "data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"artifact\":{\"parts\":[{\"type\":\"text\",\"text\":\"Streaming content 2\"}],\"index\":0,\"append\":true,\"lastChunk\":false}}}\n\n",
            "data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"id\":\"task-123\",\"status\":{\"state\":\"completed\",\"timestamp\":\"2025-04-19T12:01:00Z\"},\"final\":true}}\n\n",
        ].join("");
        
        // Setup mockito server
        let mut server = Server::new_async().await;
        
        // Create a mock for the streaming endpoint
        let mock = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "tasks/sendSubscribe"
            })))
            .with_body(sse_response)
            .create_async().await;
            
        // Create client and call streaming method using _typed version
        let mut client = A2aClient::new(&server.url());
        let mut stream = client.send_task_subscribe_typed("Test streaming request").await.unwrap();

        // Collect the results from the stream
        let mut responses = vec![];
        while let Some(response) = stream.next().await {
            responses.push(response);
            // Avoid waiting forever if something goes wrong
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        // Validate the results
        assert_eq!(responses.len(), 4, "Expected 4 streaming responses");
        
        // First should be a status
        match &responses[0] {
            Ok(StreamingResponse::Status(task)) => {
                assert_eq!(task.id, "task-123");
            },
            _ => panic!("Expected first response to be a Status"),
        }
        
        // Second and third should be artifacts
        match &responses[1] {
            Ok(StreamingResponse::Artifact(artifact)) => {
                assert_eq!(artifact.parts.len(), 1);
                if let Part::TextPart(part) = &artifact.parts[0] {
                    assert_eq!(part.text, "Streaming content 1");
                } else {
                    panic!("Expected TextPart");
                }
            },
            _ => panic!("Expected second response to be an Artifact"),
        }
        
        match &responses[2] {
            Ok(StreamingResponse::Artifact(artifact)) => {
                assert!(artifact.append.unwrap_or(false));
                assert!(!artifact.last_chunk.unwrap_or(false));
            },
            _ => panic!("Expected third response to be an Artifact"),
        }
        
        // Last should be a final response
        match &responses[3] {
            Ok(StreamingResponse::Final(task)) => {
                assert_eq!(task.id, "task-123");
                assert_eq!(task.status.state.to_string(), "completed");
            },
            _ => panic!("Expected fourth response to be a Final"),
        }
        
        mock.assert_async().await;
    }
    
    #[test]
    async fn test_resubscribe_task() {
        let task_id = "existing-task-456";
        let sse_response = vec![
            "data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"id\":\"existing-task-456\",\"status\":{\"state\":\"working\",\"timestamp\":\"2025-04-19T12:00:00Z\"},\"final\":false}}\n\n",
            "data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"artifact\":{\"parts\":[{\"type\":\"text\",\"text\":\"Continued streaming...\"}],\"index\":0,\"append\":false,\"lastChunk\":true}}}\n\n",
        ].join("");
        
        // Setup mockito server
        let mut server = Server::new_async().await;
        
        // Create a mock for the resubscribe endpoint
        let mock = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "text/event-stream")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "tasks/resubscribe",
                "params": {
                    "id": task_id
                }
            })))
            .with_body(sse_response)
            .create_async().await;
            
        // Create client and call resubscribe method using _typed version
        let mut client = A2aClient::new(&server.url());
        let mut stream = client.resubscribe_task_typed(task_id).await.unwrap();

        // Collect results
        let mut responses = vec![];
        while let Some(response) = stream.next().await {
            responses.push(response);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        
        // Validate results
        assert_eq!(responses.len(), 2);
        
        // First should be a status update
        match &responses[0] {
            Ok(StreamingResponse::Status(task)) => {
                assert_eq!(task.id, task_id);
            },
            _ => panic!("Expected first response to be a Status"),
        }
        
        // Second should be an artifact with lastChunk=true
        match &responses[1] {
            Ok(StreamingResponse::Artifact(artifact)) => {
                assert!(artifact.last_chunk.unwrap_or(false));
                if let Part::TextPart(part) = &artifact.parts[0] {
                    assert_eq!(part.text, "Continued streaming...");
                } else {
                    panic!("Expected TextPart");
                }
            },
            _ => panic!("Expected second response to be an Artifact"),
        }
        
        mock.assert_async().await;
    }
}
