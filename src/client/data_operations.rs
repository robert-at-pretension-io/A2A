use std::error::Error;
use serde_json::{json, Value, Map};
use serde::Serialize;

use crate::client::A2aClient;
use crate::client::errors::ClientError;
// Remove ErrorCompatibility import
// use crate::client::error_handling::ErrorCompatibility;
use crate::types::{Message, Role, Part, DataPart, TaskSendParams, Task};

impl A2aClient {
    /// Sends a task with structured data
    pub async fn send_task_with_data_typed<T: Serialize>(&mut self, text: &str, data: &T) -> Result<Task, ClientError> {
        // Create a message with structured data
        let message = self.create_text_and_data_message_typed(text, data)?;
        
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
        let params_value = serde_json::to_value(params)
            .map_err(|e| ClientError::JsonError(format!("Failed to serialize params: {}", e)))?;
            
        self.send_jsonrpc::<Task>("tasks/send", params_value).await
    }

    // Remove old version if not needed
    // pub async fn send_task_with_data<T: Serialize>(&mut self, text: &str, data: &T) -> Result<Task, Box<dyn Error>> {
    //     self.send_task_with_data_typed(text, data).await.into_box_error()
    // }

    /// Creates a message with both text and structured data
    pub fn create_text_and_data_message_typed<T: Serialize>(&self, text: &str, data: &T) -> Result<Message, ClientError> {
        // Create text part
        let text_part = crate::types::TextPart {
            type_: "text".to_string(),
            text: text.to_string(),
            metadata: None,
        };
        
        // Convert data to JSON Map
        let data_value = serde_json::to_value(data)
            .map_err(|e| ClientError::JsonError(format!("Failed to serialize data: {}", e)))?;
            
        let data_map = match data_value {
            Value::Object(map) => map,
            _ => return Err(ClientError::Other("Data must be a JSON object".to_string())),
        };
        
        // Create data part
        let data_part = DataPart {
            type_: "data".to_string(),
            data: data_map,
            metadata: None,
        };
        
        // Create message with both parts
        Ok(Message {
            role: Role::User,
            parts: vec![Part::TextPart(text_part), Part::DataPart(data_part)],
            metadata: None,
        })
    }

    // Remove old version if not needed
    // pub fn create_text_and_data_message<T: Serialize>(&self, text: &str, data: &T) -> Result<Message, Box<dyn Error>> {
    //     self.create_text_and_data_message_typed(text, data).into_box_error()
    // }

    /// Creates a message with just structured data
    pub fn create_data_only_message_typed<T: Serialize>(&self, data: &T) -> Result<Message, ClientError> {
        // Convert data to JSON Map
        let data_value = serde_json::to_value(data)
            .map_err(|e| ClientError::JsonError(format!("Failed to serialize data: {}", e)))?;
            
        let data_map = match data_value {
            Value::Object(map) => map,
            _ => return Err(ClientError::Other("Data must be a JSON object".to_string())),
        };
        
        // Create data part
        let data_part = DataPart {
            type_: "data".to_string(),
            data: data_map,
            metadata: None,
        };
        
        // Create message with just data part
        Ok(Message {
            role: Role::User,
            parts: vec![Part::DataPart(data_part)],
            metadata: None,
        })
    }

    // Remove old version if not needed
    // pub fn create_data_only_message<T: Serialize>(&self, data: &T) -> Result<Message, Box<dyn Error>> {
    //     self.create_data_only_message_typed(data).into_box_error()
    // }

    /// Extract data parts from a message
    pub fn extract_data_parts(message: &Message) -> Vec<&Map<String, Value>> {
        message.parts.iter()
            .filter_map(|part| {
                if let Part::DataPart(data_part) = part {
                    Some(&data_part.data)
                } else {
                    None
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use tokio::test;
    use serde_json::json;
    
    #[tokio::test]
    async fn test_create_data_message() {
        // Create sample structured data
        let data = json!({
            "name": "Test User",
            "age": 30,
            "preferences": {
                "theme": "dark",
                "notifications": true
            }
        });
        
        let client = A2aClient::new("http://example.com");
        let message = client.create_text_and_data_message(
            "Here's my user data", &data
        ).unwrap();
        
        // Validate message structure
        assert_eq!(message.role, Role::User);
        assert_eq!(message.parts.len(), 2);
        
        // Check text part
        if let Part::TextPart(text_part) = &message.parts[0] {
            assert_eq!(text_part.text, "Here's my user data");
        } else {
            panic!("Expected TextPart");
        }
        
        // Check data part
        if let Part::DataPart(data_part) = &message.parts[1] {
            assert_eq!(data_part.type_, "data");
            assert_eq!(data_part.data.get("name").unwrap().as_str().unwrap(), "Test User");
            assert_eq!(data_part.data.get("age").unwrap().as_i64().unwrap(), 30);
            assert!(data_part.data.get("preferences").unwrap().is_object());
        } else {
            panic!("Expected DataPart");
        }
    }
    
    #[tokio::test]
    async fn test_send_task_with_data() {
        // Arrange
        let text = "Here's structured data";
        let data = json!({
            "coordinates": {
                "latitude": 37.7749,
                "longitude": -122.4194
            },
            "timestamp": "2023-01-01T00:00:00Z"
        });
        
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "id": "task-with-data-123",
                "sessionId": "session-456",
                "status": {
                    "state": "submitted",
                    "timestamp": "2023-01-01T00:00:00Z"
                }
            }
        });
        
        let mut server = Server::new_async().await;
        
        // Mock server expects a request with both text and data parts
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
                            { "type": "data" }
                        ]
                    }
                }
            })))
            .with_body(response.to_string())
            .create_async().await;
        
        // Act
        let mut client = A2aClient::new(&server.url());
        // Call the _typed version
        let result = client.send_task_with_data_typed(text, &data).await.unwrap();

        // Assert
        assert_eq!(result.id, "task-with-data-123");
        
        mock.assert_async().await;
    }
    
    #[tokio::test]
    async fn test_extract_data_parts() {
        // Create a message with data parts
        let client = A2aClient::new("http://example.com");
        
        // Create data
        let data1 = json!({
            "type": "location",
            "coordinates": [37.7749, -122.4194]
        });
        
        // Create message with text and data
        let message = client.create_text_and_data_message("Here's data", &data1).unwrap();
        
        // Extract data parts
        let data_parts = A2aClient::extract_data_parts(&message);
        
        // Verify data was extracted correctly
        assert_eq!(data_parts.len(), 1);
        let extracted_data = data_parts[0];
        assert_eq!(extracted_data.get("type").unwrap().as_str().unwrap(), "location");
    }
}
