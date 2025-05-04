use crate::client::A2aClient;
use crate::client::errors::ClientError;
use crate::types::{AgentCard, AgentCapabilities};
use std::error::Error;
use std::sync::Arc; // Keep Arc
use std::time::Duration;
use tokio::sync::Mutex; // Use Tokio's Mutex for the client
use colored::*; // For colored output
use futures_util::StreamExt; // Add StreamExt for .next()

/// Configuration for the test runner
#[derive(Debug, Clone)]
pub struct TestRunnerConfig {
    pub target_url: Option<String>,
    // Removed run_unofficial_tests flag
    pub default_timeout: Duration,
    // Authentication details can be handled by creating an authenticated client later
}

/// Holds the aggregated results of the test run
#[derive(Default, Debug)]
struct TestResults {
    success_count: usize,
    failure_count: usize,
    skipped_count: usize,
    test_counter: usize,
}

// Use std::sync::Mutex for TestResults as it's not held across awaits in the same problematic way
// and avoids making the results struct itself async-aware unnecessarily.
type SharedResults = Arc<std::sync::Mutex<TestResults>>;


/// Main entry point for running the integration test suite
pub async fn run_integration_tests(config: TestRunnerConfig) -> Result<(), Box<dyn Error>> {
    println!("{}", "======================================================".blue().bold());
    println!("{}", "🚀 Starting A2A Integration Test Suite (Rust Runner)".blue().bold());
    println!("{}", "   (Running Official A2A Protocol Tests Only)".blue());
    println!("{}", "======================================================".blue().bold());

    let results: SharedResults = Arc::new(std::sync::Mutex::new(TestResults::default()));

    // --- Server Management ---
    let (_server_guard, server_url) = start_mock_server_if_needed(&config).await?;
    println!("🧪 Testing against server URL: {}", server_url.cyan());

    // --- Client Initialization ---
    // TODO: Handle authentication based on config or agent card if needed later
    let client = A2aClient::new(&server_url); // client is not mut initially
    let client_arc = Arc::new(Mutex::new(client)); // Use Arc<tokio::sync::Mutex<>>

    // --- Agent Card & Capabilities ---
    println!("{}", "\n--- Fetching Agent Card ---".cyan());
    let mut agent_card_opt: Option<AgentCard> = None;
    let mut capabilities = AgentCapabilities { // Default capabilities
        streaming: false,
        push_notifications: false,
        state_transition_history: false,
    };

    // Manually handle agent card fetching outside run_test to get the value
    {
        let test_num;
        {
            let mut results_guard = results.lock().unwrap();
            results_guard.test_counter += 1;
            test_num = results_guard.test_counter;
        }
        println!("{}", format!("--> [Test {}] Running: Get Agent Card...", test_num).yellow());

        let get_card_closure = || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let client_guard = client_clone.lock().await; // Remove mut
                client_guard.get_agent_card().await // Await directly
            }
        };

        match tokio::time::timeout(config.default_timeout, get_card_closure()).await {
            Ok(Ok(card)) => {
                println!("    {}", format!("✅ [Test {}] Success: Get Agent Card", test_num).green());
                results.lock().unwrap().success_count += 1;
                agent_card_opt = Some(card.clone()); // Store the card
                // Parse capabilities
                if let Some(caps) = agent_card_opt.as_ref().map(|c| &c.capabilities) {
                    capabilities = caps.clone();
                    println!("    Agent Capabilities: Streaming={}, Push={}, History={}",
                             capabilities.streaming, // Access bool directly
                             capabilities.push_notifications, // Access bool directly
                             capabilities.state_transition_history); // Access bool directly
                } else {
                    println!("    Agent Capabilities: Not specified in Agent Card.");
                }
            }
            Ok(Err(e)) => {
                println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Error: {}): Get Agent Card", test_num, e).yellow());
                results.lock().unwrap().failure_count += 1;
                println!("{}", "⚠️ Could not retrieve Agent Card. Assuming default capabilities (false).".yellow());
            }
            Err(_) => {
                println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Timeout): Get Agent Card", test_num).yellow());
                results.lock().unwrap().failure_count += 1;
                println!("{}", "⚠️ Could not retrieve Agent Card. Assuming default capabilities (false).".yellow());
            }
        }
    } // Tokio MutexGuard dropped automatically

    // --- Basic Task Operations ---
    println!("{}", "\n--- Basic Task Operations ---".cyan());
    let mut task_id: Option<String> = None;

    // Send Task (needs special handling to capture ID)
    {
        let test_num;
        {
            let mut results_guard = results.lock().unwrap();
            results_guard.test_counter += 1;
            test_num = results_guard.test_counter;
        }
        println!("{}", format!("--> [Test {}] Running: Send Task...", test_num).yellow());

        let send_task_closure = || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let mut client_guard = client_clone.lock().await; // Use async lock
                // Pass None for the session_id argument
                client_guard.send_task("This is a test task from Rust runner", None).await // Await directly
            }
        };

        match tokio::time::timeout(config.default_timeout, send_task_closure()).await {
            Ok(Ok(task)) => {
                task_id = Some(task.id.clone());
                println!("    {}", format!("✅ [Test {}] Success: Send Task (ID: {})", test_num, task.id).green());
                let mut results_guard = results.lock().unwrap();
                results_guard.success_count += 1;
            }
            Ok(Err(e)) => {
                println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Error: {}): Send Task", test_num, e).yellow());
                let mut results_guard = results.lock().unwrap();
                results_guard.failure_count += 1;
            }
            Err(_) => {
                println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Timeout): Send Task", test_num).yellow());
                let mut results_guard = results.lock().unwrap();
                results_guard.failure_count += 1;
            }
        }
    } // Tokio MutexGuard dropped automatically

    // Dependent tests
    if let Some(ref id) = task_id {
        let task_id_clone = id.clone(); // Clone for the closure
        run_test(
            &results,
            "Get Task Details",
            false,
            &config,
            || {
                let client_clone = Arc::clone(&client_arc);
                let id_c = task_id_clone.clone(); // Clone again for async move
                async move {
                    let mut client_guard = client_clone.lock().await; // Use async lock
                    client_guard.get_task(&id_c).await?; // Await directly
                    Ok(()) // Return Ok(()) on success
                }
            },
        ).await.ok(); // Ignore result for now, run_test handles logging/counting

        // Removed State History tests as they rely on non-standard client methods

        let task_id_clone_cancel = id.clone();
        run_test(
            &results,
            "Cancel Task",
            false,
            &config,
            || {
                let client_clone = Arc::clone(&client_arc);
                let id_c = task_id_clone_cancel.clone();
                async move {
                    let mut client_guard = client_clone.lock().await; // Use async lock
                    // Use a helper method that returns a ClientError
                    let params = crate::types::TaskIdParams {
                        id: id_c.to_string(),
                        metadata: None,
                    };
                    let params_value = serde_json::to_value(params)?;
                    client_guard.send_jsonrpc::<serde_json::Value>("tasks/cancel", params_value).await?;
                    Ok(())
                }
            },
        ).await.ok();

        let task_id_clone_after_cancel = id.clone();
        run_test(
            &results,
            "Get Task Details After Cancellation",
            false,
            &config,
            || {
                let client_clone = Arc::clone(&client_arc);
                let id_c = task_id_clone_after_cancel.clone();
                async move {
                    let mut client_guard = client_clone.lock().await; // Use async lock
                    client_guard.get_task(&id_c).await?; // Await directly
                    Ok(())
                }
            },
        ).await.ok();

    } else {
        println!("{}", "⚠️ Skipping tests dependent on successful task creation.".yellow());
    }

    // --- Streaming Tests (Conditional) ---
    println!("{}", "\n--- Streaming Tests ---".cyan());
    if capabilities.streaming { // Access bool directly
        // Test starting a streaming task
        // Note: Replicating the exact shell script behavior (background process + kill)
        // is complex. This test just checks if the subscribe call succeeds initially.
        run_test(
            &results,
            "Start Streaming Task",
            false,
            &config,
            || {
                let client_clone = Arc::clone(&client_arc);
                async move {
                    let mut client_guard = client_clone.lock().await; // Use async lock
                    // Create a streaming task with standard API
                    let message = client_guard.create_text_message("This is a streaming test");
                    
                    let params = crate::types::TaskSendParams {
                        id: uuid::Uuid::new_v4().to_string(),
                        message: message,
                        history_length: None,
                        metadata: None,
                        push_notification: None,
                        session_id: None,
                    };
                    
                    let mut stream = client_guard.send_streaming_request_typed(
                        "tasks/sendSubscribe", 
                        serde_json::to_value(params)?
                    ).await?;
                    
                    // Just check that we got a valid stream back
                    let _ = stream.next().await;
                    Ok(())
                }
            },
        ).await.ok();
    } else {
        println!("{}", "⏭️ Skipping Streaming tests (Agent capability not reported).".blue());
        results.lock().unwrap().skipped_count += 1; // Increment skipped count
    }

    // --- Push Notification Tests (Conditional) ---
    println!("{}", "\n--- Push Notification Tests ---".cyan());
    if capabilities.push_notifications { // Access bool directly
        if let Some(ref id) = task_id {
            let task_id_clone_set = id.clone();
            run_test(
                &results,
                "Set Push Notification",
                false,
                &config,
                || {
                    let client_clone = Arc::clone(&client_arc);
                    let id_c = task_id_clone_set.clone();
                    async move {
                        let mut client_guard = client_clone.lock().await; // Use async lock
                        // Use a direct jsonrpc call instead of helper method
                        let auth_info = crate::types::AuthenticationInfo {
                            schemes: vec!["Bearer".to_string()],
                            credentials: None,
                            extra: serde_json::Map::new(),
                        };
                        
                        let config = crate::types::PushNotificationConfig {
                            url: "https://example.com/webhook".to_string(),
                            authentication: Some(auth_info),
                            token: Some("test-token".to_string()),
                        };
                        
                        let params = crate::types::TaskPushNotificationConfig {
                            id: id_c.to_string(),
                            push_notification_config: config
                        };
                        
                        client_guard.send_jsonrpc::<serde_json::Value>(
                            "tasks/pushNotification/set", 
                            serde_json::to_value(params)?
                        ).await?;
                        Ok(())
                    }
                },
            ).await.ok();

            let task_id_clone_get = id.clone();
            run_test(
                &results,
                "Get Push Notification",
                false,
                &config,
                || {
                    let client_clone = Arc::clone(&client_arc);
                    let id_c = task_id_clone_get.clone();
                    async move {
                        let mut client_guard = client_clone.lock().await; // Use async lock
                        // Use a direct jsonrpc call instead of helper method
                        let params = crate::types::TaskIdParams {
                            id: id_c.to_string(),
                            metadata: None
                        };
                        
                        client_guard.send_jsonrpc::<serde_json::Value>(
                            "tasks/pushNotification/get", 
                            serde_json::to_value(params)?
                        ).await?;
                        Ok(())
                    }
                },
            ).await.ok();
        } else {
            println!("{}", "⚠️ Skipping Push Notification tests as TASK_ID was not set.".yellow());
        }
    } else {
        println!("{}", "⏭️ Skipping Push Notification tests (Agent capability not reported).".blue());
        results.lock().unwrap().skipped_count += 2; // Increment skipped count
    }

    // --- Task Batching Tests (Unofficial) ---
    // Removed Task Batching tests as they rely on non-standard client methods

    // --- Agent Skills Tests (Unofficial) ---
    // Removed Agent Skills tests as they rely on non-standard client methods

    // --- Error Handling Tests ---
    println!("{}", "\n--- Error Handling Tests ---".cyan());
    let non_existent_task_id = format!("task-does-not-exist-{}", uuid::Uuid::new_v4());
    // Removed non_existent_skill_id and non_existent_batch_id

    // Test non-existent task error
    let get_task_err_result = run_test(
        &results,
        "Error handling for non-existent task",
        false,
        &config,
        || {
            let client_clone = Arc::clone(&client_arc);
            let id_c = non_existent_task_id.clone();
            async move {
                let mut client_guard = client_clone.lock().await; // Use async lock
                client_guard.get_task(&id_c).await?; // Expect this to fail
                Ok(()) // Should not reach here
            }
        },
    ).await;
    // For error tests, we expect Err, so success is actually a failure of the test itself
    if get_task_err_result.is_ok() {
        println!("    {}", "❌ Test Failed: Expected an error but got Ok".red());
        results.lock().unwrap().failure_count += 1;
        results.lock().unwrap().success_count -= 1; // Correct count from run_test
    } else if let Err(ClientError::A2aError(ref e)) = get_task_err_result {
         if !e.is_task_not_found() {
             println!("    {}", format!("❌ Test Failed: Expected TaskNotFound error but got code {}", e.code).red());
             // Already marked as failure by run_test
         } else {
              println!("    {}", "✅ Got expected TaskNotFound error".green());
             // Correct the count: run_test marked failure, but this is expected success
             results.lock().unwrap().failure_count -= 1;
             results.lock().unwrap().success_count += 1;
         }
    }

    // Removed non-existent skill error test
    // Removed non-existent batch error test

    // --- File Operations Tests (Unofficial) ---
    // Removed File Operations tests

    // --- Authentication Validation Test (Unofficial) ---
    // Removed Authentication Validation test

    // --- Configurable Delays Tests (Unofficial) ---
    // Removed Configurable Delays tests

    // --- Dynamic Streaming Content Tests (Unofficial) ---
    // Removed Dynamic Streaming Content tests


    // --- Test Summary ---
    println!("{}", "\n======================================================".blue().bold());
    println!("{}", "📊 Test Summary".blue().bold());
    println!("{}", "======================================================".blue().bold());
    {
        let final_results = results.lock().unwrap();
        println!("Total Tests Attempted: {}", final_results.test_counter.to_string().bold());
        println!("{} {}", "Successful Tests:".green(), final_results.success_count.to_string().bold().green());
        println!("{} {}", "Unsupported/Not Working:".yellow(), final_results.failure_count.to_string().bold().yellow());
        println!("{} {}", "Skipped Tests:".blue(), final_results.skipped_count.to_string().bold().blue());
        // Removed run_unofficial_tests flag check
    }
    println!("{}", "======================================================".blue().bold());

    // Determine final exit status
    let final_results = results.lock().unwrap();
    if final_results.failure_count > 0 {
        println!("{}", "Some tests were unsupported or not working!".yellow().bold());
        // Return an error to indicate failure
        Err("Test suite finished with failures.".into())
    } else {
        println!("{}", "All attempted tests passed!".green().bold());
        Ok(())
    }
}

