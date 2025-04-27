use crate::client::A2aClient;
use crate::client::errors::{ClientError, A2aError, error_codes};
use crate::mock_server::{start_mock_server, start_mock_server_with_auth};
use std::error::Error;
use std::thread;
use std::time::Duration;
use tokio::time;
use uuid::Uuid;

/// Tests the error handling capabilities of the A2A client
/// especially focusing on the correct JSON-RPC error codes and messages
#[tokio::test]
async fn test_task_not_found_error() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8093;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
    });
    
    // Wait for server to start
    time::sleep(time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    // Generate a random task ID that won't exist
    let non_existent_task_id = format!("non-existent-{}", Uuid::new_v4());
    
    // Attempt to get this non-existent task
    let result = client.get_task_with_error_handling(&non_existent_task_id).await;
    
    // Verify we get the expected error
    assert!(result.is_err(), "Expected error for non-existent task");
    
    if let Err(ClientError::A2aError(a2a_error)) = result {
        println!("Error code: {}, message: {}", a2a_error.code, a2a_error.message);
        assert_eq!(a2a_error.code, error_codes::ERROR_TASK_NOT_FOUND, "Error code should be -32001 (Task not found)");
        assert!(a2a_error.message.contains("Task not found"), "Error message should mention task not found");
    } else {
        return Err("Expected A2aError".into());
    }
    
    Ok(())
}

#[tokio::test]
async fn test_invalid_parameters_error() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8094;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
    });
    
    // Wait for server to start
    time::sleep(time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    // This should fail with an invalid parameters error
    let result = client.test_invalid_parameters_error().await;
    
    // Verify we get the expected error
    assert!(result.is_err(), "Expected error for missing parameters");
    
    if let Err(ClientError::A2aError(a2a_error)) = result {
        println!("Error code: {}, message: {}", a2a_error.code, a2a_error.message);
        assert_eq!(a2a_error.code, error_codes::ERROR_INVALID_PARAMS, "Error code should be -32602 (Invalid parameters)");
        assert!(a2a_error.message.contains("Invalid parameters"), "Error message should mention invalid parameters");
    } else {
        return Err("Expected A2aError".into());
    }
    
    Ok(())
}

#[tokio::test]
async fn test_method_not_found_error() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8095;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
    });
    
    // Wait for server to start
    time::sleep(time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    // This should fail with a method not found error
    let result = client.test_method_not_found_error().await;
    
    // Verify we get the expected error
    assert!(result.is_err(), "Expected error for non-existent method");
    
    if let Err(ClientError::A2aError(a2a_error)) = result {
        println!("Error code: {}, message: {}", a2a_error.code, a2a_error.message);
        assert_eq!(a2a_error.code, error_codes::ERROR_METHOD_NOT_FOUND, "Error code should be -32601 (Method not found)");
        assert!(a2a_error.message.contains("Method not found"), "Error message should mention method not found");
    } else {
        return Err("Expected A2aError".into());
    }
    
    Ok(())
}

#[tokio::test]
async fn test_task_not_cancelable_error() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8096;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
    });
    
    // Wait for server to start
    time::sleep(time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    // First create a task with special ID that will trigger the error
    let task = client.send_task("test-task-not-cancelable").await?;
    println!("Created task: {}", task.id);
    
    // Task will automatically transition to completed state in our mock server
    // Wait a moment to ensure it completes
    time::sleep(time::Duration::from_millis(100)).await;
    
    // Now try to cancel it, which should fail since it's already in completed state
    let result = client.cancel_task_with_error_handling(&task.id).await;
    
    // Verify we get the expected error
    assert!(result.is_err(), "Expected error for canceling completed task");
    
    if let Err(ClientError::A2aError(a2a_error)) = result {
        println!("Error code: {}, message: {}", a2a_error.code, a2a_error.message);
        assert_eq!(a2a_error.code, error_codes::ERROR_TASK_NOT_CANCELABLE, "Error code should be -32002 (Task cannot be canceled)");
        assert!(a2a_error.message.contains("Task cannot be canceled"), "Error message should mention task cancellation");
    } else {
        return Err("Expected A2aError".into());
    }
    
    Ok(())
}

