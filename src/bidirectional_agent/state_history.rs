// state_history.rs - A2A Protocol Compliant State Transition History
//
// This file implements proper handling of task state history tracking and reporting
// according to the A2A protocol specification. It ensures the stateTransitionHistory
// capability is properly exposed through the appropriate methods.

use crate::types::{
    Task, TaskStatus, TaskState, Message, AgentCapabilities, TaskIdParams,
    JsonrpcRequest, JsonrpcResponse,
};
use crate::bidirectional_agent::error::AgentError;
use serde::{Serialize, Deserialize};
use serde_json::{json, Value};
use chrono::{DateTime, Utc};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use std::path::Path;
use std::fs::{self, File};
use log::{debug, info, warn, error};

/// Storage configuration for state history
#[derive(Debug, Clone)]
pub struct StateHistoryConfig {
    /// Directory for storing state history files
    pub storage_dir: String,
    
    /// Maximum number of state transitions to keep
    pub max_history_size: usize,
    
    /// Whether to persist history to disk
    pub persist_to_disk: bool,
}

impl Default for StateHistoryConfig {
    fn default() -> Self {
        Self {
            storage_dir: "./state_history".to_string(),
            max_history_size: 100,
            persist_to_disk: true,
        }
    }
}

/// State transition record with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    /// Task ID
    pub task_id: String,
    
    /// From state
    pub from_state: TaskState,
    
    /// To state
    pub to_state: TaskState,
    
    /// Timestamp of transition
    pub timestamp: DateTime<Utc>,
    
    /// Message associated with the transition (if any)
    pub message: Option<String>,
    
    /// Additional metadata
    pub metadata: Option<HashMap<String, Value>>,
}

/// State transition metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransitionMetrics {
    /// Task ID
    pub task_id: String,
    
    /// Total time in each state (seconds)
    pub time_in_state: HashMap<String, f64>,
    
    /// Total transitions
    pub total_transitions: usize,
    
    /// Time in most recent state (seconds)
    pub current_state_time: f64,
    
    /// Current state
    pub current_state: TaskState,
    
    /// First state transition time
    pub first_transition: DateTime<Utc>,
    
    /// Latest state transition time
    pub latest_transition: DateTime<Utc>,
}

/// Manages state history tracking in compliance with A2A protocol
pub struct StateHistoryTracker {
    /// Storage configuration
    config: StateHistoryConfig,
    
    /// In-memory state history cache
    history_cache: RwLock<HashMap<String, Vec<Task>>>,
}

impl StateHistoryTracker {
    /// Create a new state history tracker
    pub fn new(config: StateHistoryConfig) -> Result<Self, AgentError> {
        // Create storage directory if it doesn't exist
        if config.persist_to_disk {
            if !Path::new(&config.storage_dir).exists() {
                fs::create_dir_all(&config.storage_dir)
                    .map_err(|e| AgentError::ConfigError(format!(
                        "Failed to create state history directory: {}", e
                    )))?;
            }
        }
        
        Ok(Self {
            config,
            history_cache: RwLock::new(HashMap::new()),
        })
    }
    
    /// Track a state transition for a task
    pub async fn track_state(&self, task: &Task) -> Result<(), AgentError> {
        let task_id = &task.id;
        
        // Update in-memory cache
        {
            let mut cache = self.history_cache.write().unwrap();
            let history = cache.entry(task_id.clone()).or_insert_with(Vec::new);
            
            // Check if this is a new state compared to the last one
            let is_new_state = match history.last() {
                Some(last_task) => last_task.status.state != task.status.state,
                None => true, // First state is always new
            };
            
            // Only add if it's a new state or history is empty
            if is_new_state || history.is_empty() {
                // Add to history (up to max size)
                history.push(task.clone());
                
                // Trim if exceeding max size
                if history.len() > self.config.max_history_size {
                    let excess = history.len() - self.config.max_history_size;
                    history.drain(0..excess);
                }
            }
        }
        
        // Persist to disk if configured
        if self.config.persist_to_disk {
            self.persist_task_history(task_id).await?;
        }
        
        Ok(())
    }
    
