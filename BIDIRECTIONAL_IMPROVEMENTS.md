# Bidirectional Agent TaskService Improvements

This document outlines a comprehensive plan for better utilizing the TaskService in the bidirectional agent implementation, focusing on improving state management, error handling, and adding more A2A protocol features.

## Current Limitations

After analyzing the current bidirectional agent implementation and the TaskService, the following limitations were identified:

1. **Direct LLM Calls Bypass TaskService**: The `process_message_directly` method creates a Task object but then bypasses the TaskService, making direct LLM API calls instead of leveraging the TaskService's task processing capabilities.

2. **Limited Session Management**: While the agent creates task IDs and can track sessions, it doesn't fully utilize the session tracking capabilities of the A2A protocol.

3. **No Task History in REPL**: The REPL interface doesn't maintain or display task history, limiting visibility into past interactions.

4. **Minimal Input Required Handling**: The agent doesn't properly handle the A2A protocol's `InputRequired` state for interactive tasks.

5. **Limited Artifact Support**: The agent doesn't expose file or data artifacts in the REPL interface.

6. **No Direct TaskService Command Access**: The REPL doesn't provide commands to access useful TaskService methods like `get_task`, `cancel_task`, or `get_task_state_history`.

## Improvement Plan

### 1. Use TaskService for Message Processing

Replace direct LLM calls with TaskService integration in the `process_message_directly` method:

```rust
pub async fn process_message_directly(&self, message_text: &str) -> Result<String> {
    // Create a unique task ID
    let task_id = Uuid::new_v4().to_string();
    
    // Create the message
    let initial_message = Message {
        role: Role::User,
        parts: vec![Part::TextPart(TextPart {
            text: message_text.to_string(),
            metadata: None,
            type_: "text".to_string(),
        })],
        metadata: None,
    };
    
    // Create TaskSendParams instead of creating a Task directly
    let params = TaskSendParams {
        id: task_id.clone(),
        message: initial_message,
        session_id: self.current_session_id.clone(), // Add session handling
        metadata: None,
    };
    
    // Use task_service to process the task
    let task = self.task_service.process_task(params).await
        .map_err(|e| anyhow!("Failed to process task: {}", e))?;
    
    // Extract response from task
    let response = extract_text_from_task(&task);
    
    // Save task to history (implement this method)
    self.save_task_to_history(task.clone()).await?;
    
    Ok(response)
}

// Helper to extract text from task
fn extract_text_from_task(task: &Task) -> String {
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
```

### 2. Add Session Management for Conversational Context

Add session management to maintain conversation context across multiple messages:

```rust
// Add to BidirectionalAgent struct
pub struct BidirectionalAgent {
    // ... existing fields
    current_session_id: Option<String>,
    session_tasks: Arc<DashMap<String, Vec<String>>>, // Map session ID to task IDs
}

// Update initialization
impl BidirectionalAgent {
    pub fn new(config: BidirectionalAgentConfig) -> Result<Self> {
        // ... existing initialization
        
        Ok(Self {
            // ... existing fields
            current_session_id: None,
            session_tasks: Arc::new(DashMap::new()),
        })
    }
    
    // Create a new session
    pub fn create_new_session(&mut self) -> String {
        let session_id = format!("session-{}", Uuid::new_v4());
        self.current_session_id = Some(session_id.clone());
        self.session_tasks.insert(session_id.clone(), Vec::new());
        session_id
    }
    
    // Add task to current session
    async fn save_task_to_history(&self, task: Task) -> Result<()> {
        if let Some(session_id) = &self.current_session_id {
            if let Some(mut tasks) = self.session_tasks.get_mut(session_id) {
                tasks.push(task.id.clone());
            }
        }
        Ok(())
    }
    
    // Get tasks for current session
    pub async fn get_current_session_tasks(&self) -> Result<Vec<Task>> {
        let mut tasks = Vec::new();
        if let Some(session_id) = &self.current_session_id {
            if let Some(task_ids) = self.session_tasks.get(session_id) {
                for task_id in task_ids.iter() {
                    // Use TaskQueryParams to get task with history
                    let params = TaskQueryParams {
                        id: task_id.clone(),
                        history_length: None, // Get full history
                    };
                    
                    if let Ok(task) = self.task_service.get_task(params).await {
                        tasks.push(task);
                    }
                }
            }
        }
        Ok(tasks)
    }
}
```

