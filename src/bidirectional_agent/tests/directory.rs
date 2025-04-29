//! Tests for the agent directory functionality using SQLite.

// Only compile tests if bidir-core feature is enabled
#![cfg(all(test, feature = "bidir-core"))]

use crate::bidirectional_agent::{
    agent_directory::{AgentDirectory, AgentStatus},
    config::DirectoryConfig,
};
use crate::types::AgentCard;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::Barrier;
use mockito::Server;
use sqlx::SqlitePool;
use std::time::Duration; // For sleep

// Helper to create a DirectoryConfig pointing to a temporary database file.
fn test_config(db_path: String) -> DirectoryConfig {
    DirectoryConfig {
        db_path,
        verification_interval_minutes: 1, // Low interval for testing if needed
        request_timeout_seconds: 1,       // Short timeout for mock server tests
        max_failures_before_inactive: 2,  // Deactivate after 2 failures
        backoff_seconds: 1,               // Start with 1 sec backoff
        health_endpoint_path: "/health".to_string(), // Default health path
    }
}

// Helper to create a minimal valid AgentCard for testing.
fn mock_card(name: &str, url: &str) -> AgentCard {
     crate::types::AgentCard {
         name: name.to_string(),
         url: url.to_string(),
         version: "1.0".to_string(),
         capabilities: Default::default(), // Use default capabilities
         skills: vec![], // Empty skills list
         default_input_modes: vec!["text/plain".to_string()], // Default modes
         default_output_modes: vec!["text/plain".to_string()],
         description: Some(format!("Mock card for {}", name)),
         provider: None,
         documentation_url: None,
         authentication: None,
     }
}

// Helper function to get the status of an agent directly from the database.
async fn get_db_agent_status(pool: &SqlitePool, agent_id: &str) -> Option<String> {
    sqlx::query_scalar::<_, String>("SELECT status FROM agents WHERE agent_id = ?")
        .bind(agent_id)
        .fetch_optional(pool)
        .await
        .expect("Database query for status failed") // Panic on DB error during test
}

// Helper function to get the failure count directly from the database.
async fn get_db_failure_count(pool: &SqlitePool, agent_id: &str) -> Option<i32> {
    sqlx::query_scalar::<_, i32>("SELECT consecutive_failures FROM agents WHERE agent_id = ?")
        .bind(agent_id)
        .fetch_optional(pool)
        .await
        .expect("Database query for failure count failed")
}

// Helper function to get the next probe time directly from the database.
async fn get_db_next_probe_at(pool: &SqlitePool, agent_id: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    sqlx::query_scalar::<_, chrono::DateTime<chrono::Utc>>("SELECT next_probe_at FROM agents WHERE agent_id = ?")
        .bind(agent_id)
        .fetch_optional(pool)
        .await
        .expect("Database query for next probe time failed")
}


#[tokio::test]
async fn test_agent_directory_init_and_migrations() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test_init.db");
    let config = test_config(db_path.to_string_lossy().to_string());

    // Creating AgentDirectory should initialize DB and run migrations
    let directory_result = AgentDirectory::new(&config).await;
    assert!(directory_result.is_ok());

    // Verify database file exists
    assert!(db_path.exists());

    // Connect directly to verify schema (optional, but good check)
    let pool = SqlitePool::connect(&format!("sqlite:{}", db_path.display())).await.unwrap();
    let table_exists = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name='agents'")
        .fetch_optional(&pool).await.unwrap().is_some();
    assert!(table_exists, "Migrations did not create the 'agents' table");
    let index_exists = sqlx::query("SELECT name FROM sqlite_master WHERE type='index' AND name='idx_agents_status'")
        .fetch_optional(&pool).await.unwrap().is_some();
     assert!(index_exists, "Migrations did not create the 'idx_agents_status' index");
}


