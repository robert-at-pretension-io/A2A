//! Agent directory for tracking discovered agents persistently.
//!
//! This module provides functionality to maintain a directory of known agents
//! using SQLite, managing their status, verification history, and backoff.

// Only compile if bidir-core feature is enabled
#![cfg(feature = "bidir-core")]

use crate::{
    bidirectional_agent::config::DirectoryConfig,
    types::AgentCard,
};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, FromRow, migrate::MigrateDatabase, Sqlite};
use std::{path::PathBuf, sync::{Arc, Mutex}, time::Duration};
use tokio_util::sync::CancellationToken;


/// Status of an agent in the directory
#[derive(Debug, Clone, Copy, Serialize, Deserialize, sqlx::Type, PartialEq)] // Added PartialEq
#[sqlx(type_name = "TEXT", rename_all = "lowercase")] // Use sqlx attributes for DB mapping
#[serde(rename_all = "lowercase")] // Use serde attributes for JSON if needed elsewhere
pub enum AgentStatus {
    Active,
    Inactive,
}

impl AgentStatus {
    /// Returns a string representation suitable for database storage.
    fn as_str(&self) -> &'static str {
        match self {
            AgentStatus::Active => "active",
            AgentStatus::Inactive => "inactive",
        }
    }

    /// Parses a status string from the database.
    fn from_str(s: &str) -> Self {
        match s {
            "active" => AgentStatus::Active,
            _ => AgentStatus::Inactive, // Default to inactive if unknown string
        }
    }
}

/// Represents an agent entry retrieved from the database.
#[derive(Debug, Clone, FromRow)]
struct DirectoryEntry {
    // Fields must match the columns selected in queries
    agent_id: String,
    url: String,
    status: String, // Read as string from DB
    last_verified: DateTime<Utc>,
    consecutive_failures: i32, // SQLite INTEGER maps to i32
    last_failure_code: Option<i32>,
    next_probe_at: DateTime<Utc>,
    card_json: Option<String>,
}

impl DirectoryEntry {
    /// Converts the string status from the DB into the AgentStatus enum.
    fn get_status(&self) -> AgentStatus {
        AgentStatus::from_str(&self.status)
    }
}

/// Manages the persistent agent directory using SQLite.
#[derive(Clone)] // Clone is possible because inner state uses Arc/Mutex
pub struct AgentDirectory {
    db_pool: Arc<SqlitePool>,
    client: reqwest::Client,
    max_failures_before_inactive: u32,
    backoff_seconds: u64,
    verification_interval: Duration,
    health_endpoint_path: String,
    // Mutex for last_failure_code as it's updated briefly within async checks
    last_failure_code: Arc<Mutex<Option<i32>>>,
}

/// Simple struct to return basic info about active agents.
#[derive(Debug, Clone)]
pub struct ActiveAgentEntry {
    pub agent_id: String,
    pub url: String,
}


impl AgentDirectory {
    /// Creates a new AgentDirectory instance, initializes the database, and runs migrations.
    pub async fn new(config: &DirectoryConfig) -> Result<Self> {
        let db_path_str = &config.db_path;
        let db_path = PathBuf::from(db_path_str);

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await
                .with_context(|| format!("Failed to create parent directory: {}", parent.display()))?;
        }

        // Create database file if it doesn't exist
        let db_url = format!("sqlite:{}", db_path.display());
        if !Sqlite::database_exists(&db_url).await? {
            log::info!(target: "agent_directory", "Creating SQLite database at {}", db_path_str);
            Sqlite::create_database(&db_url).await
                 .with_context(|| format!("Failed to create SQLite database at {}", db_path_str))?;
        } else {
             log::debug!(target: "agent_directory", "Using existing SQLite database at {}", db_path_str);
        }

        // Create connection pool
        let pool = SqlitePool::connect(&db_url).await
            .with_context(|| format!("Failed to connect to SQLite database: {}", db_path_str))?;