### 3. Add REPL Commands for Task Operations

Add new REPL commands to interact with the TaskService:

```rust
// Add to run_repl method
pub async fn run_repl(&mut self) -> Result<()> {
    // ... existing initialization
    
    // Add new commands to help message
    println!("  :session new     - Create a new conversation session");
    println!("  :session show    - Show the current session ID");
    println!("  :history         - Show message history for current session");
    println!("  :tasks           - List all tasks in the current session");
    println!("  :task ID         - Show details for a specific task");
    println!("  :artifacts ID    - Show artifacts for a specific task");
    println!("  :cancelTask ID   - Cancel a running task");
    
    // ... existing loop
    
    // Handle new commands
    if input == ":session new" {
        let session_id = self.create_new_session();
        println!("✅ Created new session: {}", session_id);
    } else if input == ":session show" {
        if let Some(session_id) = &self.current_session_id {
            println!("🔍 Current session: {}", session_id);
        } else {
            println!("⚠️ No active session. Use :session new to create one.");
        }
    } else if input == ":history" {
        if let Some(session_id) = &self.current_session_id {
            let tasks = self.get_current_session_tasks().await?;
            if tasks.is_empty() {
                println!("📭 No messages in current session.");
            } else {
                println!("\n📝 Session History:");
                for (i, task) in tasks.iter().enumerate() {
                    if let Some(history) = &task.history {
                        for message in history {
                            let role_icon = match message.role {
                                Role::User => "👤",
                                Role::Agent => "🤖",
                                _ => "➡️",
                            };
                            
                            // Extract text from parts
                            let text = message.parts.iter()
                                .filter_map(|p| match p {
                                    Part::TextPart(tp) => Some(tp.text.clone()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            
                            // Truncate long messages for display
                            let display_text = if text.len() > 100 {
                                format!("{}...", &text[..97])
                            } else {
                                text
                            };
                            
                            println!("{} {}: {}", role_icon, message.role, display_text);
                        }
                    }
                }
            }
        } else {
            println!("⚠️ No active session. Use :session new to create one.");
        }
    } else if input == ":tasks" {
        if let Some(session_id) = &self.current_session_id {
            let tasks = self.get_current_session_tasks().await?;
            if tasks.is_empty() {
                println!("📭 No tasks in current session.");
            } else {
                println!("\n📋 Tasks in Current Session:");
                for (i, task) in tasks.iter().enumerate() {
                    println!("  {}. {} - Status: {:?}", i + 1, task.id, task.status.state);
                }
            }
        } else {
            println!("⚠️ No active session. Use :session new to create one.");
        }
    } else if input.starts_with(":task ") {
        let task_id = input.trim_start_matches(":task ").trim();
        if task_id.is_empty() {
            println!("❌ Error: No task ID provided. Use :task TASK_ID");
        } else {
            // Get task details
            let params = TaskQueryParams {
                id: task_id.to_string(),
                history_length: None,
            };
            
            match self.task_service.get_task(params).await {
                Ok(task) => {
                    println!("\n🔍 Task Details:");
                    println!("  ID: {}", task.id);
                    println!("  Status: {:?}", task.status.state);
                    println!("  Session: {}", task.session_id.unwrap_or_else(|| "None".to_string()));
                    println!("  Timestamp: {}", task.status.timestamp.map(|t| t.to_rfc3339()).unwrap_or_else(|| "None".to_string()));
                    
                    // Show artifacts count if any
                    if let Some(artifacts) = &task.artifacts {
                        println!("  Artifacts: {} (use :artifacts {} to view)", artifacts.len(), task.id);
                    } else {
                        println!("  Artifacts: None");
                    }
                    
                    // Show last message if any
                    if let Some(message) = &task.status.message {
                        let text = message.parts.iter()
                            .filter_map(|p| match p {
                                Part::TextPart(tp) => Some(tp.text.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        
                        println!("\n  Last Message: {}", text);
                    }
                },
                Err(e) => {
                    println!("❌ Error: Failed to get task: {}", e);
                }
            }
        }
    } else if input.starts_with(":artifacts ") {
        let task_id = input.trim_start_matches(":artifacts ").trim();
        if task_id.is_empty() {
            println!("❌ Error: No task ID provided. Use :artifacts TASK_ID");
        } else {
            // Get task details with artifacts
            let params = TaskQueryParams {
                id: task_id.to_string(),
                history_length: None,
            };
            
            match self.task_service.get_task(params).await {
                Ok(task) => {
                    if let Some(artifacts) = &task.artifacts {
                        if artifacts.is_empty() {
                            println!("📦 No artifacts for task {}", task.id);
                        } else {
                            println!("\n📦 Artifacts for Task {}:", task.id);
                            for (i, artifact) in artifacts.iter().enumerate() {
                                println!("  {}. {} ({})", i + 1, 
                                         artifact.name.clone().unwrap_or_else(|| format!("Artifact {}", artifact.index)),
                                         artifact.description.clone().unwrap_or_else(|| "No description".to_string()));
                                
                                // Display artifact content based on type
                                for part in &artifact.parts {
                                    match part {
                                        Part::TextPart(tp) => {
                                            println!("     Text: {}", tp.text);
                                        },
                                        Part::DataPart(_) => {
                                            println!("     Data: [JSON data]");
                                        },
                                        Part::FilePart(fp) => {
                                            println!("     File: {} ({} bytes)", 
                                                     fp.file_name.clone().unwrap_or_else(|| "unnamed".to_string()),
                                                     fp.size.unwrap_or(0));
                                        },
                                        _ => println!("     Unknown part type"),
                                    }
                                }
                            }
                        }
                    } else {
                        println!("📦 No artifacts for task {}", task.id);
                    }
                },
                Err(e) => {
                    println!("❌ Error: Failed to get task: {}", e);
                }
            }
        }
    } else if input.starts_with(":cancelTask ") {
        let task_id = input.trim_start_matches(":cancelTask ").trim();
        if task_id.is_empty() {
            println!("❌ Error: No task ID provided. Use :cancelTask TASK_ID");
        } else {
            // Cancel the task
            let params = TaskIdParams {
                id: task_id.to_string(),
            };
            
            match self.task_service.cancel_task(params).await {
                Ok(task) => {
                    println!("✅ Successfully canceled task {}", task.id);
                    println!("  Current state: {:?}", task.status.state);
                },
                Err(e) => {
                    println!("❌ Error: Failed to cancel task: {}", e);
                }
            }
        }
    }
    
    // ... remainder of run_repl method
}
```

