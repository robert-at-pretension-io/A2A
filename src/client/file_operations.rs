use std::error::Error;
use std::path::Path;
use std::fs;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json::Value;

use crate::client::A2aClient;
use crate::types::{Message, Role, Part, FilePart, FileContent, TaskSendParams, Task};

impl A2aClient {
    /// Sends a task with a file attachment by path
    pub async fn send_task_with_file(&mut self, text: &str, file_path: &str) -> Result<Task, Box<dyn Error>> {
        // Read the file and create a file message
        let message = self.create_text_and_file_message(text, file_path).await?;
        
        // Create request parameters
        let params = TaskSendParams {
            id: uuid::Uuid::new_v4().to_string(),
            message,
            history_length: None,
            metadata: None,
            push_notification: None,
            session_id: None,
        };
        
        // Send request and return result
        self.send_jsonrpc::<Task>("tasks/send", serde_json::to_value(params)?).await
    }
    
    /// Sends a task with a file attachment by bytes
    pub async fn send_task_with_file_bytes(
        &mut self, 
        text: &str, 
        file_bytes: &[u8], 
        file_name: &str,
        mime_type: Option<&str>
    ) -> Result<Task, Box<dyn Error>> {
        // Create a message with text and file parts
        let message = self.create_text_and_file_bytes_message(
            text, file_bytes, file_name, mime_type
        );
        
        // Create request parameters
        let params = TaskSendParams {
            id: uuid::Uuid::new_v4().to_string(),
            message,
            history_length: None,
            metadata: None,
            push_notification: None,
            session_id: None,
        };
        
        // Send request and return result
        self.send_jsonrpc::<Task>("tasks/send", serde_json::to_value(params)?).await
    }
    
    /// Creates a message with both text and file by path
    pub async fn create_text_and_file_message(&self, text: &str, file_path: &str) -> Result<Message, Box<dyn Error>> {
        let path = Path::new(file_path);
        
        // Check if the file exists
        if !path.exists() {
            return Err(format!("File not found: {}", file_path).into());
        }
        
        // Read the file contents
        let file_bytes = fs::read(path)?;
        
        // Get file name and mime type
        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file")
            .to_string();
            
        // Try to determine mime type from file extension
        let mime_type = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();
            
        Ok(self.create_text_and_file_bytes_message(text, &file_bytes, &file_name, Some(&mime_type)))
    }
    
    /// Creates a message with both text and file bytes
    pub fn create_text_and_file_bytes_message(
        &self, 
        text: &str, 
        file_bytes: &[u8], 
        file_name: &str,
        mime_type: Option<&str>
    ) -> Message {
        use crate::types::{TextPart, Part, Role};
        
        // Create text part
        let text_part = TextPart {
            type_: "text".to_string(),
            text: text.to_string(),
            metadata: None,
        };
        
        // Create file part
        let encoded_bytes = BASE64.encode(file_bytes);
        
        let file_content = FileContent {
            bytes: Some(encoded_bytes),
            uri: None,
            mime_type: mime_type.map(|s| s.to_string()),
            name: Some(file_name.to_string()),
        };
        
        let file_part = FilePart {
            type_: "file".to_string(),
            file: file_content,
            metadata: None,
        };
        
        // Create message with both parts
        Message {
            role: Role::User,
            parts: vec![Part::TextPart(text_part), Part::FilePart(file_part)],
            metadata: None,
        }
    }
    
    /// Creates a message with a file using a URI reference
    pub fn create_file_uri_message(&self, text: &str, file_uri: &str, mime_type: Option<&str>, file_name: Option<&str>) -> Message {
        // Create text part
        let text_part = crate::types::TextPart {
            type_: "text".to_string(),
            text: text.to_string(),
            metadata: None,
        };
        
        // Create file part with URI
        let file_content = FileContent {
            bytes: None,
            uri: Some(file_uri.to_string()),
            mime_type: mime_type.map(|s| s.to_string()),
            name: file_name.map(|s| s.to_string()),
        };
        
        let file_part = FilePart {
            type_: "file".to_string(),
            file: file_content,
            metadata: None,
        };
        
        // Create message with both parts
        Message {
            role: Role::User,
            parts: vec![Part::TextPart(text_part), Part::FilePart(file_part)],
            metadata: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use tokio::test;
    use serde_json::json;
    
    #[test]
    async fn test_create_file_message() {
        // Create a sample file content for testing
        let file_bytes = b"This is test file content";
        let file_name = "test.txt";
        let mime_type = "text/plain";
        
        let client = A2aClient::new("http://example.com");
        let message = client.create_text_and_file_bytes_message(
            "Here's my file", file_bytes, file_name, Some(mime_type)
        );
        
        // Validate the message structure
        assert_eq!(message.role, Role::User);
        assert_eq!(message.parts.len(), 2);
        
        // Check text part
        if let Part::TextPart(text_part) = &message.parts[0] {
            assert_eq!(text_part.text, "Here's my file");
        } else {
            panic!("Expected TextPart");
        }
        
        // Check file part
        if let Part::FilePart(file_part) = &message.parts[1] {
            assert_eq!(file_part.type_, "file");
            assert_eq!(file_part.file.name.as_ref().unwrap(), "test.txt");
            assert_eq!(file_part.file.mime_type.as_ref().unwrap(), "text/plain");
            assert!(file_part.file.bytes.is_some());
        } else {
            panic!("Expected FilePart");
        }
    }
    
    #[test]
    async fn test_send_task_with_file_bytes() {
        // Arrange
        let text = "Here's a file attachment";
        let file_bytes = b"Sample file content";
        let file_name = "test.txt";
        let mime_type = "text/plain";
        
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "id": "task-with-file-123",
                "sessionId": "mock-session-456",
                "status": {
                    "state": "submitted",
                    "timestamp": "2023-01-01T00:00:00Z"
                }
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Mock server expects a request with both text and file parts
        let mock = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "tasks/send",
                "params": {
                    "message": {
                        "role": "user",
                        "parts": [
                            { "type": "text" },
                            { "type": "file" }
                        ]
                    }
                }
            })))
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url());
        let result = client.send_task_with_file_bytes(text, file_bytes, file_name, Some(mime_type)).await.unwrap();
        
        // Assert
        assert_eq!(result.id, "task-with-file-123");
        
        mock.assert_async().await;
    }
    
    #[test]
    async fn test_create_file_uri_message() {
        // Use a URI to reference a file instead of embedding it
        let file_uri = "https://example.com/files/document.pdf";
        let mime_type = "application/pdf";
        let file_name = "document.pdf";
        
        let client = A2aClient::new("http://example.com");
        let message = client.create_file_uri_message(
            "Here's my document", file_uri, Some(mime_type), Some(file_name)
        );
        
        // Validate message
        assert_eq!(message.parts.len(), 2);
        
        // Check file part
        if let Part::FilePart(file_part) = &message.parts[1] {
            assert_eq!(file_part.file.uri.as_ref().unwrap(), file_uri);
            assert!(file_part.file.bytes.is_none());
            assert_eq!(file_part.file.mime_type.as_ref().unwrap(), mime_type);
        } else {
            panic!("Expected FilePart");
        }
    }
}