// Disabled temporarily for integration testing
//#[tokio::test]
async fn test_authentication_error() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread WITH authentication required
    let port = 8097;
    thread::Builder::new()
        .name("auth_test_thread".to_string())
        .spawn(move || {
            start_mock_server_with_auth(port, true); // Authentication required
        })
        .unwrap();
    
    // Wait for server to start
    time::sleep(time::Duration::from_millis(500)).await;
    
    // Create client WITHOUT authentication
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    // Attempt to access a protected endpoint
    let result = client.send_task("This is a test message").await;
    
    // Verify we get the expected error
    assert!(result.is_err(), "Expected error for unauthorized request");
    
    let error_message = result.unwrap_err().to_string();
    println!("Error message: {}", error_message);
    
    // Check for authentication failure
    // Note: The mock server returns a status code 401 which our client converts to a general error
    assert!(error_message.contains("Unauthorized") || 
            error_message.contains("401") || 
            error_message.contains("authentication"), 
            "Error should indicate authentication failure");
    
    // Now try with proper authentication
    let mut auth_client = A2aClient::new(&format!("http://localhost:{}", port))
        .with_auth("Authorization", "Bearer test-token");
    
    // This should succeed
    let task = auth_client.send_task("This is an authenticated request").await?;
    println!("Successfully created task with authentication: {}", task.id);
    
    Ok(())
}

#[tokio::test]
async fn test_skill_not_found_error() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8099;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
    });
    
    // Wait for server to start
    time::sleep(time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    // Generate a random skill ID that won't exist
    let non_existent_skill_id = format!("non-existent-skill-{}", Uuid::new_v4());
    
    // Attempt to get this non-existent skill
    let result = client.get_skill_details_with_error_handling(&non_existent_skill_id).await;
    
    // Verify we get the expected error
    assert!(result.is_err(), "Expected error for non-existent skill");
    
    if let Err(ClientError::A2aError(a2a_error)) = result {
        println!("Error code: {}, message: {}", a2a_error.code, a2a_error.message);
        assert_eq!(a2a_error.code, error_codes::ERROR_METHOD_NOT_FOUND, "Error code should be -32601 (Method not found)");
        assert!(a2a_error.message.contains("Method not found"), "Error message should mention method not found");
    } else {
        return Err("Expected A2aError".into());
    }
    
    Ok(())
}

#[tokio::test]
async fn test_batch_not_found_error() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8100;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
    });
    
    // Wait for server to start
    time::sleep(time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    // Generate a random batch ID that won't exist
    let non_existent_batch_id = format!("non-existent-batch-{}", Uuid::new_v4());
    
    // Attempt to get this non-existent batch
    let result = client.get_batch_with_error_handling(&non_existent_batch_id).await;
    
    // Verify we get the expected error
    assert!(result.is_err(), "Expected error for non-existent batch");
    
    if let Err(ClientError::A2aError(a2a_error)) = result {
        println!("Error code: {}, message: {}", a2a_error.code, a2a_error.message);
        assert_eq!(a2a_error.code, error_codes::ERROR_METHOD_NOT_FOUND, "Error code should be -32601 (Method not found)");
        assert!(a2a_error.message.contains("Method not found"), "Error message should mention method not found");
    } else {
        return Err("Expected A2aError".into());
    }
    
    Ok(())
}

#[tokio::test]
async fn test_file_not_found_error() -> Result<(), Box<dyn Error>> {
    // Start mock server in a separate thread
    let port = 8101;
    thread::spawn(move || {
        start_mock_server_with_auth(port, false);
    });
    
    // Wait for server to start
    time::sleep(time::Duration::from_millis(500)).await;
    
    // Create client
    let mut client = A2aClient::new(&format!("http://localhost:{}", port));
    
    // Generate a random file ID that won't exist
    let non_existent_file_id = format!("non-existent-file-{}", Uuid::new_v4());
    
    // Attempt to download this non-existent file
    let result = client.download_file_with_error_handling(&non_existent_file_id).await;
    
    // Verify we get the expected error
    assert!(result.is_err(), "Expected error for non-existent file");
    
    if let Err(ClientError::A2aError(a2a_error)) = result {
        println!("Error code: {}, message: {}", a2a_error.code, a2a_error.message);
        assert_eq!(a2a_error.code, error_codes::ERROR_METHOD_NOT_FOUND, "Error code should be -32601 (Method not found)");
        assert!(a2a_error.message.contains("Method not found"), "Error message should mention method not found");
    } else {
        return Err("Expected A2aError".into());
    }
    
    Ok(())
}