        // Run migrations (relative to CARGO_MANIFEST_DIR)
        log::info!(target: "agent_directory", "Running database migrations...");
        sqlx::migrate!("./src/bidirectional_agent/migrations")
            .run(&pool)
            .await
            .context("Failed to run database migrations")?;
        log::info!(target: "agent_directory", "Database migrations complete.");

        Ok(Self {
            db_pool: Arc::new(pool),
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(config.request_timeout_seconds))
                // Add other client configurations like proxy if needed from NetworkConfig
                .build()?,
            max_failures_before_inactive: config.max_failures_before_inactive,
            backoff_seconds: config.backoff_seconds,
            verification_interval: Duration::from_secs(
                config.verification_interval_minutes * 60
            ),
            health_endpoint_path: config.health_endpoint_path.clone(),
            last_failure_code: Arc::new(Mutex::new(None)),
        })
    }

    /// Adds a new agent or updates an existing one in the directory.
    /// Resets failure count and status to active on successful add/update.
    pub async fn add_agent(&self, agent_id: &str, url: &str, card: Option<AgentCard>) -> Result<()> {
        let card_json = card.and_then(|c| serde_json::to_string(&c).ok());
        let now = Utc::now();

        // Use UPSERT logic (INSERT ON CONFLICT DO UPDATE)
        sqlx::query(
            r#"
            INSERT INTO agents (agent_id, url, status, last_verified, next_probe_at, card_json, consecutive_failures, last_failure_code)
            VALUES (?, ?, ?, ?, ?, ?, 0, NULL) -- Initial values on insert
            ON CONFLICT(agent_id) DO UPDATE SET
                url = excluded.url,
                -- Reset status to active on update, reset failures/probe time
                status = excluded.status,
                last_verified = excluded.last_verified,
                consecutive_failures = 0,
                last_failure_code = NULL,
                next_probe_at = excluded.next_probe_at,
                -- Update card only if a new card is provided
                card_json = CASE WHEN excluded.card_json IS NOT NULL THEN excluded.card_json ELSE agents.card_json END
            "#
        )
        .bind(agent_id)
        .bind(url)
        .bind(AgentStatus::Active.as_str()) // status for INSERT/excluded
        .bind(now) // last_verified for INSERT/excluded
        .bind(now) // next_probe_at for INSERT/excluded (start checking now)
        .bind(card_json) // card_json for INSERT/excluded
        .execute(self.db_pool.as_ref())
        .await
        .with_context(|| format!("Failed to add/update agent '{}' in directory", agent_id))?;

        log::info!(
            target: "agent_directory",
            "Added/updated agent '{}' in directory with URL {} (status: active)",
            agent_id, url
        );
        Ok(())
    }

    /// Retrieves basic info (ID and URL) for all agents currently marked as active.
    pub async fn get_active_agents(&self) -> Result<Vec<ActiveAgentEntry>> {
        let entries = sqlx::query_as::<_, DirectoryEntry>(
            // Select only necessary columns for this purpose
            "SELECT agent_id, url, status, last_verified, consecutive_failures, last_failure_code, next_probe_at, card_json FROM agents WHERE status = ?"
        )
        .bind(AgentStatus::Active.as_str())
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to fetch active agents")?;

        // Map to the simpler ActiveAgentEntry struct
        Ok(entries.into_iter().map(|e| ActiveAgentEntry {
            agent_id: e.agent_id,
            url: e.url,
        }).collect())
    }

    /// Retrieves basic info (ID and URL) for all agents currently marked as inactive.
    pub async fn get_inactive_agents(&self) -> Result<Vec<ActiveAgentEntry>> {
         let entries = sqlx::query_as::<_, DirectoryEntry>(
            // Select only necessary columns for this purpose
            "SELECT agent_id, url, status, last_verified, consecutive_failures, last_failure_code, next_probe_at, card_json FROM agents WHERE status = ?"
        )
        .bind(AgentStatus::Inactive.as_str())
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to fetch inactive agents")?;

        // Map to the simpler ActiveAgentEntry struct
        Ok(entries.into_iter().map(|e| ActiveAgentEntry {
            agent_id: e.agent_id,
            url: e.url,
        }).collect())
    }

    /// Retrieves detailed information for a specific agent by its ID.
    pub async fn get_agent_info(&self, agent_id: &str) -> Result<serde_json::Value> {
        let entry = sqlx::query_as::<_, DirectoryEntry>(
             "SELECT agent_id, url, status, last_verified, consecutive_failures, last_failure_code, next_probe_at, card_json FROM agents WHERE agent_id = ?"
        )
        .bind(agent_id)
        .fetch_optional(self.db_pool.as_ref())
        .await
        .with_context(|| format!("Database error fetching agent info for '{}'", agent_id))?
        .ok_or_else(|| anyhow::anyhow!("Agent '{}' not found in directory", agent_id))?; // More specific error

        // Parse the card if available
        let card: Option<AgentCard> = entry.card_json.as_ref()
            .and_then(|json_str| serde_json::from_str(json_str).ok());

        Ok(serde_json::json!({
            "agent_id": entry.agent_id,
            "url": entry.url,
            "status": entry.status, // Return the string status from DB
            "last_verified": entry.last_verified,
            "consecutive_failures": entry.consecutive_failures,
            "last_failure_code": entry.last_failure_code,
            "next_check": entry.next_probe_at,
            "card": card // Include parsed card object or null
        }))
    }

    /// Performs liveness checks on agents scheduled for verification.
    /// Updates agent status, failure counts, and next probe times based on results.
    pub async fn verify_agents(&self) -> Result<()> {
        let now = Utc::now();

        // Get agents due for verification (next_probe_at <= now)
        let agents_to_verify = sqlx::query_as::<_, DirectoryEntry>(
            "SELECT agent_id, url, status, last_verified, consecutive_failures, last_failure_code, next_probe_at, card_json FROM agents WHERE next_probe_at <= ?"
        )
        .bind(now)
        .fetch_all(self.db_pool.as_ref())
        .await
        .context("Failed to fetch agents due for verification")?;

        if agents_to_verify.is_empty() {
            log::debug!(target: "agent_directory", "No agents due for verification.");
            return Ok(());
        }

        log::info!(target: "agent_directory", "Verifying {} agents due for check...", agents_to_verify.len());

        for agent in agents_to_verify {
            let is_alive = self.is_agent_alive(&agent.url).await;
            let current_status_enum = agent.get_status();
            let failure_code = self.last_failure_code.lock().unwrap().clone(); // Get code from check

            if is_alive {
                // Agent is alive: Reset status to active, reset failures, schedule next check
                let next_probe_time = now + chrono::Duration::from_std(self.verification_interval)?;

                sqlx::query(
                    r#"
                    UPDATE agents SET
                        status = ?, last_verified = ?, consecutive_failures = 0,
                        last_failure_code = NULL, next_probe_at = ?
                    WHERE agent_id = ?
                    "#
                )
                .bind(AgentStatus::Active.as_str())
                .bind(now)
                .bind(next_probe_time)
                .bind(&agent.agent_id)
                .execute(self.db_pool.as_ref())
                .await
                .with_context(|| format!("Failed to update agent '{}' status to active", agent.agent_id))?;

                if current_status_enum == AgentStatus::Inactive {
                    log::info!(
                        target: "agent_directory",
                        "Agent '{}' reactivated after being inactive",
                        agent.agent_id
                    );
                } else {
                     log::debug!(target: "agent_directory", "Agent '{}' verified successfully (still active)", agent.agent_id);
                }
            } else {
                // Agent failed verification: Increment failures, update status if threshold reached, schedule next check with backoff
                let new_failure_count = agent.consecutive_failures + 1;
                let new_status_enum = if new_failure_count >= self.max_failures_before_inactive as i32 {
                    AgentStatus::Inactive
                } else {
                    current_status_enum // Remain in current status until threshold
                };

                // Calculate exponential backoff: base * 2^(failures - 1)
                // Start backoff after the *first* failure (power >= 0)
                let backoff_power = std::cmp::max(0, new_failure_count - 1);
                // Use checked_pow for safety against overflow, though unlikely with u32
                let backoff_multiplier = 2u64.checked_pow(backoff_power as u32).unwrap_or(u64::MAX / self.backoff_seconds); // Cap multiplier
                // Use saturating_mul and min to apply backoff safely and cap duration
                let backoff_duration_secs = std::cmp::min(
                    self.backoff_seconds.saturating_mul(backoff_multiplier),
                    86400 // Cap backoff at 24 hours
                );

                let next_probe_time = now + chrono::Duration::seconds(backoff_duration_secs as i64);

                sqlx::query(
                    r#"
                    UPDATE agents SET
                        status = ?, last_verified = ?, consecutive_failures = ?,
                        last_failure_code = ?, next_probe_at = ?
                    WHERE agent_id = ?
                    "#
                )
                .bind(new_status_enum.as_str())
                .bind(now)
                .bind(new_failure_count)
                .bind(failure_code) // Store the code from the failed check
                .bind(next_probe_time)
                .bind(&agent.agent_id)
                .execute(self.db_pool.as_ref())
                .await
                .with_context(|| format!("Failed to update agent '{}' status after failure", agent.agent_id))?;

                if new_status_enum == AgentStatus::Inactive && current_status_enum == AgentStatus::Active {
                    log::warn!(
                        target: "agent_directory",
                        "Agent '{}' marked as inactive after {} consecutive failures. Status code: {:?}, Next check: {}",
                        agent.agent_id, self.max_failures_before_inactive, failure_code, next_probe_time
                    );
                } else {
                    log::warn!(
                        target: "agent_directory",
                        "Agent '{}' verification failed. Failures: {}, Status code: {:?}, Status: {:?}, Next check: {}",
                        agent.agent_id, new_failure_count, failure_code, new_status_enum, next_probe_time
                    );
                }
            }
        }

        // Update metrics after processing all due agents
        self.update_metrics().await?;
        log::info!(target: "agent_directory", "Agent verification cycle complete.");
        Ok(())
    }

    /// Checks agent liveness using HEAD, with GET fallback for specific errors.
    /// Updates `last_failure_code` as a side effect.
    async fn is_agent_alive(&self, url: &str) -> bool {
        // Reset failure code for this specific check attempt
        *self.last_failure_code.lock().unwrap() = None;

        // 1. Try HEAD request
        match self.client.head(url).send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    return true; // HEAD success means alive
                }

                // Store the non-success status code
                *self.last_failure_code.lock().unwrap() = Some(status.as_u16() as i32);

                // Fallback to GET only for specific non-success codes
                if status == StatusCode::METHOD_NOT_ALLOWED || status == StatusCode::NOT_IMPLEMENTED {
                    log::debug!(target: "agent_directory", "HEAD failed for URL {}, status code {}, trying GET fallback", url, status);
                    return self.try_get_request(url).await;
                }

                // Other HEAD errors (4xx, 5xx) indicate failure
                log::debug!(target: "agent_directory", "HEAD failed for URL {}, status code {}, agent considered inactive", url, status);
                false
            },
            Err(e) => {
                // Network or other error during HEAD request
                log::debug!(target: "agent_directory", "HEAD request error for URL {}: {}, trying GET fallback", url, e);
                // Store a generic error code (e.g., -1) if no status code available
                 *self.last_failure_code.lock().unwrap() = Some(-1);
                self.try_get_request(url).await
            }
        }
    }

    /// Performs a GET request (potentially to a health endpoint) as a fallback liveness check.
    /// Updates `last_failure_code` only if it wasn't already set by the HEAD request.
    async fn try_get_request(&self, url: &str) -> bool {
        // Construct health URL if path is configured
        let health_url = if !self.health_endpoint_path.is_empty() {
            // Ensure correct joining of URL and path
            let base_url = url.trim_end_matches('/');
            let health_path = self.health_endpoint_path.trim_start_matches('/');
            format!("{}/{}", base_url, health_path)
        } else {
            url.to_string() // Use original URL if no health path
        };

        log::debug!(target: "agent_directory", "Attempting GET request to {} for liveness check", health_url);

        // 2. Try GET request with Range header
        match self.client.get(&health_url)
            .header("Range", "bytes=0-0") // Request minimal data
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                // Consider 2xx, 206 Partial Content, or 416 Range Not Satisfiable as success
                let is_success = status.is_success() || status == StatusCode::PARTIAL_CONTENT || status == StatusCode::RANGE_NOT_SATISFIABLE;

                // Update failure code only if HEAD didn't already set one
                let mut code_guard = self.last_failure_code.lock().unwrap();
                if code_guard.is_none() {
                    *code_guard = Some(status.as_u16() as i32);
                }
                drop(code_guard); // Release mutex lock

                if is_success {
                     log::debug!(target: "agent_directory", "GET fallback to {} successful, status: {}", health_url, status);
                } else {
                     log::debug!(target: "agent_directory", "GET fallback to {} failed, status: {}", health_url, status);
                }
                is_success
            },
            Err(e) => {
                // Network or other error during GET request
                log::debug!(target: "agent_directory", "GET request to {} failed: {}", health_url, e);
                 // Update failure code only if HEAD didn't already set one
                let mut code_guard = self.last_failure_code.lock().unwrap();
                if code_guard.is_none() {
                    *code_guard = Some(-1); // Generic network error
                }
                drop(code_guard); // Release mutex lock
                false
            }
        }
    }

    /// Runs the periodic agent verification loop until cancellation.
    pub async fn run_verification_loop(self: Arc<Self>, cancel_token: CancellationToken) -> Result<()> {
        log::info!(target: "agent_directory", "Starting agent verification loop with interval {:?}...", self.verification_interval);
        // Use interval_at to start the first tick immediately
        let mut interval_timer = tokio::time::interval_at(
            tokio::time::Instant::now(), // Start immediately
            self.verification_interval
        );

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    log::info!(target: "agent_directory", "Agent directory verification loop cancelled.");
                    break;
                }
                _ = interval_timer.tick() => {
                    log::debug!(target: "agent_directory", "Running scheduled agent verification task...");
                    // Spawn verification in a separate task to avoid blocking the loop timer if verification takes long
                    let self_clone = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = self_clone.verify_agents().await {
                            log::error!(
                                target: "agent_directory",
                                "Agent verification run failed within loop: {:?}",
                                e
                            );
                        }
                    });
                }
            }
        }
        Ok(())
    }

    /// Updates and logs metrics about the agent directory state.
    async fn update_metrics(&self) -> Result<()> {
        // Could potentially run these counts in parallel
        let active_count_res = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agents WHERE status = ?")
            .bind(AgentStatus::Active.as_str())
            .fetch_one(self.db_pool.as_ref())
            .await;

        let inactive_count_res = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM agents WHERE status = ?")
            .bind(AgentStatus::Inactive.as_str())
            .fetch_one(self.db_pool.as_ref())
            .await;

        // Handle potential errors from counting
        let active_count = active_count_res.context("Failed to count active agents")?;
        let inactive_count = inactive_count_res.context("Failed to count inactive agents")?;

        // Log metrics (replace with actual metrics system later)
        log::info!(
            target: "agent_directory",
            "Agent directory metrics updated - Active: {}, Inactive: {}",
            active_count,
            inactive_count
        );

        // TODO: Expose these via a metrics endpoint (e.g., Prometheus)

        Ok(())
    }
}