#[tokio::test]
async fn test_agent_directory_basic_add_get() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test_basic.db");
    let config = test_config(db_path.to_string_lossy().to_string());
    let directory = AgentDirectory::new(&config).await.unwrap();
    let pool = directory.db_pool.clone(); // For direct checks

    // Test adding an agent
    let agent_id = "test-agent-basic";
    let agent_url = "http://basic.example.com";
    directory.add_agent(agent_id, agent_url, Some(mock_card(agent_id, agent_url))).await.unwrap();

    // Verify in DB
    assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("active".to_string()));

    // Test retrieving active agents
    let active_agents = directory.get_active_agents().await.unwrap();
    assert_eq!(active_agents.len(), 1);
    assert_eq!(active_agents[0].0, agent_id);
    assert_eq!(active_agents[0].1, agent_url);

    // Test retrieving inactive agents (should be empty)
    let inactive_agents = directory.get_inactive_agents().await.unwrap();
    assert!(inactive_agents.is_empty());

    // Test getting specific agent info
    let info = directory.get_agent_info(agent_id).await.unwrap();
    assert_eq!(info["agent_id"], agent_id);
    assert_eq!(info["url"], agent_url);
    assert_eq!(info["status"], "active");
    assert_eq!(info["consecutive_failures"], 0);
    assert!(info["last_failure_code"].is_null());
    assert!(info["card"].is_object()); // Check card was stored
    assert_eq!(info["card"]["name"], agent_id);
}

#[tokio::test]
async fn test_agent_directory_add_update() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test_update.db");
    let config = test_config(db_path.to_string_lossy().to_string());
    let directory = AgentDirectory::new(&config).await.unwrap();
    let pool = directory.db_pool.clone();

    let agent_id = "test-agent-update";
    let initial_url = "http://initial.example.com";
    let updated_url = "http://updated.example.com";

    // Add initial version
    directory.add_agent(agent_id, initial_url, Some(mock_card(agent_id, initial_url))).await.unwrap();
    let info1 = directory.get_agent_info(agent_id).await.unwrap();
    assert_eq!(info1["url"], initial_url);
    assert_eq!(info1["status"], "active");

     // Mark as inactive manually for testing update logic
    sqlx::query("UPDATE agents SET status = ?, consecutive_failures = 3 WHERE agent_id = ?")
        .bind(AgentStatus::Inactive.as_str())
        .bind(agent_id)
        .execute(&pool).await.unwrap();
     assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("inactive".to_string()));
     assert_eq!(get_db_failure_count(&pool, agent_id).await, Some(3));


    // Update the agent (e.g., discovered again with a new URL)
    let mut updated_card = mock_card(agent_id, updated_url);
    updated_card.version = "1.1".to_string(); // Change card slightly
    directory.add_agent(agent_id, updated_url, Some(updated_card)).await.unwrap();

    // Verify update
    let info2 = directory.get_agent_info(agent_id).await.unwrap();
    assert_eq!(info2["url"], updated_url);
    assert_eq!(info2["status"], "active"); // Should reset to active
    assert_eq!(info2["consecutive_failures"], 0); // Should reset failures
    assert!(info2["last_failure_code"].is_null()); // Should clear failure code
    assert_eq!(info2["card"]["version"], "1.1"); // Card should be updated
}


#[tokio::test]
async fn test_concurrent_directory_adds() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test_concurrency.db");
    let config = test_config(db_path.to_string_lossy().to_string());

    // Use Arc for shared ownership across tasks
    let directory = Arc::new(AgentDirectory::new(&config).await.unwrap());
    let barrier = Arc::new(Barrier::new(2)); // Barrier to synchronize task start

    // Task 1
    let dir1 = directory.clone();
    let barrier1 = barrier.clone();
    let task1 = tokio::spawn(async move {
        barrier1.wait().await;
        let id = "agent1";
        let url = "http://concurrent1.com";
        dir1.add_agent(id, url, Some(mock_card(id, url))).await
    });

    // Task 2
    let dir2 = directory.clone();
    let barrier2 = barrier.clone();
    let task2 = tokio::spawn(async move {
        barrier2.wait().await;
        let id = "agent2";
        let url = "http://concurrent2.com";
        dir2.add_agent(id, url, Some(mock_card(id, url))).await
    });

    // Wait for both tasks to complete
    let res1 = task1.await.unwrap();
    let res2 = task2.await.unwrap();
    assert!(res1.is_ok(), "Task 1 failed: {:?}", res1.err());
    assert!(res2.is_ok(), "Task 2 failed: {:?}", res2.err());

    // Verify both agents were added correctly
    let agents = directory.get_active_agents().await.unwrap();
    assert_eq!(agents.len(), 2, "Expected 2 active agents");
    assert!(agents.iter().any(|(id, _)| id == "agent1"), "Agent1 missing");
    assert!(agents.iter().any(|(id, _)| id == "agent2"), "Agent2 missing");
}

