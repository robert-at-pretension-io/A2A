use crate::client::A2aClient;
use crate::mock_server::start_mock_server;
use crate::types::TaskState;
use std::error::Error;
use std::thread;
use tempfile::tempdir;
use std::path::Path;
use std::fs::File;
use std::io::Write;

// This test will exercise all the new features we've added
// - File operations
// - Data operations 
// - Artifact handling
#[tokio::test]
async fn test_file_data_artifact_features() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8088;
    thread::spawn(move || {
        start_mock_server(port);
    });
    
    // Wait for server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    // Create a temporary test file
    let temp_dir = tempdir()?;
    let file_path = temp_dir.path().join("test_file.txt");
    let file_contents = "This is a test file for A2A client file operations";
    
    {
        let mut file = File::create(&file_path)?;
        file.write_all(file_contents.as_bytes())?;
    }
    
    // Test sending a task with a file attachment
    println!("Testing send_task_with_file...");
    let task_with_file = client.send_task_with_file(
        "Here's a file for processing", 
        file_path.to_str().unwrap()
    ).await?;
    
    // Check if we got artifacts in response
    if let Some(artifacts) = &task_with_file.artifacts {
        println!("Received {} artifacts", artifacts.len());
        
        // Save artifacts to temporary directory
        let output_dir = temp_dir.path().join("output");
        let saved_artifacts = client.save_artifacts(artifacts, output_dir.to_str().unwrap())?;
        
        println!("Saved {} artifacts", saved_artifacts.len());
        for artifact in &saved_artifacts {
            println!("Artifact: {} -> {}", artifact.name, artifact.path);
        }
        
        // Extract text from artifacts
        let texts = A2aClient::extract_artifact_text(artifacts);
        println!("Extracted {} text parts", texts.len());
    }
    
    // Test sending a task with structured data
    println!("Testing send_task_with_data...");
    let data = serde_json::json!({
        "analysis_parameters": {
            "model": "test-model",
            "temperature": 0.7,
            "max_tokens": 1000
        },
        "flags": {
            "debug": true,
            "verbose": false
        }
    });
    
    let task_with_data = client.send_task_with_data(
        "Process this data", 
        &data
    ).await?;
    
    // Check if we got artifacts in response to data
    if let Some(artifacts) = &task_with_data.artifacts {
        println!("Received {} artifacts for data task", artifacts.len());
    }
    
    // Test streaming with different artifact types
    println!("Testing streaming with artifacts...");
    let mut stream = client.send_task_subscribe("Test streaming with different artifact types").await?;
    
    println!("Stream created, waiting for updates...");
    use futures_util::StreamExt;
    use crate::client::streaming::StreamingResponse;
    
    // Collect all streaming updates
    let mut artifacts_count = 0;
    
    while let Some(update) = stream.next().await {
        match update {
            Ok(StreamingResponse::Artifact(artifact)) => {
                println!("Received artifact: index {}", artifact.index);
                artifacts_count += 1;
            },
            Ok(StreamingResponse::Status(task)) => {
                println!("Status update: {}", task.status.state);
            },
            Ok(StreamingResponse::Final(task)) => {
                println!("Final status: {}", task.status.state);
                break;
            },
            Err(e) => {
                println!("Stream error: {}", e);
                break;
            }
        }
    }
    
    println!("Received {} streaming artifacts", artifacts_count);
    assert!(artifacts_count > 0, "Should have received at least one artifact in streaming response");
    
    // Clean up
    temp_dir.close()?;
    
    Ok(())
}