    /// Get the state history for a task
    pub async fn get_state_history(&self, task_id: &str) -> Result<Vec<Task>, AgentError> {
        // Try in-memory cache first
        {
            let cache = self.history_cache.read().unwrap();
            if let Some(history) = cache.get(task_id) {
                return Ok(history.clone());
            }
        }
        
        // If not in cache and persistence is enabled, try to load from disk
        if self.config.persist_to_disk {
            let history = self.load_task_history(task_id).await?;
            
            // Update cache with loaded history
            {
                let mut cache = self.history_cache.write().unwrap();
                cache.insert(task_id.to_string(), history.clone());
            }
            
            return Ok(history);
        }
        
        // No history found
        Ok(Vec::new())
    }
    
    /// Get state transition metrics for a task
    pub async fn get_state_metrics(&self, task_id: &str) -> Result<StateTransitionMetrics, AgentError> {
        // Get state history
        let history = self.get_state_history(task_id).await?;
        
        // If no history, return error
        if history.is_empty() {
            return Err(AgentError::TaskNotFound(task_id.to_string()));
        }
        
        // Calculate metrics
        let mut time_in_state: HashMap<String, f64> = HashMap::new();
        let mut prev_state: Option<(TaskState, DateTime<Utc>)> = None;
        let mut first_transition: Option<DateTime<Utc>> = None;
        let mut latest_transition: Option<DateTime<Utc>> = None;
        
        for task in &history {
            let state = task.status.state.clone();
            let timestamp = task.status.timestamp.unwrap_or_else(Utc::now);
            
            if first_transition.is_none() {
                first_transition = Some(timestamp);
            }
            latest_transition = Some(timestamp);
            
            // Calculate time spent in previous state
            if let Some((prev, prev_timestamp)) = prev_state {
                let duration = timestamp.signed_duration_since(prev_timestamp);
                let seconds = duration.num_milliseconds() as f64 / 1000.0;
                
                *time_in_state.entry(format!("{:?}", prev)).or_insert(0.0) += seconds;
            }
            
            prev_state = Some((state, timestamp));
        }
        
        // Calculate time in current state (up to now)
        let now = Utc::now();
        if let Some((state, timestamp)) = prev_state {
            let duration = now.signed_duration_since(timestamp);
            let seconds = duration.num_milliseconds() as f64 / 1000.0;
            
            *time_in_state.entry(format!("{:?}", state)).or_insert(0.0) += seconds;
        }
        
        // Create metrics
        let current_task = history.last().unwrap();
        let current_state = current_task.status.state.clone();
        let current_state_time = time_in_state.get(&format!("{:?}", current_state)).cloned().unwrap_or(0.0);
        
        Ok(StateTransitionMetrics {
            task_id: task_id.to_string(),
            time_in_state,
            total_transitions: history.len() - 1, // Transitions = states - 1
            current_state_time,
            current_state,
            first_transition: first_transition.unwrap_or_else(Utc::now),
            latest_transition: latest_transition.unwrap_or_else(Utc::now),
        })
    }
    
    /// Get state transitions as formatted events
    pub async fn get_state_transitions(&self, task_id: &str) -> Result<Vec<StateTransition>, AgentError> {
        // Get state history
        let history = self.get_state_history(task_id).await?;
        
        // If no history, return empty list
        if history.is_empty() {
            return Ok(Vec::new());
        }
        
        // Convert history to transitions
        let mut transitions = Vec::new();
        let mut prev_state: Option<(TaskState, DateTime<Utc>)> = None;
        
        for task in &history {
            let state = task.status.state.clone();
            let timestamp = task.status.timestamp.unwrap_or_else(Utc::now);
            
            // Create transition record (except for the first state)
            if let Some((from_state, _)) = prev_state {
                // Only add if state actually changed
                if from_state != state {
                    // Extract message text if present
                    let message = task.status.message.as_ref().and_then(|msg| {
                        msg.parts.iter().find_map(|part| {
                            match part {
                                crate::types::Part::TextPart(text_part) => Some(text_part.text.clone()),
                                _ => None,
                            }
                        })
                    });
                    
                    transitions.push(StateTransition {
                        task_id: task_id.to_string(),
                        from_state,
                        to_state: state.clone(),
                        timestamp,
                        message,
                        metadata: None,
                    });
                }
            }
            
            prev_state = Some((state, timestamp));
        }
        
        Ok(transitions)
    }
    
