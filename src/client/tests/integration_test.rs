use crate::client::A2aClient;
use crate::mock_server::{start_mock_server, start_mock_server_with_auth};
use crate::types::{TaskState, AgentSkill};
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
// - Authentication
#[tokio::test]
async fn test_file_data_artifact_features() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread (without requiring auth)
    let port = 8088;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
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
    // Start mock server in a separate thread (without requiring auth)
    let port = 8089;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
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
    
    // Immediately cancel the task using _typed version
    let cancel_result = client.cancel_task_typed(&task_to_cancel.id).await?;

    // Get state history for canceled task using _typed version
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
    
    // Start mock server in a separate thread (without requiring auth)
    let port = 8090;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
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
    
    // Create the batch using _typed version
    let batch = client.create_task_batch_typed(batch_params).await?;
    println!("Created batch ID: {} with {} tasks", batch.id, batch.task_ids.len());

    // Check that the batch has the correct number of tasks
    assert_eq!(batch.task_ids.len(), 3, "Batch should contain 3 tasks");
    assert_eq!(batch.id, "test-batch-1", "Batch ID should match");
    assert_eq!(batch.name, Some("Test Batch".to_string()), "Batch name should match");

    // Get the batch status using _typed version
    let status = client.get_batch_status_typed(&batch.id).await?;
    println!("Batch status: {:?}", status.overall_status);

    // Check status metrics
    assert_eq!(status.total_tasks, 3, "Status should report 3 tasks");

    // Get all tasks in the batch using _typed version
    let tasks = client.get_batch_tasks_typed(&batch.id).await?;
    println!("Retrieved {} tasks", tasks.len());

    // Check tasks
    assert_eq!(tasks.len(), 3, "Should retrieve 3 tasks");

    // Test canceling a batch using _typed version
    let cancel_status = client.cancel_batch_typed(&batch.id).await?;
    println!("After cancellation status: {:?}", cancel_status.overall_status);

    // Verify at least some tasks are canceled
    assert!(cancel_status.overall_status == BatchStatus::Canceled || 
           cancel_status.overall_status == BatchStatus::PartlyCanceled,
           "Batch should be fully or partly canceled");
    
    Ok(())
}

#[tokio::test]
async fn test_agent_skills_operations() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread (without requiring auth)
    let port = 8091;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
    });
    
    // Wait for server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));

    println!("Testing agent skills operations...");

    // List all available skills using _typed version
    let skills_response = client.list_skills_typed(None).await?;
    println!("Found {} skills", skills_response.skills.len());

    // Verify we have at least one skill
    assert!(!skills_response.skills.is_empty(), "Should have at least one skill");
    
    // Grab the first skill ID for later tests
    let first_skill_id = skills_response.skills[0].id.clone();

    // Test filtering skills by tags using _typed version
    let text_skills = client.list_skills_typed(Some(vec!["text".to_string()])).await?;
    println!("Found {} skills with 'text' tag", text_skills.skills.len());

    // Verify that filtered skills all have the requested tag
    for skill in &text_skills.skills {
        if let Some(tags) = &skill.tags {
            assert!(tags.contains(&"text".to_string()), 
                   "Filtered skill should have 'text' tag");
        }
    }
    
    // Get details for a specific skill
    let skill_details = client.get_skill_details(&first_skill_id).await?;
    println!("Got details for skill: {} ({})", skill_details.skill.name, skill_details.skill.id);
    
    // Verify skill details
    assert_eq!(skill_details.skill.id, first_skill_id, "Skill ID should match");
    assert!(!skill_details.skill.name.is_empty(), "Skill should have a name");

    // Invoke a skill using _typed version
    println!("Invoking skill: {}", first_skill_id);
    let task = client.invoke_skill_typed(
        &first_skill_id,
        "Test skill invocation message",
        None,
        None
    ).await?;
    
    println!("Skill invocation created task: {}", task.id);
    
    // Verify that task has artifacts
    if let Some(artifacts) = &task.artifacts {
        println!("Skill returned {} artifacts", artifacts.len());
        assert!(!artifacts.is_empty(), "Skill should return at least one artifact");
        
        // Check if the artifact has the expected content
        for artifact in artifacts {
            if let Some(name) = &artifact.name {
                println!("Artifact name: {}", name);
                assert!(name.contains(&first_skill_id), "Artifact name should contain skill ID");
            }
            
            // Extract text content if available
            for part in &artifact.parts {
                if let crate::types::Part::TextPart(text_part) = part {
                    println!("Artifact content: {}", text_part.text);
                }
            }
        }
    } else {
        assert!(false, "Skill invocation should return artifacts");
    }
    
    // Test the agent card to verify skills integration
    let agent_card = client.get_agent_card().await?;
    println!("Agent card lists {} skills", agent_card.skills.len());
    
    // Verify that the agent card contains the skills
    assert!(!agent_card.skills.is_empty(), "Agent card should list skills");
    
    // Verify our test skill is in the agent card
    let has_test_skill = agent_card.skills.iter()
        .any(|skill| skill.id == first_skill_id);
    assert!(has_test_skill, "Agent card should contain our test skill");
    
    Ok(())
}

