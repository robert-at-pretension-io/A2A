use crate::client::A2aClient;
use crate::client::errors::ClientError;
use crate::types::{AgentCard, AgentCapabilities};
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::process::Child;
use colored::*; // For colored output

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

    // --- Agent Card & Capabilities ---
    // TODO: Fetch agent card and determine capabilities

    // --- Test Execution ---
    println!("{}", "\n===============================================".blue().bold());
    println!("{}", "🧪 Running Client Function Tests".blue().bold());
    println!("{}", "===============================================".blue().bold());

    // TODO: Implement test sections using run_test helper

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