    /// A2A protocol method handler for getting state history
    pub async fn handle_get_state_history(
        &self,
        params: &TaskIdParams,
    ) -> Result<JsonrpcResponse, AgentError> {
        let task_id = &params.id;
        
        // Get state history
        let history = self.get_state_history(task_id).await?;
        
        // Create response
        let response = JsonrpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)), // Should be passed from request
            result: Some(json!({
                "id": task_id,
                "history": history,
            })),
            error: None,
        };
        
        Ok(response)
    }
    
    /// A2A protocol method handler for getting state metrics
    pub async fn handle_get_state_metrics(
        &self,
        params: &TaskIdParams,
    ) -> Result<JsonrpcResponse, AgentError> {
        let task_id = &params.id;
        
        // Get state metrics
        let metrics = self.get_state_metrics(task_id).await?;
        
        // Create response
        let response = JsonrpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)), // Should be passed from request
            result: Some(json!({
                "id": task_id,
                "metrics": metrics,
            })),
            error: None,
        };
        
        Ok(response)
    }
    
    /// Persist task history to disk
    async fn persist_task_history(&self, task_id: &str) -> Result<(), AgentError> {
        // Get history from cache
        let history = {
            let cache = self.history_cache.read().unwrap();
            match cache.get(task_id) {
                Some(h) => h.clone(),
                None => return Ok(()), // Nothing to persist
            }
        };
        
        // Create storage path
        let file_path = Path::new(&self.config.storage_dir)
            .join(format!("{}.json", task_id));
        
        // Serialize history to JSON
        let json_str = serde_json::to_string(&history)
            .map_err(|e| AgentError::Internal(format!(
                "Failed to serialize task history: {}", e
            )))?;
        
        // Write to file
        fs::write(&file_path, json_str)
            .map_err(|e| AgentError::InternalError(format!(
                "Failed to write task history to disk: {}", e
            )))?;
        
        Ok(())
    }
    
    /// Load task history from disk
    async fn load_task_history(&self, task_id: &str) -> Result<Vec<Task>, AgentError> {
        // Create storage path
        let file_path = Path::new(&self.config.storage_dir)
            .join(format!("{}.json", task_id));
        
        // Check if file exists
        if !file_path.exists() {
            return Ok(Vec::new()); // No history file
        }
        
        // Read file
        let json_str = fs::read_to_string(&file_path)
            .map_err(|e| AgentError::InternalError(format!(
                "Failed to read task history from disk: {}", e
            )))?;
        
        // Deserialize history
        let history: Vec<Task> = serde_json::from_str(&json_str)
            .map_err(|e| AgentError::InternalError(format!(
                "Failed to deserialize task history: {}", e
            )))?;
        
        Ok(history)
    }
    
    /// Clear history for a task
    pub async fn clear_task_history(&self, task_id: &str) -> Result<(), AgentError> {
        // Remove from cache
        {
            let mut cache = self.history_cache.write().unwrap();
            cache.remove(task_id);
        }
        
        // Remove from disk if persistence is enabled
        if self.config.persist_to_disk {
            let file_path = Path::new(&self.config.storage_dir)
                .join(format!("{}.json", task_id));
            
            if file_path.exists() {
                fs::remove_file(&file_path)
                    .map_err(|e| AgentError::InternalError(format!(
                        "Failed to remove task history file: {}", e
                    )))?;
            }
        }
        
        Ok(())
    }
}

