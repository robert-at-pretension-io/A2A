# A2A Client

This is a client implementation for the Agent-to-Agent (A2A) protocol, allowing applications to communicate with any A2A-compatible agent.

For the complete protocol specification, see the [A2A Schema Overview](../docs/schema_overview.md).

## Features

### Core Features
- Task creation and management
- Basic text message support
- Task status tracking
- Authentication support

### Advanced Features
- Streaming responses via Server-Sent Events (SSE)
- Push notifications
- File operations
- Structured data operations
- Artifact management
- State transition history tracking and analysis
- Task batching for multi-task operations
- Agent skills discovery and invocation

## Usage Examples

### Basic Text Task

```rust
let mut client = A2aClient::new("https://example.com/a2a");
let task = client.send_task("Hello, A2A server!").await?;
println!("Task ID: {}", task.id);
```

### File Operations

```rust
// Send a task with a file attachment
let task = client.send_task_with_file(
    "Process this image", 
    "/path/to/image.jpg"
).await?;

// Or with file bytes
let file_bytes = std::fs::read("/path/to/document.pdf")?;
let task = client.send_task_with_file_bytes(
    "Analyze this document",
    &file_bytes,
    "document.pdf",
    Some("application/pdf")
).await?;
```

### Data Operations

```rust
// Send a task with structured data
let data = serde_json::json!({
    "parameters": {
        "model": "gpt-4",
        "temperature": 0.7,
        "max_tokens": 1000
    },
    "config": {
        "stream": true,
        "verbose": false
    }
});

let task = client.send_task_with_data("Run analysis with these parameters", &data).await?;
```

### Artifact Management

```rust
// Get artifacts from a task
let artifacts = client.get_task_artifacts("task-123").await?;

// Save artifacts to disk
let saved = client.save_artifacts(&artifacts, "/output/directory")?;
for artifact in saved {
    println!("Saved: {} -> {}", artifact.name, artifact.path);
}

// Extract text from artifacts
let texts = A2aClient::extract_artifact_text(&artifacts);
for text in texts {
    println!("Text: {}", text);
}
```

### Streaming

```rust
let mut stream = client.send_task_subscribe("Long running task").await?;

while let Some(update) = stream.next().await {
    match update {
        Ok(StreamingResponse::Artifact(artifact)) => {
            // Handle streamed artifact
        },
        Ok(StreamingResponse::Status(task)) => {
            // Handle status update
        },
        Ok(StreamingResponse::Final(task)) => {
            break; // Stream ended
        },
        Err(e) => {
            // Handle error
        }
    }
}
```

### State Transition History

```rust
// Get detailed state transition history for a task
let history = client.get_task_state_history("task-123").await?;

// Display transitions
for transition in &history.transitions {
    println!("State: {}, Time: {}", transition.state, transition.timestamp);
    
    // Transitions may include messages
    if let Some(message) = &transition.message {
        println!("With message from: {}", message.role);
    }
}

// Get a formatted report
let report = client.get_state_history_report("task-123").await?;
println!("{}", report);

// Get metrics about the task's transitions
let metrics = client.get_state_transition_metrics("task-123").await?;
println!("Total transitions: {}", metrics.total_transitions);
println!("Submission to completion: {}ms", metrics.duration.unwrap_or(0));
```

### Task Batching

```rust
// Create a batch of related tasks
let batch_params = BatchCreateParams {
    id: Some("batch-001".to_string()),
    name: Some("Document Processing Batch".to_string()),
    tasks: vec![
        "Process document 1".to_string(),
        "Process document 2".to_string(),
        "Process document 3".to_string(),
    ],
    metadata: None,
};

// Create the batch
let batch = client.create_task_batch(batch_params).await?;
println!("Created batch ID: {} with {} tasks", batch.id, batch.task_ids.len());

// Get status of all tasks in the batch
let status = client.get_batch_status(&batch.id).await?;
println!("Batch status: {:?}", status.overall_status);
println!("Tasks completed: {}/{}", 
    status.state_counts.get(&TaskState::Completed).unwrap_or(&0),
    status.total_tasks);

// Retrieve all tasks in the batch
let tasks = client.get_batch_tasks(&batch.id).await?;
for task in &tasks {
    println!("Task {} status: {}", task.id, task.status.state);
}

// Cancel all tasks in a batch
let cancel_status = client.cancel_batch(&batch.id).await?;
println!("After cancellation: {:?}", cancel_status.overall_status);
```

