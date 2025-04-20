use crate::client::A2aClient;
use crate::mock_server::{start_mock_server, start_mock_server_with_auth};
use crate::types::TaskState;
use std::error::Error;
use std::thread;
use tokio::time;
use std::time::Duration;

/// Tests the state machine functionality with a task that transitions through multiple states.
/// This test verifies that tasks properly transition through different states over time.
#[tokio::test]
async fn test_task_state_machine() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8110;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
    });
    
    // Wait for server to start
    time::sleep(time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    // Send the task with simulated state machine lifecycle
    println!("Sending task with simulated state machine lifecycle");
    let task = client.simulate_task_lifecycle(
        "Test task with simulated state machine",
        None,      // Generate random ID
        4000,      // 4 second duration
        false,     // No input required
        false,     // Don't fail
        None       // No failure message
    ).await?;
    println!("Task created with ID: {}", task.id);
    
    // Verify initial state is Submitted
    assert_eq!(task.status.state, TaskState::Submitted, "Initial state should be Submitted");
    
    // Wait a second and check if state transitioned to Working
    time::sleep(time::Duration::from_millis(1000)).await;
    let task = client.get_task(&task.id).await?;
    println!("State after 1 second: {:?}", task.status.state);
    assert_eq!(task.status.state, TaskState::Working, "State should transition to Working");
    
    // Wait for the task to complete (just over the 4 second duration)
    time::sleep(time::Duration::from_millis(3500)).await;
    let task = client.get_task(&task.id).await?;
    println!("Final state: {:?}", task.status.state);
    assert_eq!(task.status.state, TaskState::Completed, "Final state should be Completed");
    
    // Validate history contains all transitions
    assert!(task.history.is_some(), "Task should include history");
    if let Some(history) = &task.history {
        println!("History length: {}", history.len());
        // The history should include at least Submitted, Working, and Completed states
        assert!(history.len() >= 3, "History should include at least 3 messages");
    }
    
    Ok(())
}

/// Tests a task that requires input to continue
#[tokio::test]
async fn test_task_requiring_input() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8111;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
    });
    
    // Wait for server to start
    time::sleep(time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    // Create a task with a unique ID that we'll pass to both initial and follow-up requests
    let unique_task_id = format!("input-required-task-{}", uuid::Uuid::new_v4());
    
    // Send the task with the simulation config and a predefined ID
    println!("Sending task that requires input with ID: {}", unique_task_id);
    
    // Use the more direct method with explicit task_id parameter
    let task = client.simulate_task_lifecycle(
        "Test task requiring input",
        Some(&unique_task_id),  // Use our predefined ID
        4000,                  // Duration 4 seconds
        true,                  // Require input
        false,                 // Don't fail
        None                   // No failure message
    ).await?;
    
    // Verify the ID matches what we expect
    assert_eq!(task.id, unique_task_id, "Task ID should match our predefined ID");
    println!("Task created with ID: {}", task.id);
    
    // Wait 2.5 seconds to let it transition to InputRequired
    time::sleep(time::Duration::from_millis(2500)).await;
    let task = client.get_task(&task.id).await?;
    println!("State after 2.5 seconds: {:?}", task.status.state);
    assert_eq!(task.status.state, TaskState::InputRequired, "State should transition to InputRequired");
    
    // Now send a follow-up message to provide the input to the same task ID
    println!("Sending follow-up input for task: {}", unique_task_id);
    
    // Create task parameters directly (don't use metadata)
    let text_part = crate::types::TextPart {
        type_: "text".to_string(),
        text: "Here's the additional information you requested".to_string(),
        metadata: None,
    };
    
    let message = crate::types::Message {
        role: crate::types::Role::User,
        parts: vec![crate::types::Part::TextPart(text_part)],
        metadata: None,
    };
    
    // Create task parameters with same ID
    let params = crate::types::TaskSendParams {
        id: unique_task_id.clone(),
        message: message,
        history_length: None,
        metadata: None, 
        push_notification: None,
        session_id: None,
    };
    
    // Send the follow-up message with direct params
    println!("Explicitly sending with ID: {}", unique_task_id);
    let follow_up = client.send_jsonrpc::<crate::types::Task>(
        "tasks/send", 
        serde_json::to_value(params).unwrap()
    ).await?;
    println!("Follow-up response received");
    
    // Print the response JSON for debugging
    println!("Follow-up task state: {:?}", follow_up.status.state);
    
    // The response might be either Working or Completed, both are acceptable
    assert!(follow_up.status.state == TaskState::Working || follow_up.status.state == TaskState::Completed, 
           "Follow-up response should be either Working or Completed state");
    
    // Give the server a little time to update the task state after receiving input
    // It may take a moment for the task in storage to be updated
    println!("Waiting for task state to update after input...");
    
    // Try polling a few times to see if the state changes
    for attempt in 1..=5 {
        time::sleep(time::Duration::from_millis(200)).await;
        let task = client.get_task(&task.id).await?;
        println!("State after follow-up (attempt {}): {:?}", attempt, task.status.state);
        
        // If the state has changed from InputRequired, we can break
        if task.status.state != TaskState::InputRequired {
            // The task should either be Working or Completed after input
            assert!(task.status.state == TaskState::Working || task.status.state == TaskState::Completed,
                  "Task should transition to Working or Completed after input");
            break;
        }
        
        // If we've tried 5 times and the state is still InputRequired, fail the test
        if attempt == 5 && task.status.state == TaskState::InputRequired {
            // Print full task details for debugging
            println!("TASK DETAILS: {:?}", task);
            
            // Fail the test
            panic!("Task still in InputRequired state after 5 polling attempts");
        }
    }
    
    // Wait for completion
    time::sleep(time::Duration::from_millis(2500)).await;
    let task = client.get_task(&task.id).await?;
    println!("Final state: {:?}", task.status.state);
    assert_eq!(task.status.state, TaskState::Completed, "Final state should be Completed");
    
    Ok(())
}