#[tokio::test]
async fn test_agent_deactivation_and_reactivation() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test_flap.db");
    // Use max_failures = 2 for easier testing
    let config = DirectoryConfig {
         max_failures_before_inactive: 2,
         backoff_seconds: 1, // Short backoff (1s * 2^0, 1s * 2^1, ...)
         verification_interval_minutes: 1, // Not directly used here, but set
         request_timeout_seconds: 1,
         db_path: db_path.to_string_lossy().to_string(),
         health_endpoint_path: "/health".to_string(),
    };

    let directory = Arc::new(AgentDirectory::new(&config).await.unwrap());
    let pool = directory.db_pool.clone(); // Get pool for direct DB checks

    // Start a mock server
    let mut server = Server::new_async().await;
    let agent_id = "flap-agent";
    let url = server.url();

    // Add the agent initially (should be active)
    directory.add_agent(agent_id, &url, Some(mock_card(agent_id, &url))).await.unwrap();
    assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("active".to_string()));
    let initial_probe_time = get_db_next_probe_at(&pool, agent_id).await.unwrap();

    // --- Stage 1: Verification Success ---
    let _m_ok = server.mock("HEAD", "/") // Mock HEAD for success
        .with_status(200)
        .create_async().await;
    // Ensure verification runs by setting probe time to past
    sqlx::query("UPDATE agents SET next_probe_at = ? WHERE agent_id = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(10))
        .bind(agent_id)
        .execute(&pool).await.unwrap();
    directory.verify_agents().await.unwrap(); // Should succeed
    assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("active".to_string()));
    assert_eq!(get_db_failure_count(&pool, agent_id).await, Some(0)); // Failures reset
    // Check next probe time is roughly interval from now
    let probe_time_1 = get_db_next_probe_at(&pool, agent_id).await.unwrap();
    assert!(probe_time_1 > initial_probe_time);
    assert!(probe_time_1 > chrono::Utc::now());


    // --- Stage 2: First Failure ---
    server.reset().await; // Clear previous mocks
    let _m_fail1 = server.mock("HEAD", "/") // Mock HEAD for failure
        .with_status(500)
        .create_async().await;
    sqlx::query("UPDATE agents SET next_probe_at = ? WHERE agent_id = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(10))
        .bind(agent_id)
        .execute(&pool).await.unwrap();
    directory.verify_agents().await.unwrap(); // Should log warning, but remain active
    assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("active".to_string()));
    assert_eq!(get_db_failure_count(&pool, agent_id).await, Some(1)); // Failures incremented
    // Check next probe time reflects backoff (1s * 2^0)
    let probe_time_2 = get_db_next_probe_at(&pool, agent_id).await.unwrap();
     assert!(probe_time_2 > probe_time_1);
     assert!(probe_time_2 > chrono::Utc::now()); // Should be slightly in future


    // --- Stage 3: Second Failure (Deactivation) ---
     server.reset().await;
    let _m_fail2 = server.mock("HEAD", "/") // Mock HEAD for failure again
        .with_status(503)
        .create_async().await;
     sqlx::query("UPDATE agents SET next_probe_at = ? WHERE agent_id = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(10))
        .bind(agent_id)
        .execute(&pool).await.unwrap();
    directory.verify_agents().await.unwrap(); // Should mark as inactive
    assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("inactive".to_string()));
    assert_eq!(get_db_failure_count(&pool, agent_id).await, Some(2)); // Failures incremented
    // Check next probe time reflects backoff (1s * 2^1)
    let probe_time_3 = get_db_next_probe_at(&pool, agent_id).await.unwrap();
    assert!(probe_time_3 > probe_time_2);


    // Check lists
    assert!(directory.get_active_agents().await.unwrap().is_empty());
    assert_eq!(directory.get_inactive_agents().await.unwrap().len(), 1);

    // --- Stage 4: Verification Success (Reactivation) ---
    server.reset().await;
    let _m_reactivate = server.mock("HEAD", "/") // Mock HEAD for success
        .with_status(200)
        .create_async().await;

    // Force immediate verification check regardless of backoff
    sqlx::query("UPDATE agents SET next_probe_at = ? WHERE agent_id = ?")
        .bind(chrono::Utc::now() - chrono::Duration::seconds(10))
        .bind(agent_id)
        .execute(&pool)
        .await
        .unwrap();

    directory.verify_agents().await.unwrap(); // Should reactivate
    assert_eq!(get_db_agent_status(&pool, agent_id).await, Some("active".to_string()));
    assert_eq!(get_db_failure_count(&pool, agent_id).await, Some(0)); // Failures reset

    // Check lists again
    assert_eq!(directory.get_active_agents().await.unwrap().len(), 1);
    assert!(directory.get_inactive_agents().await.unwrap().is_empty());
}