/// Extension to update agent capabilities with state history support
pub fn update_agent_capabilities_with_history(capabilities: &mut AgentCapabilities) {
    capabilities.state_transition_history = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile;
    
    /// Helper to create a test task with specific state and message
    fn create_test_task(
        id: &str,
        state: TaskState,
        message: Option<&str>,
    ) -> Task {
        let status_message = message.map(|text| {
            Message {
                role: crate::types::Role::Agent,
                parts: vec![
                    crate::types::Part::TextPart(crate::types::TextPart {
                        type_: "text".to_string(),
                        text: text.to_string(),
                        metadata: None,
                    }),
                ],
                metadata: None,
            }
        });
        
        Task {
            id: id.to_string(),
            session_id: Some("test-session".to_string()),
            status: TaskStatus {
                state,
                timestamp: Some(Utc::now()),
                message: status_message,
            },
            history: None,
            artifacts: None,
            metadata: None,
        }
    }
    
    #[tokio::test]
    async fn test_track_state() {
        // Create temp directory for testing
        let temp_dir = tempfile::tempdir().unwrap();
        
        // Create config
        let config = StateHistoryConfig {
            storage_dir: temp_dir.path().to_string_lossy().to_string(),
            max_history_size: 5,
            persist_to_disk: true,
        };
        
        // Create tracker
        let tracker = StateHistoryTracker::new(config).unwrap();
        
        // Create a test task
        let task_id = "test-task-1";
        let task1 = create_test_task(task_id, TaskState::Submitted, None);
        
        // Track state
        tracker.track_state(&task1).await.unwrap();
        
        // Verify history contains the task
        let history = tracker.get_state_history(task_id).await.unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].status.state, TaskState::Submitted);
        
        // Add another state
        let task2 = create_test_task(task_id, TaskState::Working, Some("Now working"));
        tracker.track_state(&task2).await.unwrap();
        
        // Verify history updated
        let history = tracker.get_state_history(task_id).await.unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].status.state, TaskState::Submitted);
        assert_eq!(history[1].status.state, TaskState::Working);
        
        // Add same state (should not update history)
        let task3 = create_test_task(task_id, TaskState::Working, Some("Still working"));
        tracker.track_state(&task3).await.unwrap();
        
        // Verify history not updated
        let history = tracker.get_state_history(task_id).await.unwrap();
        assert_eq!(history.len(), 2);
    }
    
    #[tokio::test]
    async fn test_state_transitions() {
        // Create in-memory tracker
        let config = StateHistoryConfig {
            storage_dir: "./".to_string(),
            max_history_size: 10,
            persist_to_disk: false,
        };
        
        let tracker = StateHistoryTracker::new(config).unwrap();
        
        // Create a test task and track multiple state changes
        let task_id = "test-task-2";
        
        // Add states in sequence
        let states = [
            (TaskState::Submitted, "Task submitted"),
            (TaskState::Working, "Starting work"),
            (TaskState::InputRequired, "Need more info"),
            (TaskState::Working, "Continuing work"),
            (TaskState::Completed, "Task completed"),
        ];
        
        for (state, msg) in &states {
            let task = create_test_task(task_id, state.clone(), Some(msg));
            tracker.track_state(&task).await.unwrap();
        }
        
        // Get transitions
        let transitions = tracker.get_state_transitions(task_id).await.unwrap();
        
        // Should have 4 transitions (5 states - 1)
        assert_eq!(transitions.len(), 4);
        
        // Verify transitions
        assert_eq!(transitions[0].from_state, TaskState::Submitted);
        assert_eq!(transitions[0].to_state, TaskState::Working);
        assert_eq!(transitions[0].message, Some("Starting work".to_string()));
        
        assert_eq!(transitions[1].from_state, TaskState::Working);
        assert_eq!(transitions[1].to_state, TaskState::InputRequired);
        
        assert_eq!(transitions[2].from_state, TaskState::InputRequired);
        assert_eq!(transitions[2].to_state, TaskState::Working);
        
        assert_eq!(transitions[3].from_state, TaskState::Working);
        assert_eq!(transitions[3].to_state, TaskState::Completed);
        assert_eq!(transitions[3].message, Some("Task completed".to_string()));
    }
    
    #[tokio::test]
    async fn test_state_metrics() {
        // Create in-memory tracker
        let config = StateHistoryConfig {
            storage_dir: "./".to_string(),
            max_history_size: 10,
            persist_to_disk: false,
        };
        
        let tracker = StateHistoryTracker::new(config).unwrap();
        
        // Create a test task and track multiple state changes
        let task_id = "test-task-3";
        
        // Add states in sequence with simulated times
        let now = Utc::now();
        let states = [
            (TaskState::Submitted, "Task submitted", now - chrono::Duration::seconds(100)),
            (TaskState::Working, "Starting work", now - chrono::Duration::seconds(80)),
            (TaskState::InputRequired, "Need more info", now - chrono::Duration::seconds(60)),
            (TaskState::Working, "Continuing work", now - chrono::Duration::seconds(30)),
            (TaskState::Completed, "Task completed", now - chrono::Duration::seconds(0)),
        ];
        
        for (state, msg, timestamp) in &states {
            let mut task = create_test_task(task_id, state.clone(), Some(msg));
            task.status.timestamp = Some(*timestamp);
            tracker.track_state(&task).await.unwrap();
        }
        
        // Get metrics
        let metrics = tracker.get_state_metrics(task_id).await.unwrap();
        
        // Check metrics
        assert_eq!(metrics.task_id, task_id);
        assert_eq!(metrics.total_transitions, 4);
        assert_eq!(metrics.current_state, TaskState::Completed);
        
        // Check times in states
        let working_time = metrics.time_in_state.get("Working").unwrap_or(&0.0);
        let input_required_time = metrics.time_in_state.get("InputRequired").unwrap_or(&0.0);
        
        // Working state: (60-80) + (0-30) = 50 seconds
        assert!((*working_time - 50.0).abs() < 1.0, "Working time should be close to 50");
        
        // InputRequired state: (30-60) = 30 seconds
        assert!((*input_required_time - 30.0).abs() < 1.0, "InputRequired time should be close to 30");
    }
    
    #[tokio::test]
    async fn test_max_history_size() {
        // Create in-memory tracker with small max size
        let config = StateHistoryConfig {
            storage_dir: "./".to_string(),
            max_history_size: 3, // Only keep 3 most recent states
            persist_to_disk: false,
        };
        
        let tracker = StateHistoryTracker::new(config).unwrap();
        
        // Create a test task
        let task_id = "test-task-4";
        
        // Add 5 different states
        let states = [
            TaskState::Submitted,
            TaskState::Working,
            TaskState::InputRequired,
            TaskState::Working,
            TaskState::Completed,
        ];
        
        for state in &states {
            let task = create_test_task(task_id, state.clone(), None);
            tracker.track_state(&task).await.unwrap();
        }
        
        // Get history - should only have 3 most recent
        let history = tracker.get_state_history(task_id).await.unwrap();
        
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].status.state, TaskState::InputRequired);
        assert_eq!(history[1].status.state, TaskState::Working);
        assert_eq!(history[2].status.state, TaskState::Completed);
    }
    
    #[tokio::test]
    async fn test_clear_task_history() {
        // Create temp directory for testing
        let temp_dir = tempfile::tempdir().unwrap();
        
        // Create config
        let config = StateHistoryConfig {
            storage_dir: temp_dir.path().to_string_lossy().to_string(),
            max_history_size: 5,
            persist_to_disk: true,
        };
        
        // Create tracker
        let tracker = StateHistoryTracker::new(config).unwrap();
        
        // Create a test task
        let task_id = "test-task-5";
        let task = create_test_task(task_id, TaskState::Submitted, None);
        
        // Track state
        tracker.track_state(&task).await.unwrap();
        
        // Verify history exists
        let history = tracker.get_state_history(task_id).await.unwrap();
        assert_eq!(history.len(), 1);
        
        // Clear history
        tracker.clear_task_history(task_id).await.unwrap();
        
        // Verify history is cleared
        let history = tracker.get_state_history(task_id).await.unwrap();
        assert_eq!(history.len(), 0);
        
        // Verify file is removed
        let file_path = Path::new(&temp_dir.path())
            .join(format!("{}.json", task_id));
        assert!(!file_path.exists());
    }
}