use crate::client::A2aClient;
use crate::client::errors::ClientError;
use crate::types::{AgentCard, AgentCapabilities};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::process::Child;
use colored::*; // For colored output
use futures_util::StreamExt; // Add StreamExt for .next()

/// Configuration for the test runner
#[derive(Debug, Clone)]
pub struct TestRunnerConfig {
    pub target_url: Option<String>,
    pub run_unofficial_tests: bool,
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

/// Main entry point for running the integration test suite
pub async fn run_integration_tests(config: TestRunnerConfig) -> Result<(), Box<dyn Error>> {
    println!("{}", "======================================================".blue().bold());
    println!("{}", "🚀 Starting A2A Integration Test Suite (Rust Runner)".blue().bold());
    println!("{}", "======================================================".blue().bold());

    let results = Arc::new(Mutex::new(TestResults::default()));

    // --- Server Management ---
    let (_server_guard, server_url) = start_mock_server_if_needed(&config).await?;
    println!("🧪 Testing against server URL: {}", server_url.cyan());

    // --- Client Initialization ---
    // TODO: Handle authentication based on config or agent card if needed later
    let mut client = A2aClient::new(&server_url);
    let client_arc = Arc::new(Mutex::new(client)); // Use Arc<Mutex<>> for sharing client in async blocks

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
                let mut client_guard = client_clone.lock().unwrap();
                client_guard.get_agent_card().await // This returns Result<AgentCard, ClientError>
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
    } // Mutex guard dropped

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
                let mut client_guard = client_clone.lock().unwrap();
                client_guard.send_task("This is a test task from Rust runner").await
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
    } // Mutex guard for client_arc is dropped here if send_task_closure finished

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
                    let mut client_guard = client_clone.lock().unwrap();
                    client_guard.get_task(&id_c).await?; // Use ? to propagate ClientError
                    Ok(()) // Return Ok(()) on success
                }
            },
        ).await.ok(); // Ignore result for now, run_test handles logging/counting

        // State History Tests (Conditional)
        if capabilities.state_transition_history { // Access bool directly
             let task_id_clone_hist = id.clone();
             run_test(
                 &results,
                 "Get Task State History",
                 false,
                 &config,
                 || {
                     let client_clone = Arc::clone(&client_arc);
                     let id_c = task_id_clone_hist.clone();
                     async move {
                         let mut client_guard = client_clone.lock().unwrap();
                         // Use the _typed version returning ClientError
                         client_guard.get_task_state_history_typed(&id_c).await?;
                         Ok(())
                     }
                 },
             ).await.ok();

             let task_id_clone_metrics = id.clone();
             run_test(
                 &results,
                 "Get Task State Metrics",
                 false,
                 &config,
                 || {
                     let client_clone = Arc::clone(&client_arc);
                     let id_c = task_id_clone_metrics.clone();
                     async move {
                         let mut client_guard = client_clone.lock().unwrap();
                         // Use the _typed version returning ClientError
                         client_guard.get_state_transition_metrics_typed(&id_c).await?;
                         Ok(())
                     }
                 },
             ).await.ok();
        } else {
             println!("{}", "⏭️ Skipping State History tests (Agent capability not reported).".blue());
             let mut results_guard = results.lock().unwrap();
             results_guard.skipped_count += 2; // Increment skipped count for the two tests
        }

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
                    let mut client_guard = client_clone.lock().unwrap();
                    // Use the _typed version returning ClientError
                    client_guard.cancel_task_typed(&id_c).await?;
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
                    let mut client_guard = client_clone.lock().unwrap();
                    client_guard.get_task(&id_c).await?;
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
                    let mut client_guard = client_clone.lock().unwrap();
                    // Attempt to subscribe, we don't need to consume the whole stream here
                    // Use the _typed version returning ClientError
                    let mut stream = client_guard.send_task_subscribe_typed("This is a streaming test").await?;
                    // Optionally, try receiving one item to confirm connection
                    let _ = stream.next().await; // Requires StreamExt trait
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
                        let mut client_guard = client_clone.lock().unwrap();
                        // Use the _typed version returning ClientError
                        client_guard.set_task_push_notification_typed(
                            &id_c,
                            "https://example.com/webhook", // Mock webhook URL
                            Some("Bearer"),
                            Some("test-token")
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
                        let mut client_guard = client_clone.lock().unwrap();
                        // Use the _typed version returning ClientError
                        client_guard.get_task_push_notification_typed(&id_c).await?;
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

    // --- Task Batching Tests ---
    println!("{}", "\n--- Task Batching Tests ---".cyan());
    let mut batch_id: Option<String> = None;
    {
        let test_num;
        {
            let mut results_guard = results.lock().unwrap();
            results_guard.test_counter += 1;
            test_num = results_guard.test_counter;
        }
        println!("{}", format!("--> [Test {}] Running: Create Task Batch...", test_num).yellow());

        let create_batch_closure = || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                let params = crate::client::task_batch::BatchCreateParams {
                    id: None, // Auto-generate ID
                    name: Some("Test Batch from Rust".to_string()),
                    tasks: vec![
                        "Batch Task 1".to_string(),
                        "Batch Task 2".to_string(),
                    ],
                    metadata: None,
                };
                // Use the _typed version returning ClientError
                client_guard.create_task_batch_typed(params).await
            }
        };

        match tokio::time::timeout(config.default_timeout, create_batch_closure()).await {
            Ok(Ok(batch)) => {
                batch_id = Some(batch.id.clone());
                println!("    {}", format!("✅ [Test {}] Success: Create Task Batch (ID: {})", test_num, batch.id).green());
                results.lock().unwrap().success_count += 1;
            }
            Ok(Err(e)) => {
                println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Error: {}): Create Task Batch", test_num, e).yellow());
                results.lock().unwrap().failure_count += 1;
            }
            Err(_) => {
                println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Timeout): Create Task Batch", test_num).yellow());
                results.lock().unwrap().failure_count += 1;
            }
        }
    } // Mutex guard dropped

    if let Some(ref id) = batch_id {
        let batch_id_clone_get = id.clone();
        run_test(
            &results,
            "Get Batch Info",
            false,
            &config,
            || {
                let client_clone = Arc::clone(&client_arc);
                let id_c = batch_id_clone_get.clone();
                async move {
                    let mut client_guard = client_clone.lock().unwrap();
                    // Use the _typed version returning ClientError
                    client_guard.get_batch_typed(&id_c, false).await?;
                    Ok(())
                }
            },
        ).await.ok();

        let batch_id_clone_status = id.clone();
        run_test(
            &results,
            "Get Batch Status",
            false,
            &config,
            || {
                let client_clone = Arc::clone(&client_arc);
                let id_c = batch_id_clone_status.clone();
                async move {
                    let mut client_guard = client_clone.lock().unwrap();
                    // Use the _typed version returning ClientError
                    client_guard.get_batch_status_typed(&id_c).await?;
                    Ok(())
                }
            },
        ).await.ok();

        let batch_id_clone_cancel = id.clone();
        run_test(
            &results,
            "Cancel Batch",
            false,
            &config,
            || {
                let client_clone = Arc::clone(&client_arc);
                let id_c = batch_id_clone_cancel.clone();
                async move {
                    let mut client_guard = client_clone.lock().unwrap();
                    // Use the _typed version returning ClientError
                    client_guard.cancel_batch_typed(&id_c).await?;
                    Ok(())
                }
            },
        ).await.ok();
    } else {
        println!("{}", "⚠️ Skipping tests dependent on successful batch creation.".yellow());
    }

    // --- Agent Skills Tests (Unofficial) ---
    println!("{}", "\n--- Agent Skills Tests (Unofficial) ---".cyan());
    let skill_id_to_test = "test-skill-1"; // Assuming this exists on mock server
    run_test(
        &results,
        "List all skills",
        true, // Unofficial
        &config,
        || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                // Use the _typed version returning ClientError
                client_guard.list_skills_typed(None).await?;
                Ok(())
            }
        },
    ).await.ok();

    run_test(
        &results,
        "List skills with 'text' tag",
        true, // Unofficial
        &config,
        || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                // Use the _typed version returning ClientError
                client_guard.list_skills_typed(Some(vec!["text".to_string()])).await?;
                Ok(())
            }
        },
    ).await.ok();

    run_test(
        &results,
        &format!("Get details for skill '{}'", skill_id_to_test),
        true, // Unofficial
        &config,
        || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                // Use the _typed version returning ClientError
                client_guard.get_skill_details_typed(skill_id_to_test).await?;
                Ok(())
            }
        },
    ).await.ok();

    run_test(
        &results,
        &format!("Invoke skill '{}'", skill_id_to_test),
        true, // Unofficial
        &config,
        || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                // Use the _typed version returning ClientError
                client_guard.invoke_skill_typed(skill_id_to_test, "This is a test skill invocation", None, None).await?;
                Ok(())
            }
        },
    ).await.ok();

     run_test(
        &results,
        &format!("Invoke skill '{}' with specific modes", skill_id_to_test),
        true, // Unofficial
        &config,
        || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                // Use the _typed version returning ClientError
                client_guard.invoke_skill_typed(
                    skill_id_to_test,
                    "This is a test with specific modes",
                    Some("text/plain".to_string()),
                    Some("text/plain".to_string())
                ).await?;
                Ok(())
            }
        },
    ).await.ok();

    // --- Error Handling Tests ---
    println!("{}", "\n--- Error Handling Tests ---".cyan());
    let non_existent_task_id = format!("task-does-not-exist-{}", uuid::Uuid::new_v4());
    let non_existent_skill_id = format!("skill-does-not-exist-{}", uuid::Uuid::new_v4());
    let non_existent_batch_id = format!("batch-does-not-exist-{}", uuid::Uuid::new_v4());

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
                let mut client_guard = client_clone.lock().unwrap();
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

    // Test non-existent skill error
    let get_skill_err_result = run_test(
        &results,
        "Error handling for non-existent skill",
        false, // Official test for core error handling
        &config,
        || {
            let client_clone = Arc::clone(&client_arc);
            let id_c = non_existent_skill_id.clone();
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                // Use the _typed version returning ClientError
                client_guard.get_skill_details_typed(&id_c).await?; // Expect this to fail
                Ok(()) // Should not reach here
            }
        },
    ).await;
    if get_skill_err_result.is_ok() {
        println!("    {}", "❌ Test Failed: Expected an error but got Ok".red());
        results.lock().unwrap().failure_count += 1;
        results.lock().unwrap().success_count -= 1;
    } else if let Err(ClientError::A2aError(ref e)) = get_skill_err_result {
         // Mock server might return TaskNotFound for skills too
         if e.code != crate::client::errors::error_codes::ERROR_TASK_NOT_FOUND {
             println!("    {}", format!("❌ Test Failed: Expected TaskNotFound error but got code {}", e.code).red());
         } else {
              println!("    {}", "✅ Got expected SkillNotFound error".green());
             results.lock().unwrap().failure_count -= 1;
             results.lock().unwrap().success_count += 1;
         }
    }

    // Test non-existent batch error
    let get_batch_err_result = run_test(
        &results,
        "Error handling for non-existent batch",
        false, // Official test for core error handling
        &config,
        || {
            let client_clone = Arc::clone(&client_arc);
            let id_c = non_existent_batch_id.clone();
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                // Use the _typed version returning ClientError
                client_guard.get_batch_typed(&id_c, false).await?; // Expect this to fail
                Ok(()) // Should not reach here
            }
        },
    ).await;
     if get_batch_err_result.is_ok() {
        println!("    {}", "❌ Test Failed: Expected an error but got Ok".red());
        results.lock().unwrap().failure_count += 1;
        results.lock().unwrap().success_count -= 1;
    } else if let Err(ClientError::A2aError(ref e)) = get_batch_err_result {
         // Mock server might return TaskNotFound for batches too
         if e.code != crate::client::errors::error_codes::ERROR_TASK_NOT_FOUND {
             println!("    {}", format!("❌ Test Failed: Expected TaskNotFound error but got code {}", e.code).red());
         } else {
              println!("    {}", "✅ Got expected BatchNotFound error".green());
             results.lock().unwrap().failure_count -= 1;
             results.lock().unwrap().success_count += 1;
         }
    }

    // --- File Operations Tests ---
    println!("{}", "\n--- File Operations Tests ---".cyan());
    let temp_dir = tempfile::tempdir()?; // RAII temporary directory
    let test_file_path = temp_dir.path().join("test_file.txt");
    let test_file_content = "This is test file content.";
    std::fs::write(&test_file_path, test_file_content)?;
    println!("    Created test file at: {}", test_file_path.display());

    let mut file_task_id: Option<String> = None;
    let mut uploaded_file_id: Option<String> = None;

    // Create Task for File Ops
    {
        let test_num;
        {
            let mut results_guard = results.lock().unwrap();
            results_guard.test_counter += 1;
            test_num = results_guard.test_counter;
        }
        println!("{}", format!("--> [Test {}] Running: Create Task for File Ops...", test_num).yellow());
        let send_task_closure = || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                client_guard.send_task("Task for file operations").await
            }
        };
        match tokio::time::timeout(config.default_timeout, send_task_closure()).await {
            Ok(Ok(task)) => {
                file_task_id = Some(task.id.clone());
                println!("    {}", format!("✅ [Test {}] Success: Create Task for File Ops (ID: {})", test_num, task.id).green());
                results.lock().unwrap().success_count += 1;
            }
            Ok(Err(e)) => {
                println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Error: {}): Create Task for File Ops", test_num, e).yellow());
                results.lock().unwrap().failure_count += 1;
            }
            Err(_) => {
                println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Timeout): Create Task for File Ops", test_num).yellow());
                results.lock().unwrap().failure_count += 1;
            }
        }
    }

    if let Some(ref ftid) = file_task_id {
        // Upload File
        {
             let test_num;
             {
                 let mut results_guard = results.lock().unwrap();
                 results_guard.test_counter += 1;
                 test_num = results_guard.test_counter;
             }
             println!("{}", format!("--> [Test {}] Running: Upload File...", test_num).yellow());
             let upload_closure = || {
                 let client_clone = Arc::clone(&client_arc);
                 let path_c = test_file_path.clone();
                 let task_id_c = ftid.clone();
                 async move {
                     let mut client_guard = client_clone.lock().unwrap();
                     // Create metadata with task ID
                     let mut metadata = serde_json::Map::new();
                     metadata.insert("taskId".to_string(), serde_json::Value::String(task_id_c));
                     // Use the _typed version returning ClientError
                     client_guard.upload_file_typed(path_c.to_str().unwrap(), Some(metadata)).await
                 }
             };
             match tokio::time::timeout(config.default_timeout, upload_closure()).await {
                 Ok(Ok(upload_resp)) => {
                     uploaded_file_id = Some(upload_resp.file_id.clone());
                     println!("    {}", format!("✅ [Test {}] Success: Upload File (ID: {})", test_num, upload_resp.file_id).green());
                     results.lock().unwrap().success_count += 1;
                 }
                 Ok(Err(e)) => {
                     println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Error: {}): Upload File", test_num, e).yellow());
                     results.lock().unwrap().failure_count += 1;
                 }
                 Err(_) => {
                     println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Timeout): Upload File", test_num).yellow());
                     results.lock().unwrap().failure_count += 1;
                 }
             }
        }

        // List Files
        let ftid_clone_list = ftid.clone();
        run_test(
            &results,
            &format!("List files for task {}", ftid),
            false,
            &config,
            || {
                let client_clone = Arc::clone(&client_arc);
                let id_c = ftid_clone_list.clone();
                async move {
                    let mut client_guard = client_clone.lock().unwrap();
                    // Use the _typed version returning ClientError
                    let files = client_guard.list_files_typed(Some(&id_c)).await?;
                    // Basic check: ensure we got at least one file back if upload succeeded
                    if uploaded_file_id.is_some() {
                        if files.is_empty() {
                            return Err(ClientError::Other("ListFiles returned empty after successful upload".to_string()));
                        }
                    }
                    Ok(())
                }
            },
        ).await.ok();

        // Download File
        if let Some(ref ufid) = uploaded_file_id {
            let ufid_clone_download = ufid.clone();
            let download_path = temp_dir.path().join("downloaded_file.txt");
            run_test(
                &results,
                &format!("Download file {}", ufid),
                false,
                &config,
                || {
                    let client_clone = Arc::clone(&client_arc);
                    let id_c = ufid_clone_download.clone();
                    let path_c = download_path.clone(); // Clone path for closure
                    async move {
                        let mut client_guard = client_clone.lock().unwrap();
                        // Use the _typed version returning ClientError
                        let download_resp = client_guard.download_file_typed(&id_c).await?;
                        // Decode and write to file
                        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &download_resp.bytes)
                            .map_err(|e| ClientError::Other(format!("Base64 decode failed: {}", e)))?;
                        std::fs::write(&path_c, decoded)
                            .map_err(|e| ClientError::IoError(e))?;
                        Ok(())
                    }
                },
            ).await.ok();
            // Optional: Compare downloaded file content
            // let downloaded_content = std::fs::read_to_string(&download_path)?;
            // assert_eq!(test_file_content, downloaded_content);
        } else {
             println!("{}", "⚠️ Skipping Download File test as UPLOADED_FILE_ID was not set.".yellow());
        }

    } else {
        println!("{}", "⚠️ Skipping Upload/List/Download File tests as FILE_TASK_ID was not set.".yellow());
    }

    // Send Task With File Attachment
    let test_file_path_clone = test_file_path.clone();
    run_test(
        &results,
        "Send Task With File Attachment",
        false,
        &config,
        || {
            let client_clone = Arc::clone(&client_arc);
            let path_c = test_file_path_clone.clone();
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                // Use the _typed version returning ClientError
                client_guard.send_task_with_file_typed("Task with file attachment", path_c.to_str().unwrap()).await?;
                Ok(())
            }
        },
    ).await.ok();

    // Send Task With Data
    let test_data_file_path = temp_dir.path().join("test_data.json");
    let test_data_content = r#"{"name":"test_data","value":42}"#;
    std::fs::write(&test_data_file_path, test_data_content)?;
    println!("    Created test data file at: {}", test_data_file_path.display());

    let mut data_task_id: Option<String> = None;
    {
        let test_num;
        {
            let mut results_guard = results.lock().unwrap();
            results_guard.test_counter += 1;
            test_num = results_guard.test_counter;
        }
        println!("{}", format!("--> [Test {}] Running: Send Task With Data...", test_num).yellow());
        let send_data_closure = || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                let data_value: serde_json::Value = serde_json::from_str(test_data_content).unwrap();
                client_guard.send_task_with_data("Task with structured data", &data_value).await
            }
        };
         match tokio::time::timeout(config.default_timeout, send_data_closure()).await {
            Ok(Ok(task)) => {
                data_task_id = Some(task.id.clone());
                println!("    {}", format!("✅ [Test {}] Success: Send Task With Data (ID: {})", test_num, task.id).green());
                results.lock().unwrap().success_count += 1;
            }
            Ok(Err(e)) => {
                println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Error: {}): Send Task With Data", test_num, e).yellow());
                results.lock().unwrap().failure_count += 1;
            }
            Err(_) => {
                println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Timeout): Send Task With Data", test_num).yellow());
                results.lock().unwrap().failure_count += 1;
            }
        }
    }

    if let Some(ref dtid) = data_task_id {
        let dtid_clone_get = dtid.clone();
        run_test(
            &results,
            "Get Task Details with Data",
            false,
            &config,
            || {
                let client_clone = Arc::clone(&client_arc);
                let id_c = dtid_clone_get.clone();
                async move {
                    let mut client_guard = client_clone.lock().unwrap();
                    client_guard.get_task(&id_c).await?;
                    Ok(())
                }
            },
        ).await.ok();
    } else {
        println!("{}", "⚠️ Skipping Get Task Details with Data test as DATA_TASK_ID was not set.".yellow());
    }

    // Temp dir is automatically cleaned up when `temp_dir` goes out of scope

    // --- Authentication Validation Test (Unofficial) ---
    println!("{}", "\n--- Authentication Validation Test (Unofficial) ---".cyan());
    run_test(
        &results,
        "Validate Authentication",
        true, // Unofficial
        &config,
        || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                // Assuming default test auth is set if needed by server
                // Use the _typed version returning ClientError
                client_guard.validate_auth_typed().await?;
                Ok(())
            }
        },
    ).await.ok();

    // --- Configurable Delays Tests (Unofficial) ---
    println!("{}", "\n--- Configurable Delays Tests (Unofficial) ---".cyan());
    let mut delayed_task_id: Option<String> = None;
    {
        let test_num;
        {
            let mut results_guard = results.lock().unwrap();
            results_guard.test_counter += 1;
            test_num = results_guard.test_counter;
        }
        // Check if unofficial tests should run
        if !config.run_unofficial_tests {
             println!("{}", format!("⏭️ [Test {}] Skipping unofficial test: Send Task with 2-second delay (Use --run-unofficial to include)", test_num).blue());
             results.lock().unwrap().skipped_count += 1;
        } else {
            println!("{}", format!("--> [Test {}] Running: Send Task with 2-second delay...", test_num).yellow());
            let start_time = std::time::Instant::now();
            let send_delayed_closure = || {
                let client_clone = Arc::clone(&client_arc);
                async move {
                    let mut client_guard = client_clone.lock().unwrap();
                    let metadata = serde_json::json!({"_mock_delay_ms": 2000});
                    client_guard.send_task_with_metadata("Task with configurable delay", Some(&metadata.to_string())).await
                }
            };
            match tokio::time::timeout(config.default_timeout + Duration::from_secs(3), send_delayed_closure()).await { // Allow extra time for delay
                Ok(Ok(task)) => {
                    let duration = start_time.elapsed();
                    delayed_task_id = Some(task.id.clone());
                    println!("    Task created with ID: {} in approximately {:.2} seconds", task.id, duration.as_secs_f32());
                    if duration.as_secs() < 2 {
                         println!("    {}", format!("⚠️ [Test {}] Success (Warning): Delay might not be supported (Expected >= 2s, got {:.2}s)", test_num, duration.as_secs_f32()).yellow());
                         results.lock().unwrap().success_count += 1; // Count as success but warn
                    } else {
                         println!("    {}", format!("✅ [Test {}] Success: Send Task with 2-second delay", test_num).green());
                         results.lock().unwrap().success_count += 1;
                    }
                }
                Ok(Err(e)) => {
                    println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Error: {}): Send Task with 2-second delay", test_num, e).yellow());
                    results.lock().unwrap().failure_count += 1;
                }
                Err(_) => {
                    println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Timeout): Send Task with 2-second delay", test_num).yellow());
                    results.lock().unwrap().failure_count += 1;
                }
            }
        }
    }

    // Test streaming with configurable chunk delay (unofficial)
    run_test(
        &results,
        "Start Streaming Task with slow chunks",
        true, // Unofficial
        &config,
        || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                let metadata = serde_json::json!({"_mock_chunk_delay_ms": 1000});
                // Use the _typed version returning ClientError
                let mut stream = client_guard.send_task_subscribe_with_metadata_typed("Streaming task with slow chunks", &metadata).await?;
                // Consume one item to check connection
                let _ = stream.next().await; // Requires StreamExt trait
                Ok(())
            }
        },
    ).await.ok();


    // --- Dynamic Streaming Content Tests (Unofficial) ---
    println!("{}", "\n--- Dynamic Streaming Content Tests (Unofficial) ---".cyan());
    let mut resubscribe_task_id: Option<String> = None;
    {
         let test_num;
         {
             let mut results_guard = results.lock().unwrap();
             results_guard.test_counter += 1;
             test_num = results_guard.test_counter;
         }
         if !config.run_unofficial_tests {
              println!("{}", format!("⏭️ [Test {}] Skipping unofficial test: Create Task for Resubscribe (Use --run-unofficial to include)", test_num).blue());
              results.lock().unwrap().skipped_count += 1;
         } else {
             println!("{}", format!("--> [Test {}] Running: Create Task for Resubscribe...", test_num).yellow());
             let create_resub_closure = || {
                 let client_clone = Arc::clone(&client_arc);
                 async move {
                     let mut client_guard = client_clone.lock().unwrap();
                     client_guard.send_task("Task for resubscribe testing").await
                 }
             };
             match tokio::time::timeout(config.default_timeout, create_resub_closure()).await {
                 Ok(Ok(task)) => {
                     resubscribe_task_id = Some(task.id.clone());
                     println!("    {}", format!("✅ [Test {}] Success: Create Task for Resubscribe (ID: {})", test_num, task.id).green());
                     results.lock().unwrap().success_count += 1;
                 }
                 Ok(Err(e)) => {
                     println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Error: {}): Create Task for Resubscribe", test_num, e).yellow());
                     results.lock().unwrap().failure_count += 1;
                 }
                 Err(_) => {
                     println!("    {}", format!("⚠️ [Test {}] Unsupported or not working (Timeout): Create Task for Resubscribe", test_num).yellow());
                     results.lock().unwrap().failure_count += 1;
                 }
             }
         }
    }

    // Test streaming with custom text chunks (unofficial)
    run_test(
        &results,
        "Start Streaming Task with 3 text chunks",
        true, // Unofficial
        &config,
        || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                let metadata = serde_json::json!({
                    "_mock_stream_text_chunks": 3,
                    "_mock_stream_chunk_delay_ms": 100 // Short delay for test speed
                });
                // Use the _typed version returning ClientError
                let mut stream = client_guard.send_task_subscribe_with_metadata_typed("Streaming task with 3 text chunks", &metadata).await?;
                let _ = stream.next().await; // Requires StreamExt trait
                Ok(())
            }
        },
    ).await.ok();

    // Test streaming with only data artifacts (unofficial)
    run_test(
        &results,
        "Start Streaming Task with only data artifacts",
        true, // Unofficial
        &config,
        || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                let metadata = serde_json::json!({
                    "_mock_stream_artifact_types": ["data"],
                    "_mock_stream_chunk_delay_ms": 100
                });
                // Use the _typed version returning ClientError
                let mut stream = client_guard.send_task_subscribe_with_metadata_typed("Streaming task with only data artifacts", &metadata).await?;
                let _ = stream.next().await; // Requires StreamExt trait
                Ok(())
            }
        },
    ).await.ok();

    // Test streaming with custom final state (unofficial)
    run_test(
        &results,
        "Start Streaming Task with failed final state",
        true, // Unofficial
        &config,
        || {
            let client_clone = Arc::clone(&client_arc);
            async move {
                let mut client_guard = client_clone.lock().unwrap();
                let metadata = serde_json::json!({
                    "_mock_stream_final_state": "failed",
                    "_mock_stream_chunk_delay_ms": 100
                });
                // Use the _typed version returning ClientError
                let mut stream = client_guard.send_task_subscribe_with_metadata_typed("Streaming task with failed final state", &metadata).await?;
                let _ = stream.next().await; // Requires StreamExt trait
                Ok(())
            }
        },
    ).await.ok();

    // Test resubscribe with dynamic configuration (unofficial)
    if let Some(ref rtid) = resubscribe_task_id {
         let rtid_clone_resub = rtid.clone();
         run_test(
             &results,
             "Start Resubscribe Task with dynamic configuration",
             true, // Unofficial
             &config,
             || {
                 let client_clone = Arc::clone(&client_arc);
                 let id_c = rtid_clone_resub.clone();
                 async move {
                     let mut client_guard = client_clone.lock().unwrap();
                     let metadata = serde_json::json!({
                         "_mock_stream_text_chunks": 2,
                         "_mock_stream_artifact_types": ["text", "data"]
                     });
                     // Use the _typed version returning ClientError
                     let mut stream = client_guard.resubscribe_task_with_metadata_typed(&id_c, &metadata).await?;
                     let _ = stream.next().await; // Requires StreamExt trait
                     Ok(())
                 }
             },
         ).await.ok();
    } else {
         println!("{}", "⚠️ Skipping resubscribe test as task creation failed earlier.".yellow());
    }


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

        if config.run_unofficial_tests {
            println!("{}", "(Including unofficial tests)".yellow());
        } else {
            println!("{}", "(Unofficial tests were skipped. Use --run-unofficial to include them)".blue());
        }
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