// --- Mock Server Management ---

use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use crate::mock_server;

// Use a global flag to track if the server is running
static SERVER_RUNNING: AtomicBool = AtomicBool::new(false);

/// RAII guard to manage the mock server's lifetime
struct MockServerGuard {
    // There's no process to kill, just a flag to track
    _private: (),
}

impl Drop for MockServerGuard {
    fn drop(&mut self) {
        // We can't actually stop the server since it runs in a blocking way,
        // but we can signal that it's no longer needed
        SERVER_RUNNING.store(false, Ordering::SeqCst);
        println!("{}", "🛑 Local mock server will exit when tests complete.".yellow());
    }
}

/// Starts the local mock server if no target URL is provided in the config.
/// Returns a guard to manage the server tracking and the URL to test against.
async fn start_mock_server_if_needed(config: &TestRunnerConfig) -> Result<(MockServerGuard, String), Box<dyn Error>> {
    if let Some(url) = &config.target_url {
        // Use provided URL, no server to manage
        Ok((MockServerGuard { _private: () }, url.clone()))
    } else {
        // Start local mock server
        let port = 8080; // Fixed port for now
        let url = format!("http://localhost:{}", port);
        println!("{} {}", "🚀 Starting local mock server on port".blue().bold(), port.to_string().cyan());

        // Only start the server if it's not already running
        if !SERVER_RUNNING.load(Ordering::SeqCst) {
            SERVER_RUNNING.store(true, Ordering::SeqCst);
            
            // Start the mock server in a dedicated thread
            thread::spawn(move || {
                // Start the mock server (this is a blocking call)
                mock_server::start_mock_server(port);
            });
            
            // Wait a moment for the server to start
            println!("{}", "⏳ Waiting for local server to start...".yellow());
            tokio::time::sleep(Duration::from_secs(2)).await;
        } else {
            println!("{}", "✅ Using existing local mock server.".green());
        }
        
        println!("{}", "✅ Local mock server is ready.".green());
        Ok((MockServerGuard { _private: () }, url))
    }
}

