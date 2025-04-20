use crate::client::A2aClient;
use crate::mock_server::start_mock_server;
use crate::types::TaskState;
use std::error::Error;
use std::thread;
use futures_util::StreamExt;
use crate::client::streaming::StreamingResponse;
use serde_json::json;

/// Tests the dynamic streaming content feature of the mock server
#[tokio::test]
async fn test_dynamic_streaming_content() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8099;
    thread::spawn(move || {
        start_mock_server(port);
    });
    
    // Wait for server to start
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    // Test 1: Configure text chunks
    println!("TEST 1: Configuring text chunks");
    let metadata = json!({
        "_mock_stream_text_chunks": 3,
        "_mock_stream_chunk_delay_ms": 100
    });
    
    let mut stream = client.send_task_subscribe_with_metadata(
        "Test with 3 text chunks",
        &metadata
    ).await?;
    
    println!("Stream created, waiting for updates...");
    let mut text_artifacts_count = 0;
    
    while let Some(update) = stream.next().await {
        match update {
            Ok(StreamingResponse::Artifact(artifact)) => {
                // Count only text artifacts
                for part in &artifact.parts {
                    if let crate::types::Part::TextPart(_) = part {
                        text_artifacts_count += 1;
                    }
                }
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
    
    println!("Received {} text artifacts", text_artifacts_count);
    assert_eq!(text_artifacts_count, 3, "Should have received exactly 3 text artifacts");
    
    // Test 2: Configure artifact types
    println!("\nTEST 2: Configuring artifact types");
    let metadata = json!({
        "_mock_stream_artifact_types": ["data", "file"],
        "_mock_stream_chunk_delay_ms": 100
    });
    
    let mut stream = client.send_task_subscribe_with_metadata(
        "Test with data and file artifacts only",
        &metadata
    ).await?;
    
    println!("Stream created, waiting for updates...");
    let mut data_artifacts_count = 0;
    let mut file_artifacts_count = 0;
    let mut text_artifacts_count = 0;
    
    while let Some(update) = stream.next().await {
        match update {
            Ok(StreamingResponse::Artifact(artifact)) => {
                // Count by artifact type
                for part in &artifact.parts {
                    match part {
                        crate::types::Part::TextPart(_) => text_artifacts_count += 1,
                        crate::types::Part::DataPart(_) => data_artifacts_count += 1,
                        crate::types::Part::FilePart(_) => file_artifacts_count += 1,
                    }
                }
            },
            Ok(StreamingResponse::Status(_)) => {},
            Ok(StreamingResponse::Final(_)) => break,
            Err(_) => break,
        }
    }
    
    println!("Received: {} text, {} data, {} file artifacts", 
             text_artifacts_count, data_artifacts_count, file_artifacts_count);
    assert_eq!(text_artifacts_count, 0, "Should have received no text artifacts");
    assert_eq!(data_artifacts_count, 1, "Should have received 1 data artifact");
    assert_eq!(file_artifacts_count, 1, "Should have received 1 file artifact");
    
    // Test 3: Configure final state
    println!("\nTEST 3: Configuring final state to failed");
    let metadata = json!({
        "_mock_stream_text_chunks": 1,
        "_mock_stream_final_state": "failed",
        "_mock_stream_chunk_delay_ms": 100
    });
    
    let mut stream = client.send_task_subscribe_with_metadata(
        "Test with failed final state",
        &metadata
    ).await?;
    
    println!("Stream created, waiting for final state...");
    let mut final_state = TaskState::Working;
    
    while let Some(update) = stream.next().await {
        match update {
            Ok(StreamingResponse::Final(task)) => {
                final_state = task.status.state;
                println!("Final status: {}", final_state);
                break;
            },
            Ok(StreamingResponse::Status(task)) => {
                println!("Status update: {}", task.status.state);
            },
            _ => {},
        }
    }
    
    assert_eq!(final_state, TaskState::Failed, "Final state should be Failed");
    
    // Test 4: Test resubscribe with dynamic configuration
    println!("\nTEST 4: Testing resubscribe with dynamic configuration");
    
    // First create a task to resubscribe to
    let task = client.send_task("Task for resubscription test").await?;
    let task_id = task.id.clone();
    
    // Resubscribe with configuration
    let metadata = json!({
        "_mock_stream_text_chunks": 2,
        "_mock_stream_artifact_types": ["text", "data"],
        "_mock_stream_chunk_delay_ms": 100
    });
    
    let mut stream = client.resubscribe_task_with_metadata(&task_id, &metadata).await?;
    
    println!("Resubscribe stream created, waiting for updates...");
    let mut artifacts_count = 0;
    
    while let Some(update) = stream.next().await {
        match update {
            Ok(StreamingResponse::Artifact(_)) => {
                artifacts_count += 1;
            },
            Ok(StreamingResponse::Final(_)) => break,
            _ => {},
        }
    }
    
    println!("Received {} artifacts from resubscribe", artifacts_count);
    assert_eq!(artifacts_count, 3, "Should have received 3 artifacts (2 text + 1 data)");
    
    println!("All dynamic streaming content tests passed!");
    Ok(())
}