### 4. Proper Input Required Handling

Improve the handling of tasks that require additional input:

```rust
// Update process_message_directly to handle InputRequired
pub async fn process_message_directly(&self, message_text: &str) -> Result<String> {
    // ... existing code to prepare message and params
    
    // Check if we're continuing an existing task that is in InputRequired state
    let mut continue_task_id = None;
    if let Some(session_id) = &self.current_session_id {
        if let Some(task_ids) = self.session_tasks.get(session_id) {
            // Check the last task in the session
            if let Some(last_task_id) = task_ids.iter().last() {
                // Get the task to check its state
                let params = TaskQueryParams {
                    id: last_task_id.clone(),
                    history_length: None,
                };
                
                if let Ok(task) = self.task_service.get_task(params).await {
                    if task.status.state == TaskState::InputRequired {
                        // We should continue this task instead of creating a new one
                        continue_task_id = Some(last_task_id.clone());
                    }
                }
            }
        }
    }
    
    let task = if let Some(task_id) = continue_task_id {
        // We're continuing an existing task
        info!("Continuing task {} that was in InputRequired state", task_id);
        
        // Create follow-up message params
        let params = TaskSendParams {
            id: task_id,
            message: initial_message,
            session_id: self.current_session_id.clone(),
            metadata: None,
        };
        
        // Process as follow-up
        self.task_service.process_task(params).await
            .map_err(|e| anyhow!("Failed to process follow-up task: {}", e))?
    } else {
        // Process as new task
        let params = TaskSendParams {
            id: task_id.clone(),
            message: initial_message,
            session_id: self.current_session_id.clone(),
            metadata: None,
        };
        
        self.task_service.process_task(params).await
            .map_err(|e| anyhow!("Failed to process task: {}", e))?
    };
    
    // If the task is in InputRequired state, indicate that in the response
    if task.status.state == TaskState::InputRequired {
        let mut response = extract_text_from_task(&task);
        response.push_str("\n\n[The agent needs more information. Your next message will continue this task.]");
        self.save_task_to_history(task).await?;
        return Ok(response);
    }
    
    // Normal response handling
    // ... existing code
}
```

