use crate::bidirectional::BidirectionalAgent;
use crate::bidirectional::config::{BidirectionalAgentConfig, ServerConfig, ClientConfig, LlmConfig, ModeConfig};
use crate::bidirectional::tests::mocks::MockLlmClient;
use crate::server::repositories::task_repository::{TaskRepository, InMemoryTaskRepository};
use crate::server::services::task_service::TaskService;
use crate::types::{Task, TaskStatus, TaskState, Message, Part, TextPart, Role, TaskSendParams, Artifact, DataPart, FilePart, FileContent};
use base64;
use std::sync::Arc;
use std::collections::HashMap;
use std::path::Path;
use serde_json::{json, Map, Value};
use std::env;
use uuid::Uuid;
use chrono::Utc;
use anyhow::{Result, anyhow};

// This struct helps us test artifact support
struct TestAgentWithArtifacts {
    task_service: Arc<TaskService>,
    task_repository: Arc<InMemoryTaskRepository>,
    current_session_id: Option<String>,
    session_tasks: HashMap<String, Vec<String>>,
}

impl TestAgentWithArtifacts {
    fn new() -> Self {
        let task_repository = Arc::new(InMemoryTaskRepository::new());
        let task_service = Arc::new(TaskService::standalone(task_repository.clone()));
        
        Self {
            task_service,
            task_repository,
            current_session_id: None,
            session_tasks: HashMap::new(),
        }
    }
    
    fn create_new_session(&mut self) -> String {
        let session_id = format!("session-{}", Uuid::new_v4());
        self.current_session_id = Some(session_id.clone());
        self.session_tasks.insert(session_id.clone(), Vec::new());
        session_id
    }
    
    async fn save_task_to_history(&mut self, task: Task) -> Result<()> {
        if let Some(session_id) = &self.current_session_id {
            if let Some(tasks) = self.session_tasks.get_mut(session_id) {
                tasks.push(task.id.clone());
            }
        }
        Ok(())
    }
    
    // Process a message with a file attachment
    async fn process_message_with_file(&mut self, message_text: &str, file_path: &str, file_content: Vec<u8>) -> Result<String> {
        // Create a unique task ID
        let task_id = Uuid::new_v4().to_string();
        
        // Get filename from path
        let file_name = Path::new(file_path)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("file.bin")
            .to_string();
        
        // Create FilePart
        let file_part = FilePart {
            type_: "file".to_string(),
            file: FileContent {
                name: Some(file_name),
                mime_type: None,
                bytes: Some(base64::encode(&file_content)),
                uri: None,
            },
            metadata: None,
        };
        
        // Create message with file part and text part
        let initial_message = Message {
            role: Role::User,
            parts: vec![
                Part::TextPart(TextPart {
                    text: message_text.to_string(),
                    metadata: None,
                    type_: "text".to_string(),
                }),
                Part::FilePart(file_part),
            ],
            metadata: None,
        };
        
        // Create TaskSendParams
        let params = TaskSendParams {
            id: task_id.clone(),
            message: initial_message,
            session_id: self.current_session_id.clone(),
            metadata: None,
            history_length: None,
            push_notification: None,
        };
        
        // Use task_service to process the task
        let task = self.task_service.process_task(params).await?;
        
        // Save task to history
        self.save_task_to_history(task.clone()).await?;
        
        // Extract response text
        let response = self.extract_text_from_task(&task);
        
        // If we have artifacts, indicate them in the response
        if let Some(artifacts) = &task.artifacts {
            if !artifacts.is_empty() {
                let artifact_summary = format!("\n\n[This task has {} artifact(s). Use :artifacts {} to view them]", 
                                            artifacts.len(), task.id);
                return Ok(response + &artifact_summary);
            }
        }
        
        Ok(response)
    }
    
    // Process a message with JSON data
    async fn process_message_with_data(&mut self, message_text: &str, json_data: Map<String, Value>) -> Result<String> {
        // Create a unique task ID
        let task_id = Uuid::new_v4().to_string();
        
        // Create DataPart
        let data_part = DataPart {
            type_: "json".to_string(),
            data: json_data,
            metadata: None,
        };
        
        // Create message with data part and text part
        let initial_message = Message {
            role: Role::User,
            parts: vec![
                Part::TextPart(TextPart {
                    text: message_text.to_string(),
                    metadata: None,
                    type_: "text".to_string(),
                }),
                Part::DataPart(data_part),
            ],
            metadata: None,
        };
        
        // Create TaskSendParams
        let params = TaskSendParams {
            id: task_id.clone(),
            message: initial_message,
            session_id: self.current_session_id.clone(),
            metadata: None,
            history_length: None,
            push_notification: None,
        };
        
        // Use task_service to process the task
        let task = self.task_service.process_task(params).await?;
        
        // Save task to history
        self.save_task_to_history(task.clone()).await?;
        
        // Extract response text
        let response = self.extract_text_from_task(&task);
        
        // If we have artifacts, indicate them in the response
        if let Some(artifacts) = &task.artifacts {
            if !artifacts.is_empty() {
                let artifact_summary = format!("\n\n[This task has {} artifact(s). Use :artifacts {} to view them]", 
                                             artifacts.len(), task.id);
                return Ok(response + &artifact_summary);
            }
        }
        
        Ok(response)
    }
    
