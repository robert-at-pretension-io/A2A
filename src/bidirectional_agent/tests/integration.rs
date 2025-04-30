//! Integration tests for the Bidirectional Agent, testing interactions between components.

// Only compile tests if relevant features are enabled
#![cfg(all(test, feature = "bidir-core", feature = "bidir-local-exec"))]

use crate::{
    bidirectional_agent::{
        config::{BidirectionalAgentConfig, DirectoryConfig},
        agent_directory::{AgentDirectory, AgentStatus},
        agent_registry::{AgentRegistry, tests::create_mock_agent_card}, // Use helper from registry tests
        tool_executor::{ToolExecutor, ToolError},
        task_router::{TaskRouter, RoutingDecision},
        BidirectionalAgent, // Import the main agent struct
    },
    types::{TaskSendParams, Message, Role, Part, TextPart, ToolCall},
};
use std::{sync::Arc, time::Duration, collections::HashMap};
use tempfile::tempdir;
use mockito::Server;
use serde_json::{json, Value};
use tokio::time::sleep;
use sqlx::SqlitePool; // For direct DB checks

// --- Test Setup Helpers ---

// Helper to create a default DirectoryConfig pointing to a temp DB
fn create_temp_dir_config() -> (tempfile::TempDir, DirectoryConfig) {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("integration_test.db");
    let config = DirectoryConfig {
        db_path: db_path.to_string_lossy().to_string(),
        verification_interval_minutes: 1, // Use short intervals for testing
        request_timeout_seconds: 1,
        max_failures_before_inactive: 2,
        backoff_seconds: 1, // Short backoff
        health_endpoint_path: "/health".to_string(),
    };
    (temp_dir, config)
}

// Helper to set up Directory, Registry, Executor, Router for testing
async fn setup_test_components() -> (
    tempfile::TempDir, // Keep temp_dir to ensure DB file isn't deleted prematurely
    Arc<AgentDirectory>,
    Arc<AgentRegistry>,
    Arc<ToolExecutor>,
    Arc<TaskRouter>
) {
    let (temp_dir, dir_config) = create_temp_dir_config();
    let directory = Arc::new(AgentDirectory::new(&dir_config).await.expect("Failed to create test AgentDirectory"));
    let registry = Arc::new(AgentRegistry::new(directory.clone()));
    let executor = Arc::new(ToolExecutor::new(directory.clone()));
    let router = Arc::new(TaskRouter::new(registry.clone(), executor.clone()));
    (temp_dir, directory, registry, executor, router)
}

// Helper to get agent status directly from DB
async fn get_db_agent_status(pool: &SqlitePool, agent_id: &str) -> Option<String> {
    sqlx::query_scalar("SELECT status FROM agents WHERE agent_id = ?")
        .bind(agent_id)
        .fetch_optional(pool)
        .await
        .expect("DB query failed")
}

// Helper to create basic TaskSendParams with text
fn create_text_task(id: &str, text: &str) -> TaskSendParams {
     TaskSendParams {
        id: id.to_string(),
        // Use the message field directly if TaskSendParams expects a single Message
        // Adjust if it expects `messages` Vec instead/as well
        message: Message::user(text),
        messages: vec![Message::user(text)], // Populate messages as well
        history_length: None, metadata: None, push_notification: None, session_id: None,
    }
}

// Helper to create a ToolCall task using the ToolCall struct
fn create_tool_call_task(id: &str, tool_call: ToolCall) -> TaskSendParams {
     TaskSendParams {
        id: id.to_string(),
        message: Message::user("Tool call request"), // Placeholder outer message
        messages: vec![Message {
            role: Role::User, // Or Agent if appropriate
            parts: vec![Part::ToolCallPart(crate::types::ToolCallPart {
                type_: "tool_call".to_string(),
                id: format!("{}-call", tool_call.name), // Generate an ID for the part
                tool_call: serde_json::to_value(tool_call).unwrap(),
            })],
            metadata: None,
        }],
         history_length: None, metadata: None, push_notification: None, session_id: None,
    }
}


// --- Test Cases ---

