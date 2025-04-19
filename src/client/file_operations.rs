use std::error::Error;
use std::path::Path;
use std::fs;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json::{Value, json, Map};
use chrono::{DateTime, Utc};

use crate::client::A2aClient;
use crate::types::{Message, Role, Part, FilePart, FileContent, TaskSendParams, Task};

/// Response from uploading a file
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileUploadResponse {
    pub file_id: String,
    pub uri: String,
    pub name: String,
    pub mime_type: String,
    pub size: usize,
    pub uploaded_at: DateTime<Utc>,
}

/// Response from downloading a file
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileDownloadResponse {
    pub file_id: String,
    pub name: String,
    pub mime_type: String,
    pub bytes: String,
    pub size: usize,
}

impl A2aClient {
    /// Uploads a file to the server using the files/upload endpoint
    pub async fn upload_file(
        &mut self,
        file_path: &str,
        metadata: Option<Map<String, Value>>
    ) -> Result<FileUploadResponse, Box<dyn Error>> {
        // Read the file and create file content
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
            
        // Base64 encode the file content
        let encoded_bytes = BASE64.encode(&file_bytes);
        
        // Create the file content params
        let file_content = json!({
            "name": file_name,
            "mimeType": mime_type,
            "bytes": encoded_bytes
        });
        
        // Create the request params
        let mut params = json!({
            "file": file_content
        });
        
        // Add metadata if provided
        if let Some(meta) = metadata {
            params["metadata"] = Value::Object(meta);
        }
        
        // Send the request
        self.send_jsonrpc::<FileUploadResponse>("files/upload", params).await
    }
    
    /// Uploads a file to the server using bytes rather than a file path
    pub async fn upload_file_bytes(
        &mut self,
        file_bytes: &[u8],
        file_name: &str,
        mime_type: Option<&str>,
        metadata: Option<Map<String, Value>>
    ) -> Result<FileUploadResponse, Box<dyn Error>> {
        // Base64 encode the file content
        let encoded_bytes = BASE64.encode(file_bytes);
        
        // Determine MIME type if not provided
        let mime = match mime_type {
            Some(mt) => mt.to_string(),
            None => {
                // Try to determine from filename
                match Path::new(file_name).extension() {
                    Some(ext) => {
                        mime_guess::from_ext(ext.to_str().unwrap_or(""))
                            .first_or_octet_stream()
                            .to_string()
                    },
                    None => "application/octet-stream".to_string()
                }
            }
        };
        
        // Create the file content params
        let file_content = json!({
            "name": file_name,
            "mimeType": mime,
            "bytes": encoded_bytes
        });
        
        // Create the request params
        let mut params = json!({
            "file": file_content
        });
        
        // Add metadata if provided
        if let Some(meta) = metadata {
            params["metadata"] = Value::Object(meta);
        }
        
        // Send the request
        self.send_jsonrpc::<FileUploadResponse>("files/upload", params).await
    }
    
    /// Downloads a file by its ID using the files/download endpoint
    pub async fn download_file(&mut self, file_id: &str) -> Result<FileDownloadResponse, Box<dyn Error>> {
        // Create request params
        let params = json!({
            "fileId": file_id
        });
        
        // Send the request
        self.send_jsonrpc::<FileDownloadResponse>("files/download", params).await
    }
    
    /// Lists files associated with a task using the files/list endpoint
    pub async fn list_files(
        &mut self, 
        task_id: Option<&str>
    ) -> Result<Vec<FileUploadResponse>, Box<dyn Error>> {
        // Create request params
        let mut params = json!({});
        
        if let Some(id) = task_id {
            params = json!({
                "taskId": id
            });
        }
        
        // Define a wrapper type for the response
        #[derive(serde::Deserialize)]
        struct ListFilesResponse {
            files: Vec<FileUploadResponse>
        }
        
        // Send the request
        let response = self.send_jsonrpc::<ListFilesResponse>("files/list", params).await?;
        Ok(response.files)
    }
    
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
    
    #[test]
    async fn test_upload_file_bytes() {
        // Arrange
        let file_bytes = b"Sample file content";
        let file_name = "test.txt";
        let mime_type = "text/plain";
        
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "file_id": "file-12345",
                "uri": "files/file-12345",
                "name": "test.txt",
                "mime_type": "text/plain",
                "size": 19,
                "uploaded_at": "2023-01-01T00:00:00Z"
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Mock server expects a request with file content
        let mock = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "files/upload",
                "params": {
                    "file": {
                        "name": "test.txt",
                        "mimeType": "text/plain"
                    }
                }
            })))
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url());
        let result = client.upload_file_bytes(file_bytes, file_name, Some(mime_type), None).await.unwrap();
        
        // Assert
        assert_eq!(result.file_id, "file-12345");
        assert_eq!(result.name, "test.txt");
        assert_eq!(result.mime_type, "text/plain");
        assert_eq!(result.size, 19);
        
        mock.assert_async().await;
    }
    
    #[test]
    async fn test_download_file() {
        // Arrange
        let file_id = "file-12345";
        
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "file_id": "file-12345",
                "name": "test.txt",
                "mime_type": "text/plain",
                "bytes": "U2FtcGxlIGZpbGUgY29udGVudA==", // "Sample file content" in base64
                "size": 19
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Mock server expects a request with file ID
        let mock = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "files/download",
                "params": {
                    "fileId": "file-12345"
                }
            })))
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url());
        let result = client.download_file(file_id).await.unwrap();
        
        // Assert
        assert_eq!(result.file_id, "file-12345");
        assert_eq!(result.name, "test.txt");
        assert_eq!(result.mime_type, "text/plain");
        
        // Decode the base64 content and verify
        let decoded = BASE64.decode(&result.bytes).unwrap();
        assert_eq!(String::from_utf8(decoded).unwrap(), "Sample file content");
        
        mock.assert_async().await;
    }
    
    #[test]
    async fn test_list_files() {
        // Arrange
        let task_id = "task-12345";
        
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "files": [
                    {
                        "file_id": "file-1",
                        "uri": "files/file-1",
                        "name": "test1.txt",
                        "mime_type": "text/plain",
                        "size": 19,
                        "uploaded_at": "2023-01-01T00:00:00Z"
                    },
                    {
                        "file_id": "file-2",
                        "uri": "files/file-2",
                        "name": "test2.jpg",
                        "mime_type": "image/jpeg",
                        "size": 1024,
                        "uploaded_at": "2023-01-02T00:00:00Z"
                    }
                ]
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Mock server expects a request for file listing
        let mock = server.mock("POST", "/")
            .with_status(200)
            .with_header("content-type", "application/json")
            .match_body(mockito::Matcher::PartialJson(json!({
                "jsonrpc": "2.0",
                "method": "files/list",
                "params": {
                    "taskId": "task-12345"
                }
            })))
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url());
        let results = client.list_files(Some(task_id)).await.unwrap();
        
        // Assert
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].file_id, "file-1");
        assert_eq!(results[0].name, "test1.txt");
        assert_eq!(results[1].file_id, "file-2");
        assert_eq!(results[1].name, "test2.jpg");
        
        mock.assert_async().await;
    }
}