### Agent Skills

```rust
// List all available skills
let skills = client.list_skills(None).await?;
println!("Agent offers {} skills:", skills.skills.len());
for skill in &skills.skills {
    println!("- {} ({})", skill.name, skill.id);
}

// Filter skills by tag
let analysis_skills = client.list_skills(Some(vec!["analysis".to_string()])).await?;
println!("Found {} analysis skills", analysis_skills.skills.len());

// Get detailed information about a specific skill
let skill_details = client.get_skill_details("summarize-skill").await?;
println!("Skill: {} ({})", skill_details.skill.name, skill_details.skill.id);
println!("Description: {}", skill_details.skill.description.unwrap_or_default());

// Display examples if available
if let Some(examples) = &skill_details.skill.examples {
    println!("Examples:");
    for example in examples {
        println!("- {}", example);
    }
}

// Invoke a skill
let task = client.invoke_skill(
    "summarize-skill",
    "Please summarize this long article: [article text here]",
    Some("text/plain".to_string()),  // Input mode
    None                             // Use default output mode
).await?;

// Skill results are available as task artifacts
if let Some(artifacts) = &task.artifacts {
    for artifact in artifacts {
        // Process skill results
        println!("Skill result: {}", artifact.name.as_deref().unwrap_or("Result"));
        // Extract text content
        for part in &artifact.parts {
            if let Part::TextPart(text_part) = part {
                println!("{}", text_part.text);
            }
        }
    }
}
```

## Testing Features

### Configurable Delays

The mock server supports configurable delays to simulate network latency and server processing time, which is useful for testing client timeout handling and UI responsiveness:

```rust
// Send a task with a 2-second delay
let result = client.send_task_with_metadata(
    "Hello with delay", 
    Some(r#"{"_mock_delay_ms": 2000}"#)
).await?;

// Stream with slow chunk delivery (1 second between chunks)
let stream = client.send_task_subscribe_with_metadata(
    "Stream with slow chunks",
    &json!({
        "_mock_chunk_delay_ms": 1000
    })
).await?;
```

From the command line:
```bash
# Task with 2-second processing delay
cargo run -- client send-task --url "http://localhost:8080" \
  --message "Slow task" --metadata '{"_mock_delay_ms": 2000}'

# Stream with 1-second delay between chunks
cargo run -- client stream-task --url "http://localhost:8080" \
  --message "Slow stream" --metadata '{"_mock_chunk_delay_ms": 1000}'
```

### Dynamic Streaming Content

The mock server can simulate different streaming patterns with configurable content types, chunk counts, and final states. This helps test client robustness when processing varied streaming content:

```rust
// Configure a stream with 3 text chunks
let stream = client.send_task_subscribe_with_metadata(
    "Stream with custom text chunks",
    &json!({
        "_mock_stream_text_chunks": 3,
        "_mock_stream_chunk_delay_ms": 100
    })
).await?;

// Stream with only data and file artifacts (no text)
let stream = client.send_task_subscribe_with_metadata(
    "Stream with data and file artifacts only",
    &json!({
        "_mock_stream_artifact_types": ["data", "file"],
        "_mock_stream_chunk_delay_ms": 200
    })
).await?;

// Stream that ends with a failed status
let stream = client.send_task_subscribe_with_metadata(
    "Stream ending with error",
    &json!({
        "_mock_stream_final_state": "failed"
    })
).await?;

// Resubscribe to an existing task with custom configuration
let stream = client.resubscribe_task_with_metadata(
    "task-123",
    &json!({
        "_mock_stream_text_chunks": 2,
        "_mock_stream_artifact_types": ["text", "data"]
    })
).await?;
```