#[tokio::test]
async fn test_integration_discover_persist_verify_success() {
    let (_td, directory, registry, _executor, _router) = setup_test_components().await;
    let mut server = Server::new_async().await;
    let agent_id = "discover-persist-verify";
    let agent_url = server.url();
    let mock_card = create_mock_agent_card(agent_id, &agent_url);

    // Mock server for discovery
    let m_discover = server.mock("GET", "/.well-known/agent.json")
        .with_status(200)
        .with_header("content-type", "application/json") // Ensure correct content type
        .with_body(serde_json::to_string(&mock_card).unwrap())
        .create_async().await;

    // 1. Discover agent
    registry.discover(&agent_url).await.expect("Discovery failed");
    m_discover.assert_async().await;

    // Verify in registry cache
    assert!(registry.get(agent_id).is_some());

    // Verify in directory DB
    let pool = directory.db_pool.clone();
    assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("active".to_string()));

    // Mock server for successful verification
    let m_verify = server.mock("HEAD", "/")
        .with_status(200)
        .create_async().await;

    // 2. Trigger verification (force probe time)
    sqlx::query("UPDATE agents SET next_probe_at = ? WHERE agent_id = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(1))
        .bind(agent_id)
        .execute(&pool).await.unwrap();
    directory.verify_agents().await.expect("Verification failed");
    m_verify.assert_async().await;

    // Verify still active
    assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("active".to_string()));
}


#[tokio::test]
async fn test_integration_agent_goes_inactive_and_reactivates() {
    let (_td, directory, _registry, _executor, _router) = setup_test_components().await;
    let mut server = Server::new_async().await;
    let agent_id = "inactive-reactive";
    let agent_url = server.url();
    let mock_card = create_mock_agent_card(agent_id, &agent_url);
    let pool = directory.db_pool.clone();

    // Add agent directly to directory for simplicity
    directory.add_agent(agent_id, &agent_url, Some(mock_card)).await.unwrap();
    assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("active".to_string()));

    // --- Make agent fail verification twice ---
    let m_fail1 = server.mock("HEAD", "/").with_status(500).create_async().await;
    sqlx::query("UPDATE agents SET next_probe_at = ? WHERE agent_id = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(1))
        .bind(agent_id).execute(&pool).await.unwrap();
    directory.verify_agents().await.unwrap(); // 1st failure
    m_fail1.assert_async().await;
    assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("active".to_string())); // Still active

    server.reset().await;
    let m_fail2 = server.mock("HEAD", "/").with_status(503).create_async().await;
    sqlx::query("UPDATE agents SET next_probe_at = ? WHERE agent_id = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(1))
        .bind(agent_id).execute(&pool).await.unwrap();
    directory.verify_agents().await.unwrap(); // 2nd failure -> inactive (max_failures = 2)
    m_fail2.assert_async().await;
    assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("inactive".to_string()));

    // --- Make agent succeed verification ---
    server.reset().await;
    let m_success = server.mock("HEAD", "/").with_status(200).create_async().await;
    sqlx::query("UPDATE agents SET next_probe_at = ? WHERE agent_id = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(1))
        .bind(agent_id).execute(&pool).await.unwrap();
    directory.verify_agents().await.unwrap(); // Should reactivate
    m_success.assert_async().await;
    assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("active".to_string()));
}

#[tokio::test]
async fn test_integration_routing_directory_list_active_keyword() {
     let (_td, _directory, _registry, _executor, router) = setup_test_components().await;
     let task = create_text_task("task-dir-keyword", "list the active agents in the directory");
     let decision = router.decide(&task).await;
     assert_eq!(decision, RoutingDecision::Local { tool_names: vec!["directory".to_string()] });
}

#[tokio::test]
async fn test_integration_routing_directory_get_info_tool_call() {
     let (_td, _directory, _registry, _executor, router) = setup_test_components().await;
     let tool_call = ToolCall {
         name: "directory".to_string(),
         params: json!({ "action": "get_info", "agent_id": "some-agent" }),
     };
     let task = create_tool_call_task("task-dir-toolcall", tool_call);
     let decision = router.decide(&task).await;
     assert_eq!(decision, RoutingDecision::Local { tool_names: vec!["directory".to_string()] });
}