    // Extract text from a task
    fn extract_text_from_task(&self, task: &Task) -> String {
        // First check the status message
        if let Some(ref message) = task.status.message {
            let text = message.parts.iter()
                .filter_map(|p| match p {
                    Part::TextPart(tp) => Some(tp.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            
            if !text.is_empty() {
                return text;
            }
        }
        
        // Then check history if available
        if let Some(history) = &task.history {
            let agent_messages = history.iter()
                .filter(|m| m.role == Role::Agent)
                .flat_map(|m| m.parts.iter())
                .filter_map(|p| match p {
                    Part::TextPart(tp) => Some(tp.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            
            if !agent_messages.is_empty() {
                return agent_messages;
            }
        }
        
        // Fallback
        "No response text available.".to_string()
    }
    
    // Format artifact details as would be shown in REPL
    fn format_artifact_details(&self, task_id: &str, artifacts: &[Artifact]) -> String {
        if artifacts.is_empty() {
            return format!("📦 No artifacts for task {}", task_id);
        }
        
        let mut result = format!("\n📦 Artifacts for Task {}:", task_id);
        
        for (i, artifact) in artifacts.iter().enumerate() {
            result.push_str(&format!("\n  {}. {} ({})", i + 1, 
                            artifact.name.clone().unwrap_or_else(|| format!("Artifact {}", artifact.index)),
                            artifact.description.clone().unwrap_or_else(|| "No description".to_string())));
            
            // Display artifact content based on type
            for part in &artifact.parts {
                match part {
                    Part::TextPart(tp) => {
                        result.push_str(&format!("\n     Text: {}", tp.text));
                    },
                    Part::DataPart(_) => {
                        result.push_str("\n     Data: [JSON data]");
                    },
                    Part::FilePart(fp) => {
                        result.push_str(&format!("\n     File: {} ({} bytes)", 
                                      fp.file.name.clone().unwrap_or_else(|| "unnamed".to_string()),
                                      fp.file.bytes.as_ref().map(|b| b.len()).unwrap_or(0) / 4 * 3));
                    },
                    _ => result.push_str("\n     Unknown part type"),
                }
            }
        }
        
        result
    }
}

#[tokio::test]
async fn test_process_message_with_file() {
    // Create test agent
    let mut agent = TestAgentWithArtifacts::new();
    
    // Create a session
    let session_id = agent.create_new_session();
    
    // Process a message with a file
    let message = "Here's a file";
    let file_path = "/path/to/test.txt";
    let file_content = b"This is test file content".to_vec();
    
    let response = agent.process_message_with_file(message, file_path, file_content).await.unwrap();
    
    // The response should indicate the task completed successfully
    assert!(response.contains("Task completed successfully"));
    
    // The response should also indicate that the task has artifacts
    assert!(response.contains("[This task has"));
    assert!(response.contains("artifact(s)"));
}

#[tokio::test]
async fn test_process_message_with_data() {
    // Create test agent
    let mut agent = TestAgentWithArtifacts::new();
    
    // Create a session
    let session_id = agent.create_new_session();
    
    // Create some JSON data
    let mut data_map = Map::new();
    data_map.insert("name".to_string(), json!("test"));
    data_map.insert("value".to_string(), json!(42));
    
    // Process a message with data
    let message = "Here's some data";
    let response = agent.process_message_with_data(message, data_map).await.unwrap();
    
    // The response should indicate the task completed successfully
    assert!(response.contains("Task completed successfully"));
    
    // The response should also indicate that the task has artifacts
    assert!(response.contains("[This task has"));
    assert!(response.contains("artifact(s)"));
}

#[tokio::test]
async fn test_format_artifact_details() {
    // Create test agent
    let mut agent = TestAgentWithArtifacts::new();
    
    // Create a task ID
    let task_id = Uuid::new_v4().to_string();
    
    // Create some test artifacts
    let text_part = TextPart {
        type_: "text".to_string(),
        text: "This is test text content".to_string(),
        metadata: None,
    };
    
    let mut data_map = Map::new();
    data_map.insert("name".to_string(), json!("test"));
    data_map.insert("value".to_string(), json!(42));
    
    let data_part = DataPart {
        type_: "json".to_string(),
        data: data_map,
        metadata: None,
    };
    
    let file_part = FilePart {
        type_: "file".to_string(),
        file: FileContent {
            name: Some("test.txt".to_string()),
            mime_type: None,
            bytes: Some(base64::encode(b"This is test file content")),
            uri: None,
        },
        metadata: None,
    };
    
    let artifacts = vec![
        Artifact {
            index: 0,
            name: Some("Text Artifact".to_string()),
            parts: vec![Part::TextPart(text_part)],
            description: Some("A text artifact".to_string()),
            append: None,
            last_chunk: None,
            metadata: None,
        },
        Artifact {
            index: 1,
            name: Some("Data Artifact".to_string()),
            parts: vec![Part::DataPart(data_part)],
            description: Some("A data artifact".to_string()),
            append: None,
            last_chunk: None,
            metadata: None,
        },
        Artifact {
            index: 2,
            name: Some("File Artifact".to_string()),
            parts: vec![Part::FilePart(file_part)],
            description: Some("A file artifact".to_string()),
            append: None,
            last_chunk: None,
            metadata: None,
        },
    ];
    
    // Format the artifact details
    let formatted = agent.format_artifact_details(&task_id, &artifacts);
    
    // Verify the formatted output
    assert!(formatted.contains(&format!("Artifacts for Task {}:", task_id)));
    assert!(formatted.contains("Text Artifact"));
    assert!(formatted.contains("Data Artifact"));
    assert!(formatted.contains("File Artifact"));
    assert!(formatted.contains("This is test text content"));
    assert!(formatted.contains("[JSON data]"));
    // Check for file name without checking exact byte size
    assert!(formatted.contains("test.txt") && formatted.contains("bytes"));
}