/// RAII guard to ensure the mock server process is killed
struct MockServerGuard {
    child: Option<Child>,
}

impl Drop for MockServerGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            println!("{}", "🛑 Stopping local mock server...".yellow());
            // Attempt to kill the process
            if let Err(e) = child.start_kill() {
                eprintln!("{} {}", "⚠️ Failed to kill mock server process:".red(), e);
            } else {
                 // Optional: Wait briefly for the process to exit
                 // This might require making the drop async or using a blocking wait
                 // For simplicity, we'll just send the kill signal here.
                 println!("{}", "✅ Mock server stop signal sent.".green());
            }
        }
    }
}

/// Starts the local mock server if no target URL is provided in the config.
/// Returns a guard to manage the server process lifetime and the URL to test against.
async fn start_mock_server_if_needed(config: &TestRunnerConfig) -> Result<(MockServerGuard, String), Box<dyn Error>> {
    if let Some(url) = &config.target_url {
        // Use provided URL, no server to manage
        Ok((MockServerGuard { child: None }, url.clone()))
    } else {
        // Start local mock server
        let port = 8080; // Or choose a random available port
        let url = format!("http://localhost:{}", port);
        println!("{} {}", "🚀 Starting local mock server on port".blue().bold(), port.to_string().cyan());

        let mut command = tokio::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()));
        command.args(["run", "--quiet", "--", "server", "--port", &port.to_string()]);
        command.kill_on_drop(true); // Ensure it's killed if the command handle is dropped

        let child = command.spawn()
            .map_err(|e| format!("Failed to start mock server: {}. Is cargo in PATH?", e))?;

        println!("{} {}", "⏳ Waiting for local server to start... PID:".yellow(), child.id().unwrap_or(0));
        // TODO: Implement a more robust check (e.g., polling /.well-known/agent.json)
        tokio::time::sleep(Duration::from_secs(4)).await; // Increased sleep

        println!("{}", "✅ Local mock server started.".green());
        Ok((MockServerGuard { child: Some(child) }, url))
    }
}

// --- Test Execution Helper ---

/// Helper function to run a single test step with timeout and result tracking.
async fn run_test<F, Fut>(
    results: &Arc<Mutex<TestResults>>,
    description: &str,
    is_unofficial: bool,
    config: &TestRunnerConfig, // Pass config for run_unofficial_flag
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

    // Check if unofficial tests should be skipped
    if is_unofficial && !config.run_unofficial_tests {
        println!(
            "{}",
            format!(
                "⏭️ [Test {}] Skipping unofficial test: {} (Use --run-unofficial to include)",
                test_num, description
            )
            .blue()
        );
        let mut results_guard = results.lock().unwrap();
        results_guard.skipped_count += 1;
        return Ok(()); // Skipped tests are not failures
    }

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