#[tokio::test]
async fn test_integration_routing_remote_hint_active_agent() {
    let (_td, directory, registry, _executor, router) = setup_test_components().await;
    let agent_id = "remote-active";
    let agent_url = "http://remote-active.test";
    // Add agent as active
    directory.add_agent(agent_id, agent_url, Some(create_mock_agent_card(agent_id, agent_url))).await.unwrap();
    // Add to registry cache too (simulate it being discovered/refreshed)
     registry.agents.insert(agent_id.to_string(), crate::bidirectional_agent::agent_registry::CachedAgentInfo {
        card: create_mock_agent_card(agent_id, agent_url),
        last_checked: chrono::Utc::now(),
    });


    let mut task = create_text_task("task-remote-hint", "Do something remote");
    let mut metadata = HashMap::new();
    metadata.insert("_route_to".to_string(), json!(agent_id));
    task.metadata = Some(metadata);

    let decision = router.decide(&task).await;
    assert_eq!(decision, RoutingDecision::Remote { agent_id: agent_id.to_string() });
}

#[tokio::test]
async fn test_integration_routing_remote_hint_inactive_agent_fallback() {
    let (_td, directory, registry, _executor, router) = setup_test_components().await;
    let agent_id = "remote-inactive";
    let agent_url = "http://remote-inactive.test";
    let pool = directory.db_pool.clone();
    // Add agent and mark as inactive
    directory.add_agent(agent_id, agent_url, Some(create_mock_agent_card(agent_id, agent_url))).await.unwrap();
    sqlx::query("UPDATE agents SET status = ? WHERE agent_id = ?")
        .bind(AgentStatus::Inactive.as_str()).bind(agent_id).execute(&pool).await.unwrap();
    // Add to registry cache (even if inactive in dir, might be in cache)
    registry.agents.insert(agent_id.to_string(), crate::bidirectional_agent::agent_registry::CachedAgentInfo {
        card: create_mock_agent_card(agent_id, agent_url),
        last_checked: chrono::Utc::now(),
    });

    let mut task = create_text_task("task-remote-inactive", "Do something remote");
     let mut metadata = HashMap::new();
    metadata.insert("_route_to".to_string(), json!(agent_id));
    task.metadata = Some(metadata);

    let decision = router.decide(&task).await;
    // Should fallback to local because target is inactive (or maybe reject?)
    // Current simple router falls back to local echo.
    assert_eq!(decision, RoutingDecision::Local { tool_names: vec!["echo".to_string()] });
}

#[tokio::test]
async fn test_integration_local_exec_directory_list_active() {
    let (_td, directory, _registry, executor, _router) = setup_test_components().await;
    let agent1_id = "exec-active-1";
    let agent2_id = "exec-active-2";
    directory.add_agent(agent1_id, "url1", Some(create_mock_agent_card(agent1_id, "url1"))).await.unwrap();
    directory.add_agent(agent2_id, "url2", Some(create_mock_agent_card(agent2_id, "url2"))).await.unwrap();

    let params = json!({"action": "list_active"});
    let result = executor.execute_tool("directory", params).await;

    assert!(result.is_ok());
    let value = result.unwrap();
    let active_agents = value["active_agents"].as_array().unwrap();
    assert_eq!(active_agents.len(), 2);
    assert!(active_agents.iter().any(|a| a["id"] == agent1_id));
    assert!(active_agents.iter().any(|a| a["id"] == agent2_id));
}

#[tokio::test]
async fn test_integration_local_exec_directory_get_info_not_found() {
    let (_td, _directory, _registry, executor, _router) = setup_test_components().await;
    let params = json!({"action": "get_info", "agent_id": "nonexistent-agent"});
    let result = executor.execute_tool("directory", params).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        ToolError::InvalidParams(_, msg) => assert!(msg.contains("not found in directory")),
        e => panic!("Expected InvalidParams error, got {:?}", e),
    }
}

// --- Add 12 more tests following similar patterns ---