### 5. Add Artifact Support to REPL

Enhance the REPL to handle artifacts in messages:

```rust
// Add to process_message_directly method to handle file uploads
pub async fn process_message_with_file(&self, message_text: &str, file_path: &str) -> Result<String> {
    // Read the file
    let file_content = fs::read(file_path)
        .map_err(|e| anyhow!("Failed to read file {}: {}", file_path, e))?;
    
    // Create a unique task ID
    let task_id = Uuid::new_v4().to_string();
    
    // Get filename
    let file_name = Path::new(file_path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or("file.bin")
        .to_string();
    
    // Create FilePart
    let file_part = FilePart {
        type_: "file".to_string(),
        file_name: Some(file_name),
        mime_type: None, // Or determine based on file extension
        data: file_content,
        size: Some(file_content.len() as u64),
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
    };
    
    // Use task_service to process the task
    let task = self.task_service.process_task(params).await
        .map_err(|e| anyhow!("Failed to process task with file: {}", e))?;
    
    // Save task to history
    self.save_task_to_history(task.clone()).await?;
    
    // Extract response text
    let response = extract_text_from_task(&task);
    
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

// Add to run_repl method
pub async fn run_repl(&mut self) -> Result<()> {
    // ... existing initialization
    
    // Add new commands to help message
    println!("  :file PATH MSG   - Send message with a file attachment");
    println!("  :data JSON MSG   - Send message with JSON data");
    
    // ... existing loop
    
    // Handle file command
    if input.starts_with(":file ") {
        let params = input.trim_start_matches(":file ").trim();
        
        // Parse file path and message
        let parts: Vec<&str> = params.splitn(2, " ").collect();
        if parts.len() < 2 {
            println!("❌ Error: Invalid command format. Use :file PATH MESSAGE");
            continue;
        }
        
        let file_path = parts[0];
        let message = parts[1];
        
        // Check if file exists
        if !Path::new(file_path).exists() {
            println!("❌ Error: File '{}' does not exist.", file_path);
            continue;
        }
        
        println!("📤 Sending message with file: {}", file_path);
        match self.process_message_with_file(message, file_path).await {
            Ok(response) => {
                println!("\n🤖 Agent response:\n{}\n", response);
            },
            Err(e) => {
                println!("❌ Error processing message with file: {}", e);
            }
        }
    }
    // Handle data command
    else if input.starts_with(":data ") {
        let params = input.trim_start_matches(":data ").trim();
        
        // Parse JSON and message
        let parts: Vec<&str> = params.splitn(2, " ").collect();
        if parts.len() < 2 {
            println!("❌ Error: Invalid command format. Use :data JSON MESSAGE");
            continue;
        }
        
        let json_str = parts[0];
        let message = parts[1];
        
        // Validate JSON
        match serde_json::from_str::<serde_json::Value>(json_str) {
            Ok(json_value) => {
                println!("📤 Sending message with JSON data");
                
                // Create a unique task ID
                let task_id = Uuid::new_v4().to_string();
                
                // Convert to Map for DataPart
                let data_map = match json_value {
                    serde_json::Value::Object(map) => map,
                    _ => {
                        println!("❌ Error: JSON must be an object");
                        continue;
                    }
                };
                
                // Create DataPart
                let data_part = DataPart {
                    type_: "json".to_string(),
                    data: data_map,
                    metadata: None,
                };
                
                // Create message with data part and text part
                let initial_message = Message {
                    role: Role::User,
                    parts: vec![
                        Part::TextPart(TextPart {
                            text: message.to_string(),
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
                };
                
                // Use task_service to process the task
                match self.task_service.process_task(params).await {
                    Ok(task) => {
                        // Save task to history
                        self.save_task_to_history(task.clone()).await?;
                        
                        // Extract response text
                        let response = extract_text_from_task(&task);
                        
                        // If we have artifacts, indicate them in the response
                        let mut full_response = response.clone();
                        if let Some(artifacts) = &task.artifacts {
                            if !artifacts.is_empty() {
                                let artifact_summary = format!("\n\n[This task has {} artifact(s). Use :artifacts {} to view them]", 
                                                             artifacts.len(), task.id);
                                full_response.push_str(&artifact_summary);
                            }
                        }
                        
                        println!("\n🤖 Agent response:\n{}\n", full_response);
                    },
                    Err(e) => {
                        println!("❌ Error processing message with data: {}", e);
                    }
                }
            },
            Err(e) => {
                println!("❌ Error: Invalid JSON: {}", e);
            }
        }
    }
    
    // ... remainder of run_repl method
}
```