From the command line:
```bash
# Stream with 5 text chunks
cargo run -- client stream-task --url "http://localhost:8080" \
  --message "Custom text chunks" --metadata '{"_mock_stream_text_chunks": 5}'

# Stream with data artifact only
cargo run -- client stream-task --url "http://localhost:8080" \
  --message "Data only stream" --metadata '{"_mock_stream_artifact_types": ["data"]}'

# Stream ending in failed state
cargo run -- client stream-task --url "http://localhost:8080" \
  --message "Failing stream" --metadata '{"_mock_stream_final_state": "failed"}'
```

### State Machine Fidelity

The mock server can simulate realistic task state machine behavior, allowing testing of long-running tasks, multi-turn conversations requiring input, and failure handling:

```rust
// Create a task that takes 5 seconds to complete
let task = client.simulate_task_lifecycle(
    "Long running task",
    5000,  // Takes 5 seconds
    false, // No input required
    false, // Don't fail
    None   // No failure message
).await?;

// Check task status during processing
// Initially it will be Submitted, then Working, then Completed

// Create a task that requires additional input
let task = client.simulate_task_lifecycle(
    "Task needing more information",
    10000, // Takes 10 seconds total
    true,  // Requires input
    false, // Don't fail 
    None   // No failure message
).await?;

// After a few seconds, task will transition to InputRequired state
// Send a follow-up message to provide input
let follow_up = client.send_task_with_metadata(
    "Here's the additional information you requested",
    Some(&format!(r#"{{"id": "{}"}}"#, task.id))
).await?;

// Simulate a task that fails
let failing_task = client.simulate_task_lifecycle(
    "Task that will fail",
    3000,  // Takes 3 seconds
    false, // No input required
    true,  // Will fail
    Some("Simulated failure message") // Custom failure message
).await?;
```

From the command line:
```bash
# Task that simulates realistic processing time
cargo run -- client send-task --url "http://localhost:8080" \
  --message "Realistic task" --metadata '{"_mock_duration_ms": 5000}'

# Task that requires additional input
cargo run -- client send-task --url "http://localhost:8080" \
  --message "Interactive task" --metadata '{"_mock_duration_ms": 6000, "_mock_require_input": true}'

# Task that will fail
cargo run -- client send-task --url "http://localhost:8080" \
  --message "Failing task" --metadata '{"_mock_duration_ms": 3000, "_mock_fail": true, "_mock_fail_message": "Custom error message"}'

# Skills can also be simulated with realistic processing
cargo run -- client invoke-skill --url "http://localhost:8080" \
  --id "test-skill-1" --message "Process with realism" \
  --metadata '{"_mock_duration_ms": 4000}'
```

## More Information

- [Mock Server Implementation](../mock_server.rs): Reference implementation for A2A server endpoints.
- [Client Tests](./tests/integration_test.rs): Integration tests demonstrating client-server interactions. 
- [Schema Documentation](../docs/schema_overview.md): Detailed schema definitions for the A2A protocol.
- [A2A Test Suite](../../README.md): Overview of the full A2A testing framework.
- [Development Workflow](../../CLAUDE.md#optimal-feature-development-workflow): Guidelines for adding new features.

## Type Definitions

The client uses strongly-typed structures following the A2A protocol. Key types include:

- `Message` - A message with one or more content parts
- `Part` - Content part types including text, files, and data
- `Task` - A task with its status and artifacts
- `Artifact` - Output artifacts produced by tasks
- `PushNotificationConfig` - Configuration for webhook notifications
- `TaskBatch` - A group of related tasks managed together
- `BatchStatus` - Overall status of a task batch (Completed, Working, etc)
- `AgentSkill` - Description of a skill offered by an agent
- `SkillListResponse` - Response containing available skills
- `SkillDetailsResponse` - Detailed information about a specific skill

See [types.rs](../types.rs) for complete type definitions.