#[tokio::test]
async fn test_state_transition_history() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8089;
    thread::spawn(move || {
        start_mock_server(port);
    });
    
    // Wait for server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    println!("Testing state transition history...");
    
    // Create a task to get state history (this will trigger multiple state transitions)
    let task = client.send_task("Create a task with state history tracking").await?;
    println!("Created task ID: {}", task.id);
    
    // Get state transition history
    let history = client.get_task_state_history(&task.id).await?;
    
    println!("State transitions for task {}:", history.task_id);
    for (i, transition) in history.transitions.iter().enumerate() {
        println!("{}. State: {}", i + 1, transition.state);
    }
    
    // Verify we have multiple transitions 
    assert!(history.transitions.len() >= 2, "Should have at least 2 state transitions");
    
    // Verify the states follow a logical progression
    if history.transitions.len() >= 3 {
        assert_eq!(history.transitions[0].state, TaskState::Submitted, "First state should be Submitted");
        assert_eq!(history.transitions[1].state, TaskState::Working, "Second state should be Working");
        
        // The final state should be either Completed or another terminal state
        let last_state = &history.transitions.last().unwrap().state;
        assert!(matches!(last_state, 
            TaskState::Completed | TaskState::Failed | TaskState::Canceled),
            "Final state should be a terminal state"
        );
    }
    
    // Get state transition metrics
    let metrics = client.get_state_transition_metrics(&task.id).await?;
    println!("\nState transition metrics:");
    println!("{}", metrics.to_string());
    
    // Verify metrics
    assert!(metrics.total_transitions >= 2, "Should have at least 2 transitions in metrics");
    assert!(metrics.submitted_count >= 1, "Should have at least one Submitted state");
    assert!(metrics.working_count >= 1, "Should have at least one Working state");
    
    // Get state history report (formatted)
    let report = client.get_state_history_report(&task.id).await?;
    println!("\nState history report:\n{}", report);
    
    // Cancel a task and verify the Canceled state is tracked
    let task_to_cancel = client.send_task("This task will be canceled").await?;
    println!("\nCreated task for cancellation: {}", task_to_cancel.id);
    
    // Immediately cancel the task
    let cancel_result = client.cancel_task(&task_to_cancel.id).await?;
    
    // Get state history for canceled task
    let cancel_history = client.get_task_state_history(&task_to_cancel.id).await?;
    
    println!("State transitions for canceled task {}:", cancel_history.task_id);
    for (i, transition) in cancel_history.transitions.iter().enumerate() {
        println!("{}. State: {}", i + 1, transition.state);
    }
    
    // Verify that the last state is Canceled
    if let Some(last_transition) = cancel_history.transitions.last() {
        assert_eq!(last_transition.state, TaskState::Canceled, "Last state should be Canceled");
    }
    
    Ok(())
}

#[tokio::test]
async fn test_task_batch_operations() -> Result<(), Box<dyn Error>> {
    use crate::client::task_batch::{BatchCreateParams, BatchStatus};
    
    // Start mock server in a separate thread
    let port = 8090;
    thread::spawn(move || {
        start_mock_server(port);
    });
    
    // Wait for server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    println!("Testing task batch operations...");
    
    // Create a batch of tasks
    let batch_params = BatchCreateParams {
        id: Some("test-batch-1".to_string()),
        name: Some("Test Batch".to_string()),
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
    
    // Check that the batch has the correct number of tasks
    assert_eq!(batch.task_ids.len(), 3, "Batch should contain 3 tasks");
    assert_eq!(batch.id, "test-batch-1", "Batch ID should match");
    assert_eq!(batch.name, Some("Test Batch".to_string()), "Batch name should match");
    
    // Get the batch status
    let status = client.get_batch_status(&batch.id).await?;
    println!("Batch status: {:?}", status.overall_status);
    
    // Check status metrics
    assert_eq!(status.total_tasks, 3, "Status should report 3 tasks");
    
    // Get all tasks in the batch
    let tasks = client.get_batch_tasks(&batch.id).await?;
    println!("Retrieved {} tasks", tasks.len());
    
    // Check tasks
    assert_eq!(tasks.len(), 3, "Should retrieve 3 tasks");
    
    // Test canceling a batch
    let cancel_status = client.cancel_batch(&batch.id).await?;
    println!("After cancellation status: {:?}", cancel_status.overall_status);
    
    // Verify at least some tasks are canceled
    assert!(cancel_status.overall_status == BatchStatus::Canceled || 
           cancel_status.overall_status == BatchStatus::PartlyCanceled,
           "Batch should be fully or partly canceled");
    
    Ok(())
}