#[tokio::test]
async fn test_auth_integration_flow() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8092;
    thread::spawn(move || {
        start_mock_server(port);
    });
    
    // Wait for server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    println!("Testing authentication integration flow...");
    
    // Test 1: Get agent card to check auth requirements
    let base_url = format!("http://localhost:{}", port);
    let client_no_auth = A2aClient::new(&base_url);
    let agent_card = client_no_auth.get_agent_card().await?;
    
    // Verify agent card has auth requirements
    assert!(agent_card.authentication.is_some(), "Agent card should have authentication field");
    let auth_schemes = &agent_card.authentication.unwrap().schemes;
    println!("Agent requires authentication schemes: {:?}", auth_schemes);
    
    // Test 2: Attempt operation without auth (should fail)
    let mut client_no_auth = A2aClient::new(&base_url);
    let task_id = "auth-test-task";
    let get_result = client_no_auth.get_task(task_id).await;
    
    // Should be an error because we don't have authentication
    assert!(get_result.is_err(), "Request without auth should fail");
    println!("Request without auth failed as expected: {}", get_result.unwrap_err());
    
    // Test 3: Create client with Bearer token auth
    let auth_header = "Authorization";
    let auth_value = "Bearer test-token-123";
    let mut client_with_bearer = A2aClient::new(&base_url)
        .with_auth(auth_header, auth_value);

    // Validate auth works using _typed version
    let is_valid = client_with_bearer.validate_auth_typed().await?;
    assert!(is_valid, "Auth validation should succeed with valid Bearer token");

    // Perform operations with auth
    let task = client_with_bearer.send_task("Task with auth").await?;
    println!("Successfully created task with auth: {}", task.id);
    
    let task_details = client_with_bearer.get_task(&task.id).await?;
    println!("Successfully retrieved task with auth: {} (state: {})", 
             task_details.id, task_details.status.state);
    
    // Test 4: Create client with API Key auth
    let api_key_header = "X-API-Key";
    let api_key_value = "test-api-key-123";
    let mut client_with_apikey = A2aClient::new(&base_url)
        .with_auth(api_key_header, api_key_value);

    // Validate API Key auth works using _typed version
    let is_valid = client_with_apikey.validate_auth_typed().await?;
    assert!(is_valid, "Auth validation should succeed with valid API Key");

    // Perform operations with API Key auth
    let task = client_with_apikey.send_task("Task with API Key auth").await?;
    println!("Successfully created task with API Key auth: {}", task.id);
    
    // Test 5: Test canceling a task with authentication
    let canceled_task_id = client_with_bearer.cancel_task(&task.id).await?;
    println!("Successfully canceled task with auth: {}", canceled_task_id);
    
    // Get the task to verify it's canceled
    let canceled_task = client_with_bearer.get_task(&canceled_task_id).await?;
    assert_eq!(canceled_task.status.state, TaskState::Canceled, 
               "Task should be canceled");
    
    // Test 6: Verify auth with other operations (batch, streaming, etc.)
    // Create a batch of tasks
    use crate::client::task_batch::BatchCreateParams;
    
    let batch_params = BatchCreateParams {
        id: Some("auth-test-batch".to_string()),
        name: Some("Auth Test Batch".to_string()),
        tasks: vec![
            "Auth test task 1".to_string(),
            "Auth test task 2".to_string(),
        ],
        metadata: None,
    };
    
    // Create the batch with auth
    let batch = client_with_bearer.create_task_batch(batch_params).await?;
    println!("Created batch with auth: {} with {} tasks", 
             batch.id, batch.task_ids.len());
    
    // Get batch status with auth
    let status = client_with_bearer.get_batch_status(&batch.id).await?;
    println!("Successfully retrieved batch status with auth: {:?}", 
             status.overall_status);

    // Test 7: Streaming with auth using _typed version
    println!("Testing streaming with auth...");
    let mut stream = client_with_bearer
        .send_task_subscribe_typed("Test streaming with authentication").await?;

    println!("Stream created with auth, waiting for updates...");
    use futures_util::StreamExt;
    use crate::client::streaming::StreamingResponse;
    
    // Collect a few streaming updates
    let mut updates_count = 0;
    while let Some(update) = stream.next().await {
        match update {
            Ok(StreamingResponse::Artifact(artifact)) => {
                println!("Received artifact with auth: index {}", artifact.index);
                updates_count += 1;
            },
            Ok(StreamingResponse::Status(task)) => {
                println!("Status update with auth: {}", task.status.state);
                updates_count += 1;
            },
            Ok(StreamingResponse::Final(task)) => {
                println!("Final status with auth: {}", task.status.state);
                break;
            },
            Err(e) => {
                println!("Stream error: {}", e);
                break;
            }
        }
        
        // Only read a few updates to keep test fast
        if updates_count >= 3 {
            break;
        }
    }
    
    assert!(updates_count > 0, "Should have received at least one streaming update with auth");
    
    Ok(())
}