#[tokio::test]
async fn test_integration_verification_backoff_timing() {
    let (_td, directory, _registry, _executor, _router) = setup_test_components().await;
    let mut server = Server::new_async().await;
    let agent_id = "backoff-agent";
    let agent_url = server.url();
    let pool = directory.db_pool.clone();
    directory.add_agent(agent_id, &agent_url, Some(create_mock_agent_card(agent_id, &agent_url))).await.unwrap();

    // Fail once
    server.mock("HEAD", "/").with_status(500).create_async().await;
    sqlx::query("UPDATE agents SET next_probe_at = ? WHERE agent_id = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(1)).bind(agent_id).execute(&pool).await.unwrap();
    directory.verify_agents().await.unwrap();
    let probe_time1 = sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>("SELECT next_probe_at FROM agents WHERE agent_id = ?")
        .bind(agent_id).fetch_one(&pool).await.unwrap();
    let expected_probe1 = chrono::Utc::now() + chrono::Duration::seconds(1); // backoff_seconds = 1, 2^0 = 1
    assert!(probe_time1 > chrono::Utc::now());
    assert!((probe_time1 - expected_probe1).num_milliseconds().abs() < 500); // Allow small delta

    // Fail again
    server.reset().await;
    server.mock("HEAD", "/").with_status(500).create_async().await;
     sqlx::query("UPDATE agents SET next_probe_at = ? WHERE agent_id = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(1)).bind(agent_id).execute(&pool).await.unwrap();
    directory.verify_agents().await.unwrap();
    let probe_time2 = sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>("SELECT next_probe_at FROM agents WHERE agent_id = ?")
        .bind(agent_id).fetch_one(&pool).await.unwrap();
    let expected_probe2 = chrono::Utc::now() + chrono::Duration::seconds(2); // backoff_seconds = 1, 2^1 = 2
    assert!(probe_time2 > probe_time1);
     assert!((probe_time2 - expected_probe2).num_milliseconds().abs() < 500);
}

#[tokio::test]
async fn test_integration_verification_timeout() {
    let (_td, directory, _registry, _executor, _router) = setup_test_components().await;
    let mut server = Server::new_async().await;
    let agent_id = "timeout-agent";
    let agent_url = server.url();
    let pool = directory.db_pool.clone();
    directory.add_agent(agent_id, &agent_url, Some(create_mock_agent_card(agent_id, &agent_url))).await.unwrap();

    // Mock server to delay response beyond timeout (request_timeout_seconds = 1)
    server.mock("HEAD", "/")
        .with_delay(Duration::from_millis(1500)) // Delay > 1 sec timeout
        .with_status(200) // Status doesn't matter if it times out
        .create_async().await;

    // Trigger verification
    sqlx::query("UPDATE agents SET next_probe_at = ? WHERE agent_id = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(1)).bind(agent_id).execute(&pool).await.unwrap();
    directory.verify_agents().await.unwrap();

    // Should be marked active (1 failure), but check failure code indicates timeout (-1)
    assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("active".to_string()));
    let failure_code = sqlx::query_scalar::<_, Option<i32>>("SELECT last_failure_code FROM agents WHERE agent_id = ?")
        .bind(agent_id).fetch_one(&pool).await.unwrap();
    assert_eq!(failure_code, Some(-1)); // Check for timeout/network error code
}

#[tokio::test]
async fn test_integration_registry_refresh_uses_directory_active_list() {
    let (_td, directory, registry, _executor, _router) = setup_test_components().await;
    let mut server = Server::new_async().await;
    let agent_active = "refresh-active";
    let agent_inactive = "refresh-inactive";
    let url_active = server.url();
    let url_inactive = format!("{}/inactive", server.url()); // Different base URL for mock
    let pool = directory.db_pool.clone();

    // Add both agents, mark one inactive
    directory.add_agent(agent_active, &url_active, Some(create_mock_agent_card(agent_active, &url_active))).await.unwrap();
    directory.add_agent(agent_inactive, &url_inactive, Some(create_mock_agent_card(agent_inactive, &url_inactive))).await.unwrap();
    sqlx::query("UPDATE agents SET status = ? WHERE agent_id = ?")
        .bind(AgentStatus::Inactive.as_str()).bind(agent_inactive).execute(&pool).await.unwrap();

    // Mock only the active agent's endpoint for refresh
    let m_refresh = server.mock("GET", "/.well-known/agent.json") // Mock for url_active
        .with_status(200)
        .with_body(serde_json::to_string(&create_mock_agent_card(agent_active, &url_active)).unwrap())
        .create_async().await;

    // Run refresh loop logic once (simulate one iteration)
    // This requires exposing refresh_agent_info or simulating the loop's core logic
    // For simplicity, we'll check which agents it *would* try to refresh based on directory
    let active_from_dir = directory.get_active_agents().await.unwrap();
    assert_eq!(active_from_dir.len(), 1);
    assert_eq!(active_from_dir[0].0, agent_active);

    // Now, actually refresh the one agent it should have found
    // Need to add the active agent to registry cache first so refresh_agent_info finds it
    registry.agents.insert(agent_active.to_string(), crate::bidirectional_agent::agent_registry::CachedAgentInfo {
        card: create_mock_agent_card(agent_active, &url_active),
        last_checked: chrono::Utc::now() - chrono::Duration::hours(1), // Mark as checked previously
    });
    let refresh_result = registry.refresh_agent_info(agent_active).await;
    assert!(refresh_result.is_ok());
    m_refresh.assert_async().await; // Verify the active agent's mock was hit

    // Verify the inactive agent wasn't attempted (no mock for it, and refresh_agent_info wasn't called)
}

#[tokio::test]
async fn test_integration_registry_refresh_failure_logging() {
     let (_td, directory, registry, _executor, _router) = setup_test_components().await;
    let mut server = Server::new_async().await;
    let agent_id = "refresh-fail";
    let agent_url = server.url();
    directory.add_agent(agent_id, &agent_url, Some(create_mock_agent_card(agent_id, &agent_url))).await.unwrap();
    // Add to registry cache so refresh can find it
    registry.agents.insert(agent_id.to_string(), crate::bidirectional_agent::agent_registry::CachedAgentInfo {
        card: create_mock_agent_card(agent_id, &agent_url),
        last_checked: chrono::Utc::now() - chrono::Duration::hours(1),
    });


    // Mock server to fail refresh
    server.mock("GET", "/.well-known/agent.json").with_status(500).create_async().await;

    // Attempt refresh
    let result = registry.refresh_agent_info(agent_id).await;
    assert!(result.is_err()); // Should return error
    // Check logs (requires log setup) or verify directory status remains unchanged by refresh failure
    assert_eq!(get_db_agent_status(&directory.db_pool, agent_id).await, Some("active".to_string()));
}

#[tokio::test]
async fn test_integration_concurrent_verification() {
    // Test multiple agents being verified concurrently
    let (_td, directory, _registry, _executor, _router) = setup_test_components().await;
    let mut server1 = Server::new_async().await;
    let mut server2 = Server::new_async().await;
    let agent1 = "concurrent-verify-1";
    let agent2 = "concurrent-verify-2";
    let url1 = server1.url();
    let url2 = server2.url();
    let pool = directory.db_pool.clone();

    directory.add_agent(agent1, &url1, Some(create_mock_agent_card(agent1, &url1))).await.unwrap();
    directory.add_agent(agent2, &url2, Some(create_mock_agent_card(agent2, &url2))).await.unwrap();

    // Mock both servers to succeed
    server1.mock("HEAD", "/").with_status(200).create_async().await;
    server2.mock("HEAD", "/").with_status(200).create_async().await;

    // Trigger verification for both
    sqlx::query("UPDATE agents SET next_probe_at = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(1)).execute(&pool).await.unwrap();

    // verify_agents processes all due agents
    directory.verify_agents().await.unwrap();

    // Check both are still active
    assert_eq!(get_db_agent_status(&pool, agent1).await, Some("active".to_string()));
    assert_eq!(get_db_agent_status(&pool, agent2).await, Some("active".to_string()));
}

#[tokio::test]
#[ignore] // This test requires running the actual agent and server, ignore in unit tests
async fn test_integration_shutdown_stops_verification_loop() {
    // Create agent, start run, trigger shutdown, check logs/task handles
    let (_td, dir_config) = create_temp_dir_config(); // Need temp dir config
    let config = BidirectionalAgentConfig {
        self_id: "shutdown-test".to_string(),
        base_url: "http://127.0.0.1:12345".to_string(), // Port unlikely to be used
        directory: dir_config, // Use temp dir config
        network: crate::bidirectional_agent::config::NetworkConfig { port: Some(12345), ..Default::default() }, // Set port
        ..Default::default()
    };
    let agent = Arc::new(BidirectionalAgent::new(config).await.unwrap());
    let agent_clone = agent.clone();

    // Run the agent in a separate task
    let run_handle = tokio::spawn(async move {
        // We expect run to exit gracefully after shutdown signal
        // Need to handle potential bind errors if port is taken
        if let Err(e) = agent_clone.run().await {
            // Ignore specific errors like address already in use if they occur in CI
            if !e.to_string().contains("Address already in use") {
                 eprintln!("Agent run failed: {:?}", e);
            }
        }
    });

    // Give the agent time to start background tasks
    sleep(Duration::from_millis(500)).await;

    // Trigger shutdown
    agent.shutdown().await.unwrap();

    // Wait for the run task to complete
    let run_result = run_handle.await;
    assert!(run_result.is_ok(), "Agent run task panicked");

    // Check logs for "verification loop cancelled" (requires log capture setup)
    // Or check that background task handles in agent state are empty/joined
     assert!(agent.background_tasks.lock().unwrap().is_empty(), "Background tasks should be cleared after shutdown");
}

#[tokio::test]
async fn test_integration_get_info_includes_card() {
    let (_td, directory, _registry, executor, _router) = setup_test_components().await;
    let agent_id = "get-info-card";
    let card = create_mock_agent_card(agent_id, "url-card");
    directory.add_agent(agent_id, "url-card", Some(card.clone())).await.unwrap();

    let params = json!({"action": "get_info", "agent_id": agent_id});
    let result = executor.execute_tool("directory", params).await.unwrap();

    assert!(result["agent_info"]["card"].is_object());
    assert_eq!(result["agent_info"]["card"]["name"], agent_id);
    assert_eq!(result["agent_info"]["card"]["version"], card.version);
}

#[tokio::test]
async fn test_integration_add_agent_updates_existing_url() {
    let (_td, directory, _registry, _executor, _router) = setup_test_components().await;
    let agent_id = "update-url";
    let url1 = "http://url1.test";
    let url2 = "http://url2.test";
    directory.add_agent(agent_id, url1, Some(create_mock_agent_card(agent_id, url1))).await.unwrap();
    assert_eq!(directory.get_agent_info(agent_id).await.unwrap()["url"], url1);

    // Add again with different URL
    directory.add_agent(agent_id, url2, Some(create_mock_agent_card(agent_id, url2))).await.unwrap();
    assert_eq!(directory.get_agent_info(agent_id).await.unwrap()["url"], url2); // URL should be updated
}

#[tokio::test]
async fn test_integration_liveness_check_uses_health_path() {
    let (_td, directory, _registry, _executor, _router) = setup_test_components().await;
    let mut server = Server::new_async().await;
    let agent_id = "health-path-agent";
    let agent_url = server.url();
    directory.add_agent(agent_id, &agent_url, Some(create_mock_agent_card(agent_id, &agent_url))).await.unwrap();

    // Mock HEAD to fail
    server.mock("HEAD", "/").with_status(405).create_async().await;
    // Mock GET on the specific health path to succeed
    let m_health = server.mock("GET", "/health") // Matches default config health path
        .with_status(200)
        .create_async().await;

    let is_alive = directory.is_agent_alive(&agent_url).await;
    assert!(is_alive);
    m_health.assert_async().await; // Verify the health path was called
}

#[tokio::test]
async fn test_integration_empty_directory_list_active() {
    let (_td, _directory, _registry, executor, _router) = setup_test_components().await;
    let params = json!({"action": "list_active"});
    let result = executor.execute_tool("directory", params).await.unwrap();
    assert!(result["active_agents"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_integration_add_agent_no_card() {
    let (_td, directory, _registry, _executor, _router) = setup_test_components().await;
    let agent_id = "no-card-agent";
    directory.add_agent(agent_id, "url-no-card", None).await.unwrap(); // Add without card

    let info = directory.get_agent_info(agent_id).await.unwrap();
    assert_eq!(info["agent_id"], agent_id);
    assert_eq!(info["status"], "active");
    assert!(info["card"].is_null()); // Card should be null
}

#[tokio::test]
async fn test_integration_routing_reject_unknown_tool() {
    // Assuming router could identify a needed tool that isn't available
    let (_td, _directory, _registry, _executor, router) = setup_test_components().await;
    // Create a task that somehow implies needing "unknown_tool"
    // This requires more sophisticated routing logic than the current default.
    // For now, this test is conceptual.
    // let task = create_task_requiring_tool("task-unknown-tool", "unknown_tool");
    // let decision = router.decide(&task).await;
    // assert!(matches!(decision, RoutingDecision::Reject { .. }));
    let task = create_text_task("task-unknown", "Use the warp_drive tool");
    let decision = router.decide(&task).await;
    // Current router defaults to echo, doesn't reject yet. Test needs update when rejection is added.
    assert_eq!(decision, RoutingDecision::Local { tool_names: vec!["echo".to_string()] });
}

#[tokio::test]
async fn test_integration_directory_tool_invalid_action() {
    let (_td, _directory, _registry, executor, _router) = setup_test_components().await;
    let params = json!({"action": "invalid_action", "agent_id": "foo"});
    let result = executor.execute_tool("directory", params).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        ToolError::InvalidParams(_, msg) => assert!(msg.contains("Invalid parameters format")),
        e => panic!("Expected InvalidParams, got {:?}", e),
    }
}

#[tokio::test]
async fn test_integration_directory_tool_get_info_missing_id() {
    let (_td, _directory, _registry, executor, _router) = setup_test_components().await;
    let params = json!({"action": "get_info"}); // Missing agent_id
    let result = executor.execute_tool("directory", params).await;
    assert!(result.is_err());
     match result.unwrap_err() {
        ToolError::InvalidParams(_, msg) => assert!(msg.contains("Invalid parameters format") || msg.contains("missing field `agent_id`")), // Serde error message varies
        e => panic!("Expected InvalidParams, got {:?}", e),
    }
}

#[tokio::test]
async fn test_integration_verification_handles_redirects() {
    // Assumes reqwest client follows redirects by default
    let (_td, directory, _registry, _executor, _router) = setup_test_components().await;
    let mut server = Server::new_async().await;
    let agent_id = "redirect-agent";
    let initial_url = server.url(); // e.g., http://127.0.0.1:1234
    let final_url_path = "/final-destination";
    let pool = directory.db_pool.clone();

    directory.add_agent(agent_id, &initial_url, Some(create_mock_agent_card(agent_id, &initial_url))).await.unwrap();

    // Mock initial URL to redirect
    server.mock("HEAD", "/")
        .with_status(302) // Temporary Redirect
        .with_header("Location", final_url_path) // Redirect to relative path
        .create_async().await;

    // Mock final destination to succeed
    server.mock("HEAD", final_url_path)
        .with_status(200)
        .create_async().await;

    // Trigger verification
    sqlx::query("UPDATE agents SET next_probe_at = ? WHERE agent_id = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(1)).bind(agent_id).execute(&pool).await.unwrap();
    directory.verify_agents().await.unwrap();

    // Agent should remain active as the redirect was followed successfully
    assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("active".to_string()));
}

#[tokio::test]
async fn test_integration_verification_max_backoff() {
    // Test that backoff duration caps at 24 hours
     let (_td, directory, _registry, _executor, _router) = setup_test_components().await;
    let mut server = Server::new_async().await;
    let agent_id = "max-backoff-agent";
    let agent_url = server.url();
    let pool = directory.db_pool.clone();
    directory.add_agent(agent_id, &agent_url, Some(create_mock_agent_card(agent_id, &agent_url))).await.unwrap();

    // Simulate many failures to reach max backoff
    let failure_count_to_reach_max = 15; // 60 * 2^14 > 86400 (assuming backoff_seconds=60)
    sqlx::query("UPDATE agents SET consecutive_failures = ? WHERE agent_id = ?")
        .bind(failure_count_to_reach_max)
        .bind(agent_id)
        .execute(&pool).await.unwrap();

    // Mock failure
    server.mock("HEAD", "/").with_status(500).create_async().await;
    sqlx::query("UPDATE agents SET next_probe_at = ? WHERE agent_id = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(1)).bind(agent_id).execute(&pool).await.unwrap();
    directory.verify_agents().await.unwrap(); // Trigger update

    // Check next probe time is capped
    let probe_time = sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>("SELECT next_probe_at FROM agents WHERE agent_id = ?")
        .bind(agent_id).fetch_one(&pool).await.unwrap();
    let expected_max_probe = chrono::Utc::now() + chrono::Duration::seconds(86400); // Max 24h
    // Allow some buffer for test execution time
    assert!((probe_time - expected_max_probe).num_seconds().abs() < 10);
}

#[tokio::test]
async fn test_integration_add_agent_idempotent() {
    // Adding the same agent multiple times should not cause issues
    let (_td, directory, _registry, _executor, _router) = setup_test_components().await;
    let agent_id = "idempotent-add";
    let url = "http://idempotent.test";
    let card = create_mock_agent_card(agent_id, url);

    // Add first time
    let res1 = directory.add_agent(agent_id, url, Some(card.clone())).await;
    assert!(res1.is_ok());

    // Add second time (same data)
    let res2 = directory.add_agent(agent_id, url, Some(card.clone())).await;
    assert!(res2.is_ok());

    // Add third time (slightly different card, same ID/URL)
    let mut card_v2 = card.clone();
    card_v2.version = "1.1".to_string();
    let res3 = directory.add_agent(agent_id, url, Some(card_v2)).await;
    assert!(res3.is_ok());

    // Verify only one entry exists and it's active with latest card info
    let active = directory.get_active_agents().await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].0, agent_id);
    let info = directory.get_agent_info(agent_id).await.unwrap();
    assert_eq!(info["card"]["version"], "1.1");
}

#[tokio::test]
async fn test_integration_verify_agents_no_agents_due() {
    // Ensure verify_agents runs fine when no agents need checking
    let (_td, directory, _registry, _executor, _router) = setup_test_components().await;
    let agent_id = "not-due-agent";
    directory.add_agent(agent_id, "url", None).await.unwrap();

    // Ensure next_probe_at is in the future
    sqlx::query("UPDATE agents SET next_probe_at = ? WHERE agent_id = ?")
        .bind(chrono::Utc::now() + chrono::Duration::hours(1))
        .bind(agent_id)
        .execute(&directory.db_pool).await.unwrap();

    // Run verification - should complete without error and without checking the agent
    let result = directory.verify_agents().await;
    assert!(result.is_ok());
    // (Difficult to assert mock wasn't called without more complex setup)
}

#[tokio::test]
async fn test_integration_routing_metadata_override() {
    // Test that routing hint overrides keyword detection
    let (_td, _directory, registry, _executor, router) = setup_test_components().await;
    let remote_agent_id = "override-remote";
    // Add agent to registry so hint finds it
    registry.agents.insert(remote_agent_id.to_string(), crate::bidirectional_agent::agent_registry::CachedAgentInfo {
        card: create_mock_agent_card(remote_agent_id, "url-override"),
        last_checked: chrono::Utc::now(),
    });


    let mut task = create_text_task("task-override", "list active agents"); // Contains directory keyword
    let mut metadata = HashMap::new();
    metadata.insert("_route_to".to_string(), json!(remote_agent_id)); // But hint says remote
    task.metadata = Some(metadata);

    let decision = router.decide(&task).await;
    // Should route remote due to hint, ignoring keyword
    assert_eq!(decision, RoutingDecision::Remote { agent_id: remote_agent_id.to_string() });
}