// --- Test Execution Helper ---

/// Helper function to run a single test step with timeout and result tracking.
async fn run_test<F, Fut>(
    results: &SharedResults, // Use the type alias
    description: &str,
    _is_unofficial: bool, // Keep signature but ignore the flag
    config: &TestRunnerConfig, // Pass config
    test_logic: F,
) -> Result<(), ClientError>
where
    F: FnOnce() -> Fut + Send, // Ensure closure is Send
    Fut: std::future::Future<Output = Result<(), ClientError>> + Send, // Ensure future is Send
{
    let test_num;
    {
        let mut results_guard = results.lock().unwrap();
        results_guard.test_counter += 1;
        test_num = results_guard.test_counter;
    } // Mutex guard is dropped here

    // Removed check for unofficial tests

    println!(
        "{}",
        format!("--> [Test {}] Running: {}...", test_num, description).yellow()
    );

    // Execute the test logic with a timeout
    match tokio::time::timeout(config.default_timeout, test_logic()).await {
        Ok(Ok(_)) => {
            // Test logic succeeded
            println!(
                "    {}",
                format!("✅ [Test {}] Success: {}", test_num, description).green()
            );
            let mut results_guard = results.lock().unwrap();
            results_guard.success_count += 1;
            Ok(())
        }
        Ok(Err(e)) => {
            // Test logic failed with a ClientError
            println!(
                "    {}",
                format!(
                    "⚠️ [Test {}] Unsupported or not working (Error: {}): {}",
                    test_num, e, description
                )
                .yellow()
            );
            let mut results_guard = results.lock().unwrap();
            results_guard.failure_count += 1;
            Err(e) // Propagate the error if needed, but the test run continues
        }
        Err(_) => {
            // Timeout occurred
            println!(
                "    {}",
                format!(
                    "⚠️ [Test {}] Unsupported or not working (Timeout): {}",
                    test_num, description
                )
                .yellow()
            );
            let mut results_guard = results.lock().unwrap();
            results_guard.failure_count += 1;
            Err(ClientError::Other(format!("Test timed out: {}", description)))
        }
    }
}