## Comparison with Current Implementation

### Current Implementation Problems:

1. In the current `process_message_directly` method, a Task object is created artificially, but then the TaskService is bypassed:

```rust
// Current implementation problem - routing happens independently of TaskService
let router = BidirectionalTaskRouter::new(self.llm.clone(), self.agent_directory.clone());
let decision = router.decide_execution_mode(&task).await?;

match decision {
    ExecutionMode::Local => {
        // PROBLEM: Direct LLM call bypasses TaskService
        let prompt = format!("Process this request and provide a helpful response:\n\n{}", message_text);
        self.llm.complete(&prompt).await
    },
    // ...
}
```

2. When processing locally, the agent directly calls the LLM client instead of using TaskService's workflow that includes proper error handling, state management, artifact processing, and more.

3. The REPL has no commands to inspect or manipulate tasks, see their history, or view artifacts.

### Benefits of Proposed Improvements:

1. **Full TaskService Integration**: Using the TaskService's `process_task` method for all message processing ensures proper task state management, error handling, and artifact processing according to the A2A protocol.

2. **Session Continuity**: Maintaining task history in sessions provides context across multiple messages.

3. **Protocol Feature Exposure**: Adding REPL commands to access TaskService methods exposes the rich functionality of the A2A protocol:
   - State history tracking
   - Task cancellation
   - Artifact viewing
   - Follow-up message handling

4. **Better Error Handling**: The TaskService provides structured error handling through `ServerError` types.

5. **Proper InputRequired Handling**: The improvements enable proper handling of tasks that require additional input, supporting interactive workflows.

## Implementation Strategy

The improvements should be implemented in this order:

1. **Session Management Infrastructure**: First, implement the session tracking to enable conversation history.
2. **TaskService Integration**: Replace direct LLM calls with TaskService process_task calls.
3. **REPL Command Updates**: Add new REPL commands to interact with tasks and sessions.
4. **InputRequired Handling**: Improve the handling of tasks requiring additional input.
5. **Artifact Support**: Add file and data artifact support to REPL commands.

## Testing Strategy

Following the project's test-driven development approach, each improvement should be tested thoroughly:

1. Write unit tests for session management methods.
2. Create integration tests that compare responses from direct LLM calls vs. TaskService.
3. Test each new REPL command with various input scenarios.
4. Verify InputRequired handling with mock tasks that return different states.
5. Test artifact support with file and data transfers, ensuring proper processing.

By implementing these improvements, the bidirectional agent will better leverage the TaskService capabilities, providing a more powerful and feature-rich agent experience that aligns with the A2A protocol standard.