#[tokio::test]
async fn test_liveness_check_fallback_to_get() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test_fallback.db");
    let config = test_config(db_path.to_string_lossy().to_string());
    let directory = AgentDirectory::new(&config).await.unwrap();

    let mut server = Server::new_async().await;
    let url = server.url();

    // Mock HEAD to return 405 (Method Not Allowed)
    let _m_head = server.mock("HEAD", "/")
        .with_status(405)
        .create_async().await;

    // Mock GET on the health path to return 200 OK
    let _m_get = server.mock("GET", "/health") // Matches config.health_endpoint_path
        .with_status(200)
        .create_async().await;

    // Call the internal liveness check directly for testing
    let is_alive = directory.is_agent_alive(&url).await;

    assert!(is_alive, "Agent should be considered alive due to successful GET fallback");
    // Check failure code was stored from HEAD
    assert_eq!(directory.last_failure_code.lock().unwrap().clone(), Some(405));
}

 #[tokio::test]
async fn test_liveness_check_get_range_success() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test_get_range.db");
    let config = test_config(db_path.to_string_lossy().to_string());
    let directory = AgentDirectory::new(&config).await.unwrap();

    let mut server = Server::new_async().await;
    let url = server.url();

    // Mock HEAD to fail (e.g., timeout simulation by not mocking it, or return 500)
     let _m_head = server.mock("HEAD", "/")
        .with_status(500) // Simulate HEAD failure
        .create_async().await;

    // Mock GET to return 206 Partial Content or 416 Range Not Satisfiable
    let _m_get = server.mock("GET", "/health")
        .match_header("Range", "bytes=0-0")
        .with_status(206) // Simulate success with Range request
        .create_async().await;

    let is_alive = directory.is_agent_alive(&url).await;
    assert!(is_alive, "Agent should be alive if GET with Range succeeds (206)");
    // Failure code should be from HEAD initially, GET success doesn't clear it here
     assert_eq!(directory.last_failure_code.lock().unwrap().clone(), Some(500));
}

 #[tokio::test]
async fn test_liveness_check_all_fail() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("test_all_fail.db");
    let config = test_config(db_path.to_string_lossy().to_string());
    let directory = AgentDirectory::new(&config).await.unwrap();

    let mut server = Server::new_async().await;
    let url = server.url();

    // Mock HEAD to fail
     let _m_head = server.mock("HEAD", "/")
        .with_status(503)
        .create_async().await;
    // Mock GET to also fail
    let _m_get = server.mock("GET", "/health")
        .with_status(404)
        .create_async().await;

    let is_alive = directory.is_agent_alive(&url).await;
    assert!(!is_alive, "Agent should be inactive if both HEAD and GET fail");
    // Failure code should be from the *last* check attempt (GET) if HEAD didn't set it,
    // but in this case HEAD failed first.
    assert_eq!(directory.last_failure_code.lock().unwrap().clone(), Some(503));
}