/// Tests a simulated task failure
#[tokio::test]
async fn test_task_failure() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8112;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
    });
    
    // Wait for server to start
    time::sleep(time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    // Send a task that will simulate failure
    println!("Sending task that will fail");
    let task = client.simulate_task_lifecycle(
        "Test task that will fail",
        None,       // Generate random ID
        3000,       // 3 second duration
        false,      // No input required
        true,       // Will fail
        Some("Simulated task failure for testing") // Failure message
    ).await?;
    println!("Task created with ID: {}", task.id);
    
    // Wait a bit for the task to start processing
    time::sleep(time::Duration::from_millis(1000)).await;
    let task = client.get_task(&task.id).await?;
    println!("State after 1 second: {:?}", task.status.state);
    assert_eq!(task.status.state, TaskState::Working, "State should transition to Working");
    
    // Wait for the task to fail
    time::sleep(time::Duration::from_millis(2500)).await;
    let task = client.get_task(&task.id).await?;
    println!("Final state: {:?}", task.status.state);
    assert_eq!(task.status.state, TaskState::Failed, "Final state should be Failed");
    
    // Verify the failure message
    if let Some(message) = &task.status.message {
        let text = message.parts.iter().find_map(|part| {
            if let crate::types::Part::TextPart(text_part) = part {
                Some(&text_part.text)
            } else {
                None
            }
        });
        
        if let Some(message_text) = text {
            println!("Failure message: {}", message_text);
            assert!(message_text.contains("Simulated task failure"), "Failure message should contain expected text");
        } else {
            return Err("No text part found in failure message".into());
        }
    } else {
        return Err("No failure message found".into());
    }
    
    Ok(())
}

/// Tests skill invocation with state machine simulation
#[tokio::test]
async fn test_skill_invocation_state_machine() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8113;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
    });
    
    // Wait for server to start
    time::sleep(time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    // Get the skill details
    let skill_id = "test-skill-1"; // Echo skill
    
    // Create a message for the skill
    let text_part = crate::types::TextPart {
        type_: "text".to_string(),
        text: "Echo this message with state machine simulation".to_string(),
        metadata: None,
    };
    
    let message = crate::types::Message {
        role: crate::types::Role::User,
        parts: vec![crate::types::Part::TextPart(text_part)],
        metadata: None,
    };
    
    // Configure the skill invocation with state machine simulation
    let params = serde_json::json!({
        "id": skill_id,
        "message": message,
        "metadata": {
            "_mock_duration_ms": 3000,
            "_mock_require_input": false
        }
    });
    
    // Invoke the skill
    println!("Invoking skill with state machine simulation");
    let result: crate::types::Task = client.send_jsonrpc("skills/invoke", params).await?;
    println!("Skill invocation task ID: {}", result.id);
    
    // Verify initial state is Submitted
    assert_eq!(result.status.state, TaskState::Submitted, "Initial state should be Submitted");
    
    // Wait a bit and check if state transitioned to Working
    time::sleep(time::Duration::from_millis(1000)).await;
    let task = client.get_task(&result.id).await?;
    println!("State after 1 second: {:?}", task.status.state);
    assert_eq!(task.status.state, TaskState::Working, "State should transition to Working");
    
    // Wait for completion
    time::sleep(time::Duration::from_millis(2500)).await;
    let task = client.get_task(&result.id).await?;
    println!("Final state: {:?}", task.status.state);
    assert_eq!(task.status.state, TaskState::Completed, "Final state should be Completed");
    
    // Verify the task has the expected artifacts
    assert!(task.artifacts.is_some() && !task.artifacts.as_ref().unwrap().is_empty(), 
            "Task should have artifacts");
    
    if let Some(artifacts) = &task.artifacts {
        if let Some(name) = &artifacts[0].name {
            assert!(name.contains("test-skill-1"), "Artifact should be from test-skill-1");
        }
    }
    
    Ok(())
}