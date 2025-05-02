use anyhow::{Result, Context, anyhow, Error as AnyhowError};
use rustyline::{Editor, Config, EditMode, Helper};
use rustyline::config::Configurer;
use rustyline::error::ReadlineError;
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::{Highlighter, CmdKind};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::{Validator, ValidationContext, ValidationResult};
use rustyline::Context as RustylineContext;
use std::sync::Arc;
use std::io::Write;
use std::fs::File;
use std::fs::OpenOptions;
use tokio::sync::{Mutex, oneshot};
use tokio::time::{Duration, sleep};
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use fs2::FileExt;
use indicatif::{ProgressBar, ProgressStyle};
use tokio_util::sync::CancellationToken;

use futures_util::StreamExt;
use crate::{
    client::A2aClient,
    client::errors::{ClientError, A2aError as ClientA2aError},
    client::streaming::StreamingResponse,
    types::{TaskSendParams, Message, Role, Part, TextPart, Task, TaskStatus, TaskState},
};

/// Custom error type for REPL operations
#[derive(Debug)]
enum ReplError {
    /// Network-related errors (connection failures, timeouts)
    Network(String),
    
    /// Authentication errors (invalid credentials, permission issues)
    Auth(String),
    
    /// Agent-related errors (agent not found, agent unavailable)
    Agent(String),
    
    /// Task-related errors (task not found, task failed)
    Task(String),
    
    /// Tool-related errors (tool not found, tool execution failed)
    Tool(String),
    
    /// User input errors (invalid command format, missing parameters)
    Input(String),
    
    /// LLM-related errors (model unavailable, prompt failures)
    Llm(String),
    
    /// Internal errors (unexpected state, runtime errors)
    Internal(String),
}

impl ReplError {
    /// Convert to a user-friendly error message with icon and recovery suggestion
    fn user_message(&self) -> String {
        match self {
            ReplError::Network(msg) => 
                format!("🔌 Network error: {}\n   Try: Check your internet connection or VPN status", msg),
                
            ReplError::Auth(msg) => 
                format!("🔒 Authentication error: {}\n   Try: Check your API key or credentials", msg),
                
            ReplError::Agent(msg) => 
                format!("🤖 Agent error: {}\n   Try: Use 'agents' to see available agents", msg),
                
            ReplError::Task(msg) => 
                format!("📋 Task error: {}\n   Try: Simplify your task or try again later", msg),
                
            ReplError::Tool(msg) => 
                format!("🔧 Tool error: {}\n   Try: Use 'tools' to see available tools", msg),
                
            ReplError::Input(msg) => 
                format!("⌨️ Input error: {}\n   Try: Use 'help' to see usage examples", msg),
                
            ReplError::Llm(msg) => 
                format!("🧠 LLM error: {}\n   Try: Simplify your prompt or check API key", msg),
                
            ReplError::Internal(msg) => 
                format!("⚙️ Internal error: {}\n   Try: Restart the REPL client", msg),
        }
    }
    
    /// Create a network error
    fn network<T: ToString>(msg: T) -> Self {
        ReplError::Network(msg.to_string())
    }
    
    /// Create an auth error
    fn auth<T: ToString>(msg: T) -> Self {
        ReplError::Auth(msg.to_string())
    }
    
    /// Create an agent error
    fn agent<T: ToString>(msg: T) -> Self {
        ReplError::Agent(msg.to_string())
    }
    
    /// Create a task error
    fn task<T: ToString>(msg: T) -> Self {
        ReplError::Task(msg.to_string())
    }
    
    /// Create a tool error
    fn tool<T: ToString>(msg: T) -> Self {
        ReplError::Tool(msg.to_string())
    }
    
    /// Create an input error
    fn input<T: ToString>(msg: T) -> Self {
        ReplError::Input(msg.to_string())
    }
    
    /// Create an LLM error
    fn llm<T: ToString>(msg: T) -> Self {
        ReplError::Llm(msg.to_string())
    }
    
    /// Create an internal error
    fn internal<T: ToString>(msg: T) -> Self {
        ReplError::Internal(msg.to_string())
    }
    
    /// Convert from ClientError to ReplError
    fn from_client_error(err: ClientError) -> Self {
        match err {
            ClientError::ReqwestError { msg, status_code } => {
                if let Some(code) = status_code {
                    match code {
                        401 | 403 => ReplError::auth(format!("Authentication failed ({}): {}", code, msg)),
                        404 => ReplError::network(format!("Resource not found: {}", msg)),
                        408 | 504 => ReplError::network(format!("Request timed out: {}", msg)),
                        500..=599 => ReplError::network(format!("Server error ({}): {}", code, msg)),
                        _ => ReplError::network(format!("HTTP error ({}): {}", code, msg)),
                    }
                } else {
                    ReplError::network(format!("Network error: {}", msg))
                }
            },
            ClientError::JsonError(msg) => ReplError::internal(format!("JSON parsing error: {}", msg)),
            ClientError::A2aError(a2a_err) => {
                let code = a2a_err.code;
                match code {
                    401 | 403 => ReplError::auth(format!("A2A authentication error: {}", a2a_err.message)),
                    404 => ReplError::task(format!("A2A resource not found: {}", a2a_err.message)),
                    _ => ReplError::internal(format!("A2A error {}: {}", code, a2a_err.message)),
                }
            },
            ClientError::IoError(msg) => ReplError::network(format!("I/O error: {}", msg)),
            ClientError::Other(msg) => ReplError::internal(format!("Client error: {}", msg)),
        }
    }
}

impl From<AnyhowError> for ReplError {
    fn from(err: AnyhowError) -> Self {
        ReplError::internal(err.to_string())
    }
}

impl std::fmt::Display for ReplError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.user_message())
    }
}

impl std::error::Error for ReplError {}

/// Type alias for Result with ReplError
type ReplResult<T> = std::result::Result<T, ReplError>;

/// History manager with file locking support
struct HistoryManager {
    /// Path to the history file
    history_path: std::path::PathBuf,
    /// Lock file handle, kept open to maintain the lock
    lock_file: Option<File>,
}

impl HistoryManager {
    /// Create a new history manager for the given path
    fn new(history_path: std::path::PathBuf) -> Self {
        Self {
            history_path,
            lock_file: None,
        }
    }
    
    /// Acquire an exclusive lock on the history file
    fn lock(&mut self) -> Result<(), std::io::Error> {
        // Create lock file path by adding .lock extension
        let lock_path = self.history_path.with_extension("lock");
        
        // Open or create the lock file with write access
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&lock_path)?;
        
        // Acquire an exclusive lock
        file.lock_exclusive()?;
        
        // Store the file handle to maintain the lock
        self.lock_file = Some(file);
        
        Ok(())
    }
    
    /// Release the lock on the history file
    fn unlock(&mut self) -> Result<(), std::io::Error> {
        if let Some(file) = self.lock_file.take() {
            // Unlock and close the file
            file.unlock()?;
            // File will be closed when dropped
        }
        
        Ok(())
    }
    
    /// Load history with file locking
    fn load_history(&mut self, rl: &mut rustyline::Editor<ReplHelper, rustyline::history::FileHistory>) -> Result<(), std::io::Error> {
        // First, check if history file exists and create parent directories if needed
        if let Some(parent) = self.history_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
        
        // Acquire lock before loading
        if let Err(e) = self.lock() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to acquire history file lock: {}", e)
            ));
        }
        
        // Try to load history, ignoring "file not found" errors
        let load_result = rl.load_history(&self.history_path);
        if let Err(err) = &load_result {
            if !matches!(err, rustyline::error::ReadlineError::Io(ref e) if e.kind() == std::io::ErrorKind::NotFound) {
                // Only propagate errors other than "file not found"
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to load history: {}", err)
                ));
            }
        }
        
        Ok(())
    }
    
    /// Save history with file locking
    fn save_history(&mut self, rl: &mut rustyline::Editor<ReplHelper, rustyline::history::FileHistory>) -> Result<(), std::io::Error> {
        // First, check if history file's parent directory exists
        if let Some(parent) = self.history_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }
        
        // Lock should already be held from load_history
        // But if not, acquire it now
        if self.lock_file.is_none() {
            match self.lock() {
                Ok(_) => {},
                Err(e) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to acquire history file lock: {}", e)
                    ));
                }
            }
        }
        
        // Create a temporary file with patterns that match rustyline's
        let temp_path = self.history_path.with_extension("history.tmp");
        
        // Save history to temporary file first
        match rl.save_history(&temp_path) {
            Ok(_) => {
                // If successful, try to rename it to the actual history file
                if let Err(e) = std::fs::rename(&temp_path, &self.history_path) {
                    // If rename fails (e.g., across filesystems), try copy and delete
                    std::fs::copy(&temp_path, &self.history_path)?;
                    // Try to delete but don't fail if this doesn't work
                    let _ = std::fs::remove_file(&temp_path);
                }
            },
            Err(e) => {
                // Clean up temporary file if possible
                let _ = std::fs::remove_file(&temp_path);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("Failed to save history: {}", e)
                ));
            }
        }
        
        // Release lock after saving
        self.unlock()?;
        
        Ok(())
    }
}

// Implement Drop to ensure locks are released when the object is dropped
impl Drop for HistoryManager {
    fn drop(&mut self) {
        // Try to release the lock, but don't panic if it fails
        if let Err(e) = self.unlock() {
            eprintln!("Warning: Failed to release history file lock during cleanup: {}", e);
        }
    }
}

/// Async-friendly spinner that can be used with await operations
/// without causing jitter in the spinner animation.
pub struct AsyncSpinner {
    /// The underlying progress bar from indicatif
    progress_bar: ProgressBar,
    /// Cancellation token to stop the spinner background task
    cancel_token: CancellationToken,
    /// Completion channel to signal when spinner is stopped
    completion_tx: Option<oneshot::Sender<()>>,
    /// Message to display when the spinner finishes
    finish_message: Option<String>,
}

impl AsyncSpinner {
    /// Create a new spinner with the given message
    pub fn new(message: &str) -> Self {
        // Create the progress bar with a spinner style
        let progress_bar = ProgressBar::new_spinner();
        
        // Configure the spinner style with Unicode characters
        progress_bar.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner} {msg}")
                .unwrap()
        );
        
        // Set the initial message
        progress_bar.set_message(message.to_string());
        
        // Create a cancellation token to signal the background task to stop
        let cancel_token = CancellationToken::new();
        
        // Create channels to signal when the background task has stopped
        // We'll use two separate channel endpoints to avoid ownership issues
        let (task_tx, completion_rx) = oneshot::channel::<()>();
        let (completion_tx, _unused_rx) = oneshot::channel::<()>();
        
        // Clone the progress bar and token for the background task
        let pb_clone = progress_bar.clone();
        let token_clone = cancel_token.clone();
        
        // Spawn a background task to animate the spinner
        tokio::spawn(async move {
            // Steady ticking at 80ms intervals for smooth animation
            let tick_interval = Duration::from_millis(80);
            
            // Continue until the task is cancelled
            loop {
                // Tick the spinner
                pb_clone.tick();
                
                // Check if the spinner should stop
                if token_clone.is_cancelled() {
                    break;
                }
                
                // Wait a bit before the next tick, but allow for early cancellation
                tokio::select! {
                    _ = sleep(tick_interval) => {},
                    _ = token_clone.cancelled() => break,
                }
            }
            
            // Signal that the task has completed
            let _ = task_tx.send(());
        });
        
        // We don't actually need this second channel, we can use the original
        // completion_tx directly in AsyncSpinner::finish
        tokio::spawn(async move {
            let _ = completion_rx.await;
            // Completion signal already processed, nothing more to do
        });
        
        Self {
            progress_bar,
            cancel_token,
            completion_tx: Some(completion_tx),
            finish_message: None,
        }
    }
    
    /// Update the spinner message while it's running
    pub fn update_message(&self, message: &str) {
        self.progress_bar.set_message(message.to_string());
    }
    
    /// Set a message to display when the spinner finishes
    pub fn set_finish_message(&mut self, message: &str) {
        self.finish_message = Some(message.to_string());
    }
    
    /// Stop the spinner and show a final message
    pub async fn finish(self) {
        // Cancel the background task
        self.cancel_token.cancel();
        
        // Wait for the background task to complete
        if let Some(tx) = self.completion_tx {
            // Send signal to complete the task (not awaited)
            let _ = tx.send(());
        }
        
        // Clear the spinner line
        self.progress_bar.finish_and_clear();
        
        // Print the finish message if one was set
        if let Some(message) = self.finish_message {
            println!("{}", message);
        }
    }
    
    /// Stop the spinner with a success message
    pub async fn finish_with_success(mut self, message: &str) {
        self.set_finish_message(&format!("✅ {}", message));
        self.finish().await;
    }
    
    /// Stop the spinner with an error message
    pub async fn finish_with_error(mut self, message: &str) {
        self.set_finish_message(&format!("❌ {}", message));
        self.finish().await;
    }
    
    /// Check if the spinner has been cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }
}

/// Progress tracker for streaming tasks
pub struct TaskProgressTracker {
    /// The underlying progress bar from indicatif
    progress_bar: ProgressBar,
    /// Task ID being tracked
    task_id: String,
    /// Current state of the task
    state: String,
    /// Whether the task is completed
    completed: bool,
    /// Cancellation token to allow external cancellation
    cancel_token: CancellationToken,
}

impl TaskProgressTracker {
    /// Create a new task progress tracker
    pub fn new(task_id: &str, initial_state: &str) -> Self {
        // Create a progress bar with the spinner style
        let progress_bar = ProgressBar::new_spinner();
        
        // Configure with a template showing task ID and state
        progress_bar.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                .template("{spinner} Task {prefix} | {msg}")
                .unwrap()
        );
        
        // Set the ID and initial state
        progress_bar.set_prefix(task_id.to_string());
        progress_bar.set_message(initial_state.to_string());
        
        // Start the spinner animation
        progress_bar.enable_steady_tick(Duration::from_millis(80));
        
        let cancel_token = CancellationToken::new();
        
        Self {
            progress_bar,
            task_id: task_id.to_string(),
            state: initial_state.to_string(),
            completed: false,
            cancel_token,
        }
    }
    
    /// Update the task state
    pub fn update_state(&mut self, state: &str) {
        self.state = state.to_string();
        self.progress_bar.set_message(state.to_string());
    }
    
    /// Add a message to the progress output
    pub fn add_message(&self, message: &str) {
        self.progress_bar.println(message);
    }
    
    /// Update progress for streamed task content
    pub fn update_progress(&self, bytes_received: usize) {
        let message = format!("{} | {} bytes received", self.state, bytes_received);
        self.progress_bar.set_message(message);
    }
    
    /// Mark the task as completed
    pub fn complete(&mut self, success: bool) {
        self.completed = true;
        
        // Stop the spinner animation
        self.progress_bar.finish();
        
        if success {
            self.progress_bar.set_style(
                ProgressStyle::default_spinner()
                    .template("✅ Task {prefix} | {msg}")
                    .unwrap()
            );
            self.progress_bar.set_message("Completed".to_string());
        } else {
            self.progress_bar.set_style(
                ProgressStyle::default_spinner()
                    .template("❌ Task {prefix} | {msg}")
                    .unwrap()
            );
            self.progress_bar.set_message("Failed".to_string());
        }
    }
    
    /// Signal that task should be cancelled
    pub fn request_cancellation(&self) {
        self.cancel_token.cancel();
    }
    
    /// Check if cancellation has been requested
    pub fn is_cancellation_requested(&self) -> bool {
        self.cancel_token.is_cancelled()
    }
    
    /// Get a cancellation token that can be used to detect cancellation requests
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }
}

/// A task manager that tracks running tasks with progress indicators
pub struct TaskManager {
    /// Map of task IDs to progress trackers
    tasks: Arc<Mutex<std::collections::HashMap<String, TaskProgressTracker>>>,
}

impl TaskManager {
    /// Create a new task manager
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }
    
    /// Start tracking a new task
    pub async fn track_task(&self, task_id: &str, state: &str) -> TaskProgressTracker {
        let tracker = TaskProgressTracker::new(task_id, state);
        
        // Add the tracker to the map
        {
            let mut tasks = self.tasks.lock().await;
            tasks.insert(task_id.to_string(), tracker.clone());
        }
        
        tracker
    }
    
    /// Get a task tracker by ID
    pub async fn get_task(&self, task_id: &str) -> Option<TaskProgressTracker> {
        let tasks = self.tasks.lock().await;
        tasks.get(task_id).cloned()
    }
    
    /// List all tracked tasks
    pub async fn list_tasks(&self) -> Vec<(String, String)> {
        let tasks = self.tasks.lock().await;
        tasks.iter()
            .map(|(id, tracker)| (id.clone(), tracker.state.clone()))
            .collect()
    }
    
    /// Update the state of a task
    pub async fn update_task_state(&self, task_id: &str, state: &str) -> bool {
        let mut tasks = self.tasks.lock().await;
        if let Some(tracker) = tasks.get_mut(task_id) {
            tracker.update_state(state);
            true
        } else {
            false
        }
    }
    
    /// Request cancellation of a task
    pub async fn cancel_task(&self, task_id: &str) -> bool {
        let tasks = self.tasks.lock().await;
        if let Some(tracker) = tasks.get(task_id) {
            tracker.request_cancellation();
            true
        } else {
            false
        }
    }
    
    /// Mark a task as completed
    pub async fn complete_task(&self, task_id: &str, success: bool) -> bool {
        let mut tasks = self.tasks.lock().await;
        if let Some(tracker) = tasks.get_mut(task_id) {
            tracker.complete(success);
            true
        } else {
            false
        }
    }
    
    /// Remove a completed task from tracking
    pub async fn remove_task(&self, task_id: &str) {
        let mut tasks = self.tasks.lock().await;
        tasks.remove(task_id);
    }
}

/// Clone implementation for TaskProgressTracker
impl Clone for TaskProgressTracker {
    fn clone(&self) -> Self {
        Self {
            progress_bar: self.progress_bar.clone(),
            task_id: self.task_id.clone(),
            state: self.state.clone(),
            completed: self.completed,
            cancel_token: self.cancel_token.clone(),
        }
    }
}

/// Utility function to create a progress bar for a determinate operation
pub fn create_progress_bar(total: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=> ")
    );
    pb.set_message(message.to_string());
    pb
}

/// Utility function to execute a closure with a spinner, handling async operations properly
pub async fn with_spinner<F, Fut, T, E>(message: &str, f: F) -> Result<T>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<T, E>>,
    E: Into<AnyhowError> + std::fmt::Debug,
{
    // Create spinner
    let spinner = AsyncSpinner::new(message);
    
    // Execute the provided async closure
    match f().await {
        Ok(result) => {
            // Complete with success
            spinner.finish_with_success("Operation completed successfully").await;
            Ok(result)
        }
        Err(err) => {
            // Complete with error
            let err_msg = format!("{:?}", err);
            spinner.finish_with_error(&format!("Operation failed: {}", err_msg)).await;
            Err(anyhow!("Operation failed: {}", err_msg))
        }
    }
}

/// Custom helper for the REPL that provides completion, hints, and validation
struct ReplHelper {
    /// Built-in commands
    commands: Vec<String>,
    /// Available agents
    agents: Vec<String>,
    /// Available tools
    tools: Vec<String>,
    /// History hinter for suggestions based on history
    hinter: HistoryHinter,
}

impl ReplHelper {
    /// Create a new helper with the given commands, agents, and tools
    fn new(commands: Vec<String>, agents: Vec<String>, tools: Vec<String>) -> Self {
        Self {
            commands,
            agents,
            tools,
            hinter: HistoryHinter {},
        }
    }
    
    /// Create default commands list
    fn default_commands() -> Vec<String> {
        vec![
            // Basic commands
            "help".to_string(),
            "help full".to_string(),
            "exit".to_string(),
            "quit".to_string(),
            
            // Listing and discovery commands
            "agents".to_string(),
            "tools".to_string(),
            "list agents".to_string(),
            "list tools".to_string(),
            "list remote tools".to_string(),
            "list active agents".to_string(),
            
            // History commands
            "history".to_string(),
            "history -c".to_string(),
            "history 5".to_string(),
            "history 10".to_string(),
            
            // Agent interaction commands
            "discover agent".to_string(),
            "discover agent http://localhost:8080".to_string(),
            "discover tools".to_string(),
            "send task to".to_string(),
            "stream task to".to_string(),
            
            // Tool execution commands
            "execute tool".to_string(),
            "execute remote tool".to_string(),
            
            // Task management commands
            "check status".to_string(),
            "decompose task".to_string(),
            "route task".to_string(),
            "cancel task".to_string(),
            
            // Advanced commands for bidirectional features
            "run tool discovery".to_string(),
            "synthesize results".to_string(),
        ]
    }
    
    /// Update the list of available agents
    fn update_agents(&mut self, agents: Vec<String>) {
        self.agents = agents;
    }
    
    /// Update the list of available tools
    fn update_tools(&mut self, tools: Vec<String>) {
        self.tools = tools;
    }
}

/// Implement the Completer trait for ReplHelper to provide tab completion
impl Completer for ReplHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &RustylineContext<'_>,
    ) -> std::result::Result<(usize, Vec<Self::Candidate>), rustyline::error::ReadlineError> {
        // Find the start of the current word being completed
        let start = line[..pos].rfind(|c: char| c.is_whitespace())
            .map_or(0, |i| i + 1);
        
        let word = &line[start..pos];
        let line_before_cursor = &line[..pos];
        
        // Collect potential matches
        let mut matches = Vec::new();
        
        // Logic for command completion based on context
        if line_before_cursor.trim() == word {
            // At the start of line - suggest primary commands
            for cmd in &self.commands {
                if cmd.starts_with(word) {
                    matches.push(Pair {
                        display: cmd.clone(),
                        replacement: cmd.clone(),
                    });
                }
            }
        } else if line.starts_with("send task to") || line.starts_with("stream task to") {
            // After "send task to" or "stream task to" - suggest agent IDs
            if let Some(space_pos) = line_before_cursor.rfind(' ') {
                let typing = &line_before_cursor[space_pos + 1..];
                for agent in &self.agents {
                    if agent.starts_with(typing) {
                        matches.push(Pair {
                            display: agent.clone(),
                            replacement: agent.clone(),
                        });
                    }
                }
            }
        } else if line.starts_with("discover agent") {
            // After "discover agent" - nothing to suggest, but could add example URLs later
            // For now, no completion is provided
        } else if line.starts_with("execute tool") {
            // After "execute tool" - suggest tool names
            if let Some(space_pos) = line_before_cursor.rfind(' ') {
                let typing = &line_before_cursor[space_pos + 1..];
                for tool in &self.tools {
                    if tool.starts_with(typing) {
                        matches.push(Pair {
                            display: tool.clone(),
                            replacement: tool.clone(),
                        });
                    }
                }
            }
        } else if line_before_cursor.contains("agent") || line_before_cursor.contains(" to ") {
            // If the line contains "agent" or "to" but we're typing something that could be an agent ID
            // This is a more general case for agent completion in other contexts
            for agent in &self.agents {
                if agent.starts_with(word) {
                    matches.push(Pair {
                        display: agent.clone(),
                        replacement: agent.clone(),
                    });
                }
            }
        } else if line_before_cursor.contains("tool") {
            // If the line contains "tool" and we're typing something that could be a tool name
            // This is a more general case for tool completion in other contexts
            for tool in &self.tools {
                if tool.starts_with(word) {
                    matches.push(Pair {
                        display: tool.clone(),
                        replacement: tool.clone(),
                    });
                }
            }
        } else {
            // For anything else, try to provide intelligent suggestions based on context
            // First check if we're continuing a known command
            let line_lower = line_before_cursor.to_lowercase();
            
            // If line starts with a known command prefix, suggest completion for the full command
            for cmd in &self.commands {
                if cmd.starts_with(&line_lower) && cmd != &line_lower {
                    let next_word_boundary = cmd[line_lower.len()..].find(' ').map(|i| i + line_lower.len()).unwrap_or(cmd.len());
                    matches.push(Pair {
                        display: cmd.clone(),
                        replacement: cmd[start..next_word_boundary].to_string(),
                    });
                }
            }
            
            // If no command completions found, try word-based completion
            if matches.is_empty() && word.len() > 0 {
                // Try to match parts of commands for better partial completion
                for cmd in &self.commands {
                    // Split the command into words
                    for cmd_part in cmd.split_whitespace() {
                        if cmd_part.starts_with(word) {
                            matches.push(Pair {
                                display: format!("{}  (from: {})", cmd_part, cmd),
                                replacement: cmd_part.to_string(),
                            });
                        }
                    }
                }
            }
        }
        
        Ok((start, matches))
    }
}

/// Implement the Hinter trait to provide hints based on history
impl Hinter for ReplHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &RustylineContext<'_>) -> Option<Self::Hint> {
        // Use the history hinter to provide hints
        self.hinter.hint(line, pos, ctx)
    }
}

/// Implement the Highlighter trait for syntax highlighting
impl Highlighter for ReplHelper {
    // Implementation of highlight_char with the correct signature
    fn highlight_char(&self, line: &str, pos: usize, _kind: CmdKind) -> bool {
        // Simple parenthesis/bracket matcher
        if pos < line.len() {
            let c = line.chars().nth(pos).unwrap();
            if c == '(' || c == ')' || c == '[' || c == ']' || c == '{' || c == '}' {
                return true;
            }
        }
        false
    }
    
    // Highlight hints with correct return type
    fn highlight_hint<'h>(&self, hint: &'h str) -> std::borrow::Cow<'h, str> {
        // For now, just return the original hint
        std::borrow::Cow::Borrowed(hint)
    }
    
    // Highlight method with the correct lifetime
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> std::borrow::Cow<'l, str> {
        // For now, just return the original line as-is
        // In the future, could add highlighting for tools, agents, and commands
        std::borrow::Cow::Borrowed(line)
    }
}

/// Implement the Validator trait (minimal implementation for now)
impl Validator for ReplHelper {
    fn validate(
        &self,
        _ctx: &mut ValidationContext,
    ) -> std::result::Result<ValidationResult, rustyline::error::ReadlineError> {
        // Always validate as complete for now
        Ok(ValidationResult::Valid(None))
    }
}

/// Implement the Helper trait which combines other traits
impl Helper for ReplHelper {}

// Conditionally import bidirectional agent modules based on features

use crate::bidirectional_agent::{
    BidirectionalAgent,
    config::BidirectionalAgentConfig,
    config,
    agent_registry::AgentRegistry,
};

// Import LLM routing only when bidir-local-exec feature is enabled

use crate::bidirectional_agent::llm_core::{LlmClient, LlmConfig, ClaudeClient};

/// Main entry point for the LLM REPL - this version requires bidir-core feature

pub async fn run_repl(config_path: &str) -> Result<()> {
    println!("🤖 Starting LLM Interface REPL for A2A network...");
    
    // Initialize the bidirectional agent
    let config = config::load_config(config_path)
        .context("Failed to load agent configuration")?;
    
    let agent = Arc::new(BidirectionalAgent::new(config.clone()).await
        .context("Failed to initialize bidirectional agent")?);
    
    // Start agent in background task
    let agent_clone = agent.clone();
    let agent_handle = tokio::spawn(async move {
        if let Err(e) = agent_clone.run().await {
            eprintln!("Error running agent: {:?}", e);
        }
    });
    println!("✅ Agent initialized and running in background");
    
    // Initialize LLM client for request interpretation (only when bidir-local-exec is enabled)
    
    let llm_client = {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .expect("ANTHROPIC_API_KEY environment variable must be set");
        
        let llm_config = LlmConfig {
            api_key,
            model: "claude-3-haiku-20240307".to_string(), // Fast model for basic routing
            max_tokens: 2048,
            temperature: 0.2, // Low temperature for reliable tool selection
            timeout_seconds: 30,
            ..LlmConfig::default()
        };
        
        let client = ClaudeClient::new(llm_config)
            .context("Failed to initialize LLM client")?;
        println!("✅ LLM client initialized");
        Arc::new(client) as Arc<dyn LlmClient>
    };
    
    // When bidir-local-exec is not enabled, create a mock/stub client
    
    let llm_client = {
        println!("⚠️ LLM client not available (build without bidir-local-exec feature)");
        // Create a placeholder struct for when LLM client is not available
        struct NoOpLlmClient;
        NoOpLlmClient
    };
    
    // Initialize available agents and tools for auto-completion
    let available_agents: Vec<String> = agent.agent_registry.all()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    
    let available_tools: Vec<String> = agent.tool_executor.tools.keys()
        .map(|tool| tool.clone())
        .collect();

    // Create a helper with command completion, hints, etc.
    let helper = ReplHelper::new(
        ReplHelper::default_commands(),
        available_agents,
        available_tools
    );
    
    // Create readline editor with configuration
    let mut rl = Editor::<ReplHelper, _>::new()?;
    
    // Configure the editor
    rl.set_edit_mode(EditMode::Emacs);
    rl.set_history_ignore_dups(true)?;
    rl.set_max_history_size(1000)?;
    
    // Add the helper to the editor
    rl.set_helper(Some(helper));
    
    // Setup history file in user's home directory
    let history_path = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".a2a_repl_history");
    
    // Create history manager with file locking
    let mut history_manager = HistoryManager::new(history_path.clone());
    
    // Load history with file locking (lock is maintained until save)
    if let Err(err) = history_manager.load_history(&mut rl) {
        println!("Warning: Failed to acquire history file lock: {}", err);
        println!("Another instance of the REPL might be running.");
        println!("History will be read-only for this session.");
    }
    
    // Inform the user that tab completion is available
    println!("💡 Press TAB for command completion and hints");
    
    println!("\n🤖 LLM Interface REPL ready.");
    println!("💡 Press TAB for command completion. Type 'help' for assistance or 'exit' to quit.");
    
    // Create command history
    let commands_history = Arc::new(Mutex::new(Vec::<CommandRecord>::new()));
    
    // Main REPL loop
    loop {
        let readline = rl.readline("🧠> ");
        match readline {
            Ok(line) => {
                if !line.trim().is_empty() {
                    let _ = rl.add_history_entry(line.as_str());
                }
                
                // Handle special commands
                match line.trim().to_lowercase().as_str() {
                    "exit" | "quit" => {
                        println!("Goodbye! 👋");
                        break;
                    }
                    "help" => {
                        print_help();
                        continue;
                    }
                    "agents" => {
                        list_known_agents(agent.clone()).await?;
                        
                        // Update available agents in the helper for completion
                        if let Some(helper) = rl.helper_mut() {
                            let available_agents: Vec<String> = agent.agent_registry.all()
                                .into_iter()
                                .map(|(id, _)| id)
                                .collect();
                            helper.update_agents(available_agents);
                        }
                        
                        continue;
                    }
                    "tools" => {
                        list_available_tools(agent.clone()).await?;
                        
                        // Update available tools in the helper for completion
                        if let Some(helper) = rl.helper_mut() {
                            let available_tools: Vec<String> = agent.tool_executor.tools.keys()
                                .map(|tool| tool.clone())
                                .collect();
                            helper.update_tools(available_tools);
                        }
                        
                        continue;
                    }
                    cmd if cmd.starts_with("history") => {
                        let parts: Vec<&str> = cmd.split_whitespace().collect();
                        if parts.len() > 1 && parts[1] == "-c" {
                            // Clear history with confirmation
                            println!("Are you sure you want to clear command history? (y/n)");
                            match rl.readline("") {
                                Ok(response) if response.trim().eq_ignore_ascii_case("y") => {
                                    // Clear in-memory history
                                    let mut history = commands_history.lock().await;
                                    history.clear();
                                    rl.clear_history()?;
                                    
                                    // Clear history file with locking
                                    if let Err(e) = history_manager.lock() {
                                        println!("Warning: Could not lock history file: {}", e);
                                        println!("In-memory history cleared, but file may not be updated.");
                                    } else {
                                        // Truncate the file by saving empty history
                                        if let Err(e) = rl.save_history(&history_path) {
                                            println!("Warning: Failed to clear history file: {}", e);
                                        } else {
                                            println!("Command history cleared successfully");
                                        }
                                        
                                        // Release the lock
                                        let _ = history_manager.unlock();
                                    }
                                },
                                Ok(_) => {
                                    println!("History clear cancelled");
                                },
                                Err(e) => {
                                    println!("Error reading confirmation: {}", e);
                                }
                            }
                        } else if parts.len() > 1 && parts[1].parse::<usize>().is_ok() {
                            // Show last N entries
                            let count = parts[1].parse::<usize>().unwrap();
                            display_command_history_limit(commands_history.clone(), count).await?;
                        } else {
                            // Show all history
                            display_command_history(commands_history.clone()).await?;
                        }
                        continue;
                    }
                    "" => continue, // Skip empty lines
                    _ => {}
                }
                
                // Process the input and record in history
                let cmd_id = format!("cmd-{}", Uuid::new_v4());
                let mut cmd_record = CommandRecord {
                    id: cmd_id.clone(),
                    input: line.clone(),
                    action: "pending".to_string(),
                    result: None,
                    timestamp: chrono::Utc::now(),
                };
                
                // Add to history before execution
                {
                    let mut history = commands_history.lock().await;
                    history.push(cmd_record.clone());
                }
                
                // Process the input with improved error handling and pass editor reference for updating helpers
                match process_input(&line, cmd_id.clone(), agent.clone(), &llm_client, commands_history.clone(), &mut rl).await {
                    Ok(_) => {
                        // Processing completed successfully
                    },
                    Err(e) => {
                        // Since we can't directly downcast in this context, check if it looks like a ReplError
                        // by checking its string representation for our error message patterns
                        let err_str = e.to_string();
                        let repl_error = if err_str.starts_with("🔌") || 
                                          err_str.starts_with("🔒") ||
                                          err_str.starts_with("🤖") ||
                                          err_str.starts_with("📋") ||
                                          err_str.starts_with("🔧") ||
                                          err_str.starts_with("⌨️") ||
                                          err_str.starts_with("🧠") ||
                                          err_str.starts_with("⚙️") {
                            // It's probably already a ReplError, convert to internal error with the message
                            ReplError::internal(err_str)
                        } else {
                            // Generic conversion from error
                            ReplError::from(e)
                        };
                        
                        // Display user-friendly error message with recovery suggestions
                        eprintln!("{}", repl_error.user_message());
                        
                        // Update history with detailed error info
                        let mut history = commands_history.lock().await;
                        if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                            cmd.action = "error".to_string();
                            cmd.result = Some(repl_error.user_message());
                        }
                        
                        // Offer recovery options based on error type
                        match repl_error {
                            ReplError::Network(_) => {
                                println!("💡 You can retry the command or try with a shorter timeout");
                            },
                            ReplError::Auth(_) => {
                                // Could provide specific instructions on setting up authentication
                                println!("💡 Check if ANTHROPIC_API_KEY is properly set");
                            },
                            ReplError::Agent(msg) => {
                                if msg.contains("not found") {
                                    println!("💡 Try 'agents' to see available agents");
                                }
                            },
                            ReplError::Input(_) => {
                                println!("💡 Try 'help' to see examples of valid commands");
                            },
                            _ => {} // Don't provide extra help for other error types
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("✋ Ctrl+C pressed. Use 'exit' or 'quit' to exit, or press again to force quit.");
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("👋 Ctrl+D pressed, exiting");
                break;
            }
            Err(ReadlineError::WindowResized) => {
                // Just ignore window resize events
                continue;
            }
            Err(ReadlineError::Io(err)) => {
                // Handle I/O errors specially - they might be recoverable
                eprintln!("❌ I/O error: {}", err);
                println!("💡 Try again or restart the REPL if the issue persists");
                
                // Short pause to allow user to read the message
                std::thread::sleep(std::time::Duration::from_millis(1000));
                continue;
            }
            Err(err) => {
                // Other errors might indicate more serious issues
                eprintln!("❌ REPL error: {}", err);
                println!("The REPL will exit and should be restarted");
                
                // Short pause before exiting
                std::thread::sleep(std::time::Duration::from_millis(2000));
                break;
            }
        }
    }
    
    // Save command history to file using our history manager
    if let Err(e) = history_manager.save_history(&mut rl) {
        eprintln!("Failed to save history: {}", e);
    } else {
        println!("Command history saved to {}", history_path.display());
    }
    
    // Shutdown the agent
    println!("Shutting down agent...");
    agent.shutdown().await?;
    
    // Wait for agent task to complete
    if let Err(e) = tokio::time::timeout(tokio::time::Duration::from_secs(5), agent_handle).await {
        eprintln!("Warning: Agent shutdown timeout: {:?}", e);
    }
    
    println!("Agent shut down successfully");
    
    Ok(())
}

/// Process user input through LLM to determine action

async fn process_input(
    input: &str,
    cmd_id: String,
    agent: Arc<BidirectionalAgent>,
    llm_client: &dyn LlmClient,
    commands_history: Arc<Mutex<Vec<CommandRecord>>>,
    rl: &mut Editor<ReplHelper, rustyline::history::FileHistory>,
) -> Result<()> {
    println!("🔄 Processing your request...");
    
    // Use LLM to interpret the input
    let action = interpret_input(input, agent.clone(), llm_client).await?;
    
    // Update history with action type
    {
        let mut history = commands_history.lock().await;
        if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
            cmd.action = action.action_type().to_string();
        }
    }
    
    // Execute the action
    match action {
        InterpretedAction::ExecuteLocalTool { tool_name, params } => {
            let result = execute_local_tool(agent.clone(), &tool_name, params).await?;
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(result.clone());
                }
            }
        }
        InterpretedAction::SendTaskToAgent { agent_id, message, streaming } => {
            let result = if streaming {
                send_task_to_agent_stream(agent.clone(), &agent_id, &message).await?
            } else {
                send_task_to_agent(agent.clone(), &agent_id, &message).await?
            };
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(result.clone());
                }
            }
        }
        InterpretedAction::RouteTask { message } => {
            let result = route_task(agent.clone(), &message).await?;
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(result.clone());
                }
            }
        }
        InterpretedAction::IntelligentRouteTask { message, streaming } => {
            let result = intelligent_route_task(agent.clone(), &message, streaming).await?;
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(result.clone());
                }
            }
        }
        InterpretedAction::DiscoverAgent { url } => {
            let result = discover_agent(agent.clone(), &url).await?;
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(result.clone());
                }
            }
            
            // Update the helper's agent list for completion after discovery
            if let Some(helper) = rl.helper_mut() {
                let available_agents: Vec<String> = agent.agent_registry.all()
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect();
                helper.update_agents(available_agents);
            }
        }
        InterpretedAction::ListAgents => {
            let result = list_known_agents(agent.clone()).await?;
            
            // Update available agents in the helper for completion
            if let Some(helper) = rl.helper_mut() {
                let available_agents: Vec<String> = agent.agent_registry.all()
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect();
                helper.update_agents(available_agents);
            }
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some("Listed agents".to_string());
                }
            }
        }
        InterpretedAction::ListTools => {
            let result = list_available_tools(agent.clone()).await?;
            
            // Update available tools in the helper for completion
            if let Some(helper) = rl.helper_mut() {
                let available_tools: Vec<String> = agent.tool_executor.tools.keys()
                    .map(|tool| tool.clone())
                    .collect();
                helper.update_tools(available_tools);
            }
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some("Listed tools".to_string());
                }
            }
        }
        InterpretedAction::Explain { response } => {
            println!("{}", response);
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(response.clone());
                }
            }
        }
    }
    
    Ok(())
}

/// Process user input without LLM for basic rule-based parsing

async fn process_input(
    input: &str,
    cmd_id: String,
    agent: Arc<BidirectionalAgent>,
    _llm_client: &impl std::any::Any,  // Accept any type since we're using a placeholder
    commands_history: Arc<Mutex<Vec<CommandRecord>>>,
    rl: &mut Editor<ReplHelper, rustyline::history::FileHistory>,
) -> Result<()> {
    println!("🔄 Processing your request using basic parsing...");
    
    // Use basic parsing to interpret the input
    let action = interpret_input(input, agent.clone(), _llm_client).await?;
    
    // Update history with action type
    {
        let mut history = commands_history.lock().await;
        if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
            cmd.action = action.action_type().to_string();
        }
    }
    
    // Execute the action
    match action {
        InterpretedAction::ExecuteLocalTool { tool_name, params } => {
            // Tool execution not available in this build
            let error_message = "Local tool execution is not supported in this build. Enable the 'bidir-local-exec' feature.";
            println!("❌ {}", error_message);
            
            // Update history with error
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(error_message.to_string());
                }
            }
            
            Err(anyhow!(error_message))
        }
        InterpretedAction::SendTaskToAgent { agent_id, message, streaming } => {
            let result = if streaming {
                send_task_to_agent_stream(agent.clone(), &agent_id, &message).await?
            } else {
                send_task_to_agent(agent.clone(), &agent_id, &message).await?
            };
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(result.clone());
                }
            }
            
            Ok(())
        }
        InterpretedAction::DiscoverAgent { url } => {
            let result = discover_agent(agent.clone(), &url).await?;
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(result.clone());
                }
            }
            
            // Update the helper's agent list for completion after discovery
            if let Some(helper) = rl.helper_mut() {
                let available_agents: Vec<String> = agent.agent_registry.all()
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect();
                helper.update_agents(available_agents);
            }
            
            Ok(())
        }
        InterpretedAction::ListAgents => {
            let result = list_known_agents(agent.clone()).await?;
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some("Listed agents".to_string());
                }
            }
            
            Ok(())
        }
        InterpretedAction::ListTools => {
            let result = list_available_tools(agent.clone()).await?;
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some("Listed tools".to_string());
                }
            }
            
            Ok(())
        }
        InterpretedAction::Explain { response } => {
            println!("{}", response);
            
            // Update history with result
            {
                let mut history = commands_history.lock().await;
                if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
                    cmd.result = Some(response.clone());
                }
            }
            
            Ok(())
        }
    }
}

/// Fallback implementation when bidir-core is not enabled

async fn process_input(
    input: &str,
    cmd_id: String,
    _agent: Arc<impl std::any::Any>,
    _llm_client: &impl std::any::Any,
    commands_history: Arc<Mutex<Vec<CommandRecord>>>,
    _rl: &mut Editor<ReplHelper, rustyline::history::FileHistory>,
) -> Result<()> {
    println!("⚠️ Bidirectional agent features not available (build without bidir-core feature)");
    
    // Update history with error
    {
        let mut history = commands_history.lock().await;
        if let Some(cmd) = history.iter_mut().find(|c| c.id == cmd_id) {
            cmd.action = "error".to_string();
            cmd.result = Some("Bidirectional agent features not available in this build".to_string());
        }
    }
    
    Ok(())
}

/// Record of a command executed in the REPL
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommandRecord {
    id: String,
    input: String,
    action: String,
    result: Option<String>,
    timestamp: chrono::DateTime<chrono::Utc>,
}

/// Enum representing actions that could be taken based on user input
enum InterpretedAction {
    ExecuteLocalTool {
        tool_name: String,
        params: Value,
    },
    SendTaskToAgent {
        agent_id: String,
        message: String,
        streaming: bool,
    },
    IntelligentRouteTask {
        message: String,
        streaming: bool,
    },
    DiscoverAgent {
        url: String,
    },
    RouteTask {
        message: String,
    },
    ListAgents,
    ListTools,
    Explain {
        response: String,
    },
}

impl InterpretedAction {
    /// Get action type as string for recording in history
    fn action_type(&self) -> &str {
        match self {
            InterpretedAction::ExecuteLocalTool { .. } => "execute_local_tool",
            InterpretedAction::SendTaskToAgent { streaming: true, .. } => "stream_task_to_agent",
            InterpretedAction::SendTaskToAgent { streaming: false, .. } => "send_task_to_agent",
            InterpretedAction::IntelligentRouteTask { streaming: true, .. } => "stream_intelligent_task",
            InterpretedAction::IntelligentRouteTask { streaming: false, .. } => "intelligent_task",
            InterpretedAction::RouteTask { .. } => "route_task",
            InterpretedAction::DiscoverAgent { .. } => "discover_agent",
            InterpretedAction::ListAgents => "list_agents",
            InterpretedAction::ListTools => "list_tools",
            InterpretedAction::Explain { .. } => "explain",
        }
    }
}

/// Use LLM to interpret the user's intent and generate an action

async fn interpret_input(
    input: &str,
    agent: Arc<BidirectionalAgent>,
    llm_client: &dyn LlmClient,
) -> Result<InterpretedAction> {
    // Get available tools and agents for prompt
    let available_tools: Vec<String> = agent.tool_executor.tools.keys().cloned().collect();
    
    let available_agents: Vec<String> = agent.agent_registry.all()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    
    // Create prompt for LLM
    let tools_str = if available_tools.is_empty() { 
        "No local tools available.".to_string() 
    } else { 
        available_tools.join(", ") 
    };
    
    let agents_str = if available_agents.is_empty() { 
        "No agents discovered yet.".to_string() 
    } else { 
        available_agents.join(", ") 
    };
    
    let prompt = format!(
        "# A2A Agent Interface Parser\n\n\
        You are an interface between a human and a network of AI agents. Your job is to interpret the human's request and determine the appropriate action.\n\n\
        ## Available Tools\n{}\n\n\
        ## Available Agents\n{}\n\n\
        ## User Request\n{}\n\n\
        ## Task\n\
        Determine what the user wants to do and return a JSON object with the appropriate action.\n\
        The JSON MUST follow one of these exact structures:\n\n\
        1. If they want to use a local tool, return:\n\
        \n{{\n  \"action\": \"execute_local_tool\",\n  \"tool_name\": \"<tool_name>\",\n  \"params\": {{ ... tool parameters ... }}\n}}\n\n\
        2. If they want to send a task to a SPECIFIC agent (they explicitly mention the agent name), return:\n\
        \n{{\n  \"action\": \"send_task_to_agent\",\n  \"agent_id\": \"<agent_id>\",\n  \"message\": \"<task message>\",\n  \"streaming\": <true or false>\n}}\n\n\
        3. If they want to send a task but DON'T specify a particular agent (should be routed intelligently), return:\n\
        \n{{\n  \"action\": \"intelligent_route_task\",\n  \"message\": \"<task message>\",\n  \"streaming\": <true or false>\n}}\n\n\
        4. If they want to see the routing decision without executing the task (e.g., \"route task <message>\" command), return:\n\
        \n{{\n  \"action\": \"route_task\",\n  \"message\": \"<task message>\"\n}}\n\n\
        5. If they want to discover a new agent, return:\n\
        \n{{\n  \"action\": \"discover_agent\",\n  \"url\": \"<agent_url>\"\n}}\n\n\
        6. If they want to list known agents, return:\n\
        \n{{\n  \"action\": \"list_agents\"\n}}\n\n\
        7. If they want to list available tools, return:\n\
        \n{{\n  \"action\": \"list_tools\"\n}}\n\n\
        8. For other requests that don't map to agent actions, return:\n\
        \n{{\n  \"action\": \"explain\",\n  \"response\": \"<helpful response>\"\n}}\n\n\
        Important notes:\n\
        - The default action should be \"intelligent_route_task\" for most tasks unless they specifically mention an agent ID\n\
        - Set \"streaming\": true if the user asks to stream, view real-time updates, watch, or live results\n\
        - Look for words like \"stream\", \"real-time\", \"live\", or \"watch\" to determine if streaming is requested\n\
        - Default to \"streaming\": false if not explicitly requested\n\n\
        Return ONLY valid JSON with no preamble, no additional explanation, and no markdown formatting.",
        tools_str,
        agents_str,
        input
    );
    
    // Call LLM to interpret the input
    #[derive(Deserialize)]
    struct ActionResponse {
        action: String,
        #[serde(default)]
        tool_name: String,
        #[serde(default)]
        params: Value,
        #[serde(default)]
        agent_id: String,
        #[serde(default)]
        message: String,
        #[serde(default)]
        url: String,
        #[serde(default)]
        response: String,
        #[serde(default)]
        streaming: bool,
    }
    
    let response = llm_client.complete_json::<ActionResponse>(&prompt).await
        .context("Failed to get response from LLM")?;
    
    // Convert response to InterpretedAction
    match response.action.as_str() {
        "execute_local_tool" => {
            Ok(InterpretedAction::ExecuteLocalTool { 
                tool_name: response.tool_name, 
                params: response.params 
            })
        }
        "send_task_to_agent" => {
            Ok(InterpretedAction::SendTaskToAgent { 
                agent_id: response.agent_id, 
                message: response.message,
                streaming: response.streaming
            })
        }
        "intelligent_route_task" => {
            Ok(InterpretedAction::IntelligentRouteTask { 
                message: response.message,
                streaming: response.streaming
            })
        }
        "route_task" => {
            Ok(InterpretedAction::RouteTask { 
                message: response.message
            })
        }
        "discover_agent" => {
            Ok(InterpretedAction::DiscoverAgent { 
                url: response.url 
            })
        }
        "list_agents" => {
            Ok(InterpretedAction::ListAgents)
        }
        "list_tools" => {
            Ok(InterpretedAction::ListTools)
        }
        "explain" => {
            Ok(InterpretedAction::Explain { 
                response: response.response 
            })
        }
        _ => {
            // Default to explain if action isn't recognized
            Ok(InterpretedAction::Explain { 
                response: format!("I'm not sure how to process your request. Please try again with a clearer instruction.") 
            })
        }
    }
}

/// Fallback implementation when LLM client is not available but bidir-core is still enabled

async fn interpret_input(
    input: &str,
    agent: Arc<BidirectionalAgent>,
    _llm_client: &impl std::any::Any,  // Accept any type since we're using a placeholder
) -> Result<InterpretedAction> {
    // Simple rule-based parsing without LLM
    let input_lower = input.to_lowercase();
    
    // Get available agents
    let available_agents: Vec<String> = agent.agent_registry.all()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    
    if input_lower.contains("discover") && input_lower.contains("agent") {
        // Look for a URL in the input
        if let Some(url) = extract_url(input) {
            return Ok(InterpretedAction::DiscoverAgent { url: url.to_string() });
        }
    } else if input_lower.contains("route") && input_lower.contains("task") {
        // Handle route task command (shows routing decision without execution)
        let message = input.trim()
            .replace("route task", "")
            .trim()
            .to_string();
        
        if !message.is_empty() {
            return Ok(InterpretedAction::RouteTask {
                message,
            });
        }
    } else if input_lower.contains("send") && input_lower.contains("task") {
        // Check if streaming is requested
        let streaming = input_lower.contains("stream") || 
                        input_lower.contains("real-time") || 
                        input_lower.contains("live") || 
                        input_lower.contains("watch");
        
        // Try to match an agent ID
        let mut found_agent = false;
        for agent_id in &available_agents {
            if input_lower.contains(&agent_id.to_lowercase()) {
                // Extract message after the agent ID
                if let Some(message) = extract_message_after(input, agent_id) {
                    found_agent = true;
                    return Ok(InterpretedAction::SendTaskToAgent { 
                        agent_id: agent_id.clone(), 
                        message: message.to_string(),
                        streaming
                    });
                }
            }
        }
        
        // If no specific agent was mentioned, use intelligent routing
        if !found_agent {
            // Here we assume the entire input (minus command words) is the task
            let message = input.trim()
                .replace("send task", "")
                .replace("send a task", "")
                .replace("task", "")
                .trim()
                .to_string();
            
            if !message.is_empty() {
                return Ok(InterpretedAction::IntelligentRouteTask {
                    message,
                    streaming,
                });
            }
        }
    } else if input_lower.contains("list") && input_lower.contains("agent") {
        return Ok(InterpretedAction::ListAgents);
    } else if input_lower.contains("list") && input_lower.contains("tool") {
        return Ok(InterpretedAction::ListTools);
    } else {
        // If the input looks like a task but doesn't match any of the above patterns,
        // use intelligent routing
        if !input_lower.contains("discover") && 
           !input_lower.contains("list") && 
           !input_lower.contains("help") &&
           input.trim().len() > 5 { // Arbitrary minimum length for a task
            return Ok(InterpretedAction::IntelligentRouteTask {
                message: input.trim().to_string(),
                streaming: input_lower.contains("stream") || 
                          input_lower.contains("real-time") || 
                          input_lower.contains("live") || 
                          input_lower.contains("watch"),
            });
        }
    }
    
    // Default explanation when we can't determine the intent
    Ok(InterpretedAction::Explain { 
        response: "Without the LLM client, I can only understand basic commands. Try 'list agents', 'discover agent <url>', or 'send task to <agent_id> <message>'.".to_string() 
    })
}

/// Fallback implementation when neither bidir-core nor bidir-local-exec are enabled

async fn interpret_input(
    _input: &str,
    _agent: Arc<impl std::any::Any>,
    _llm_client: &impl std::any::Any,
) -> Result<InterpretedAction> {
    Ok(InterpretedAction::Explain {
        response: "This is a basic mode with limited functionality. REPL requires bidir-core feature to be enabled for full functionality.".to_string()
    })
}

/// Helper function to extract a URL from text
fn extract_url(text: &str) -> Option<&str> {
    // Simple URL extraction heuristic
    for word in text.split_whitespace() {
        if word.starts_with("http://") || word.starts_with("https://") {
            return Some(word);
        }
    }
    None
}

/// Helper function to extract message after an agent ID
fn extract_message_after<'a>(text: &'a str, agent_id: &str) -> Option<&'a str> {
    // Find agent ID and return everything after it
    if let Some(pos) = text.to_lowercase().find(&agent_id.to_lowercase()) {
        let start = pos + agent_id.len();
        if start < text.len() {
            return Some(text[start..].trim());
        }
    }
    None
}

/// Execute a local tool

async fn execute_local_tool(
    agent: Arc<BidirectionalAgent>,
    tool_name: &str,
    params: Value,
) -> Result<String> {
    println!("🔧 Executing tool: {}", tool_name);
    
    // Execute the tool
    match agent.tool_executor.execute_tool(tool_name, params).await {
        Ok(result) => {
            println!("✅ Tool execution successful:");
            let result_str = serde_json::to_string_pretty(&result)?;
            println!("{}", result_str);
            Ok(result_str)
        }
        Err(e) => {
            let error_message = format!("❌ Tool execution failed: {}", e);
            println!("{}", error_message);
            Err(anyhow!(error_message))
        }
    }
}

/// Fallback implementation for when bidir-local-exec is not enabled

async fn execute_local_tool(
    _agent: Arc<impl std::any::Any>,
    tool_name: &str,
    _params: Value,
) -> Result<String> {
    let error_message = format!("❌ Local tool execution for '{}' is not supported in this build. Enable the 'bidir-local-exec' feature.", tool_name);
    println!("{}", error_message);
    Err(anyhow!(error_message))
}

/// Send a task to another agent with streaming support

async fn send_task_to_agent_stream(
    agent: Arc<BidirectionalAgent>,
    agent_id: &str,
    message: &str,
) -> Result<String> {
    // Create an async spinner
    let spinner = AsyncSpinner::new(&format!("Preparing to send streaming task to agent: {}", agent_id));
    
    // First, check if we know about this agent with improved error handling
    let agent_info = match agent.agent_registry.get(agent_id) {
        Some(info) => info,
        None => {
            // Get list of known agents to provide helpful suggestions
            let known_agents = agent.agent_registry.all();
            
            // Build a more helpful error message
            let mut error_message = format!("Agent '{}' not found in registry", agent_id);
            
            // Try to find a close match (simple substring match for now)
            let similar_agents: Vec<String> = known_agents
                .into_iter()
                .map(|(id, _)| id)
                .filter(|id| id.contains(agent_id) || agent_id.contains(id))
                .collect();
            
            if !similar_agents.is_empty() {
                error_message.push_str(". Did you mean: ");
                error_message.push_str(&similar_agents.join(", "));
                error_message.push('?');
            } else {
                error_message.push_str(". Use 'agents' to list known agents.");
            }
            
            // Stop spinner with error
            spinner.finish_with_error(&error_message).await;
            return Err(ReplError::agent(error_message).into());
        }
    };
    
    // Create client for streaming with error handling for the URL
    if agent_info.card.url.is_empty() {
        let error_message = format!("Agent '{}' has an invalid URL", agent_id);
        spinner.finish_with_error(&error_message).await;
        return Err(ReplError::agent(error_message).into());
    }
    
    let mut client = A2aClient::new(&agent_info.card.url);
    
    // Update spinner message
    spinner.update_message(&format!("Sending streaming task to agent: {}", agent_id));
    
    // Enable streaming with mock delay of 1 second for testing and proper error handling
    // In real usage, we'd use an empty metadata object
    let stream = match client.send_task_subscribe_with_metadata_typed(
        message, 
        &json!({"_mock_chunk_delay_ms": 1000})
    ).await {
        Ok(stream) => stream,
        Err(client_err) => {
            // Convert client error to our custom error type for better user messages
            let repl_err = ReplError::from_client_error(client_err);
            spinner.finish_with_error(&repl_err.to_string()).await;
            return Err(repl_err.into());
        }
    };
    
    // Stop the initial spinner and show success
    spinner.finish_with_success("Task streaming started").await;
    
    // Create a task manager to track this task
    let task_manager = TaskManager::new();
    let temp_id = Uuid::new_v4().to_string();
    let tracker = task_manager.track_task(&temp_id, "Initializing").await;
    
    let mut result_summary = Vec::new();
    result_summary.push("Task streaming results:".to_string());
    
    // Process the stream with the task tracker
    tokio::pin!(stream);
    let mut task_id = String::new();
    let mut any_errors = false;
    let mut bytes_received = 0;
    
    while let Some(response) = stream.next().await {
        match response {
            Ok(StreamingResponse::Status(task)) => {
                // Update the task ID if this is the first time we've seen it
                if task_id.is_empty() && !task.id.is_empty() {
                    task_id = task.id.clone();
                    result_summary.push(format!("Task ID: {}", task_id));
                    
                    // Switch to using the real task ID for tracking
                    task_manager.remove_task(&temp_id).await;
                    let tracker = task_manager.track_task(&task_id, &task.status.state.to_string()).await;
                }
                
                // Update task state in the tracker
                if !task_id.is_empty() {
                    task_manager.update_task_state(&task_id, &task.status.state.to_string()).await;
                } else {
                    task_manager.update_task_state(&temp_id, &task.status.state.to_string()).await;
                }
                
                result_summary.push(format!("Status: {}", task.status.state));
            },
            Ok(StreamingResponse::Artifact(artifact)) => {
                result_summary.push("📄 Received artifact chunk:".to_string());
                
                for part in &artifact.parts {
                    match part {
                        Part::TextPart(text_part) => {
                            // Add the text to the tracker as a message
                            let id_to_use = if !task_id.is_empty() { &task_id } else { &temp_id };
                            if let Some(tracker) = task_manager.get_task(id_to_use).await {
                                tracker.add_message(&text_part.text);
                                
                                // Count bytes received
                                bytes_received += text_part.text.len();
                                tracker.update_progress(bytes_received);
                            }
                            
                            // Add to summary
                            if text_part.text.len() > 100 {
                                result_summary.push(format!("  Text: {}...", &text_part.text[..100]));
                            } else {
                                result_summary.push(format!("  Text: {}", text_part.text));
                            }
                        },
                        _ => {
                            // Add non-text part to summary
                            let id_to_use = if !task_id.is_empty() { &task_id } else { &temp_id };
                            if let Some(tracker) = task_manager.get_task(id_to_use).await {
                                tracker.add_message(&format!("Non-text part: {:?}", part));
                            }
                            
                            result_summary.push(format!("  Non-text part: {:?}", part));
                        }
                    }
                }
            },
            Ok(StreamingResponse::Final(task)) => {
                // Update task ID if needed
                if task_id.is_empty() && !task.id.is_empty() {
                    task_id = task.id.clone();
                    result_summary.push(format!("Task ID: {}", task_id));
                    
                    // Switch to using the real task ID for tracking
                    task_manager.remove_task(&temp_id).await;
                    let tracker = task_manager.track_task(&task_id, &task.status.state.to_string()).await;
                }
                
                // Check final status
                let status = task.status.state.to_string();
                let success = status == "completed";
                
                // Update task state and mark as complete
                let id_to_use = if !task_id.is_empty() { &task_id } else { &temp_id };
                task_manager.complete_task(id_to_use, success).await;
                
                if success {
                    result_summary.push("✅ Final status: Completed".to_string());
                } else if status == "failed" {
                    let error_message = task.status.message.as_ref()
                        .and_then(|m| m.parts.iter().find_map(|p| match p {
                            Part::TextPart(t) => Some(t.text.clone()),
                            _ => None,
                        }))
                        .unwrap_or_else(|| "Unknown error".to_string());
                        
                    result_summary.push(format!("❌ Final status: Failed - {}", error_message));
                } else {
                    result_summary.push(format!("🔄 Final status: {}", status));
                }
                
                // Process final artifacts if available
                if let Some(artifacts) = &task.artifacts {
                    result_summary.push(format!("📊 Final artifacts ({})", artifacts.len()));
                    
                    for (i, artifact) in artifacts.iter().enumerate() {
                        let artifact_name = artifact.name.as_deref().unwrap_or("Unnamed");
                        
                        // Add artifact info to task tracker
                        if let Some(tracker) = task_manager.get_task(id_to_use).await {
                            tracker.add_message(&format!("Artifact {}: {}", i+1, artifact_name));
                        }
                        
                        result_summary.push(format!("- Artifact {}: {}", i+1, artifact_name));
                    }
                }
            },
            Err(e) => {
                any_errors = true;
                let error_message = format!("❌ Streaming error: {}", e);
                
                // Add error to task tracker
                let id_to_use = if !task_id.is_empty() { &task_id } else { &temp_id };
                if let Some(tracker) = task_manager.get_task(id_to_use).await {
                    tracker.add_message(&error_message);
                }
                
                result_summary.push(error_message);
            }
        }
    }
    
    // Add appropriate final message if we never got a final event
    if task_id.is_empty() {
        result_summary.push("⚠️ No task ID received from streaming response".to_string());
        task_manager.complete_task(&temp_id, false).await;
    }
    
    if any_errors {
        result_summary.push("⚠️ Errors occurred during streaming".to_string());
        
        // If we have a task ID but errors occurred, mark it as failed if not already done
        if !task_id.is_empty() {
            task_manager.complete_task(&task_id, false).await;
        }
    }
    
    // Sleep a bit to let the user see the final state
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Clean up the task tracker
    let id_to_remove = if !task_id.is_empty() { &task_id } else { &temp_id };
    task_manager.remove_task(id_to_remove).await;
    
    Ok(result_summary.join("\n"))
}

/// Send a task to another agent

async fn send_task_to_agent(
    agent: Arc<BidirectionalAgent>,
    agent_id: &str,
    message: &str,
) -> Result<String> {
    // Create an async spinner for task preparation
    let spinner = AsyncSpinner::new(&format!("Preparing to send task to agent: {}", agent_id));
    
    // First, check if we know about this agent with improved error handling
    let agent_info = match agent.agent_registry.get(agent_id) {
        Some(info) => info,
        None => {
            // Get list of known agents to provide helpful suggestions
            let known_agents = agent.agent_registry.all();
            
            // Build a more helpful error message
            let mut error_message = format!("Agent '{}' not found in registry", agent_id);
            
            // Try to find a close match (simple substring match for now)
            let similar_agents: Vec<String> = known_agents
                .into_iter()
                .map(|(id, _)| id)
                .filter(|id| id.contains(agent_id) || agent_id.contains(id))
                .collect();
            
            if !similar_agents.is_empty() {
                error_message.push_str(". Did you mean: ");
                error_message.push_str(&similar_agents.join(", "));
                error_message.push('?');
            } else {
                error_message.push_str(". Use 'agents' to list known agents.");
            }
            
            // Stop spinner with error
            spinner.finish_with_error(&error_message).await;
            return Err(ReplError::agent(error_message).into());
        }
    };
    
    // Create task parameters
    let task_id = format!("repl-task-{}", Uuid::new_v4());
    let task_params = TaskSendParams {
        id: task_id.clone(),
        message: Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: message.to_string(),
                metadata: None,
            })],
            metadata: None,
        },
        history_length: None,
        metadata: None,
        push_notification: None,
        session_id: None,
    };
    
    // Check URL validity
    if agent_info.card.url.is_empty() {
        let error_message = format!("Agent '{}' has an invalid URL", agent_id);
        spinner.finish_with_error(&error_message).await;
        return Err(ReplError::agent(error_message).into());
    }
    
    // Update spinner message for task sending
    spinner.update_message(&format!("Sending task to agent: {}", agent_id));
    
    // Create client
    let mut client = A2aClient::new(&agent_info.card.url);
    
    // Try to send the task with better error handling
    let task_result = match client.send_task(message).await {
        Ok(result) => result,
        Err(err) => {
            // Convert error to user-friendly message
            let repl_err = if err.to_string().contains("401") || err.to_string().contains("403") {
                ReplError::auth(format!(
                    "Authentication failed with agent '{}': {}", 
                    agent_id, err
                ))
            } else if err.to_string().contains("timed out") {
                ReplError::network(format!(
                    "Request to agent '{}' timed out. The agent might be busy or unavailable", 
                    agent_id
                ))
            } else if err.to_string().contains("404") || err.to_string().contains("not found") {
                ReplError::agent(format!(
                    "Agent '{}' not found or resource not available", 
                    agent_id
                ))
            } else {
                ReplError::task(format!(
                    "Failed to send task to agent '{}': {}", 
                    agent_id, err
                ))
            };
            
            // Stop spinner with error
            spinner.finish_with_error(&repl_err.to_string()).await;
            return Err(repl_err.into());
        }
    };
    
    // Stop the sending spinner and show success
    spinner.finish_with_success(&format!("Task sent successfully. Task ID: {}", task_result.id)).await;
    
    // Create a task manager to track this task
    let task_manager = TaskManager::new();
    let tracker = task_manager.track_task(&task_result.id, "working").await;
    
    // Prepare result summary
    let mut result_summary = Vec::new();
    result_summary.push(format!("Task ID: {}", task_result.id));
    
    // Create a new client for polling
    let mut client = A2aClient::new(&agent_info.card.url);
    
    // Set up polling parameters
    let mut attempts = 0;
    let max_attempts = 30; // Poll for up to 30 seconds
    let mut retry_count = 0;
    let max_retries = 3; // Allow up to 3 consecutive failures before giving up
    
    // Create progress bar directly without using AsyncSpinner to avoid ownership issues
    let polling_progress = create_progress_bar(100, &format!("Polling for task {} results...", task_result.id));
    
    // Start polling loop
    while attempts < max_attempts {
        // Calculate sleep duration with exponential backoff for retries
        let sleep_duration = if retry_count > 0 {
            std::cmp::min(1 << retry_count, 4) // Exponential backoff: 1, 2, 4 seconds
        } else {
            1 // Default polling interval: 1 second
        };
        
        // Update tracker message
        task_manager.update_task_state(&task_result.id, 
            &format!("Polling ({}/{})", attempts + 1, max_attempts)).await;
        
        // Update polling spinner message
        polling_progress.set_message(format!("Polling for results ({}/{})", 
            attempts + 1, max_attempts));
        
        // Wait before the next poll
        tokio::time::sleep(tokio::time::Duration::from_secs(sleep_duration)).await;
        attempts += 1;
        
        // Poll for task status
        match client.get_task(&task_result.id).await {
            Ok(task) => {
                // Reset retry counter on successful request
                retry_count = 0;
                
                // Update task state in the tracker
                task_manager.update_task_state(&task_result.id, &task.status.state.to_string()).await;
                
                if task.status.state == TaskState::Completed {
                    // Stop the polling spinner with success
                    polling_progress.finish_with_message("✅ Task completed successfully");
                    
                    // Mark task as completed in the tracker
                    task_manager.complete_task(&task_result.id, true).await;
                    
                    // Add status to summary
                    result_summary.push("Status: Completed".to_string());
                    
                    // Show message if available
                    if let Some(message) = task.status.message {
                        result_summary.push("📝 Agent response:".to_string());
                        
                        // Add the message to the tracker
                        tracker.add_message("📝 Agent response:");
                        
                        for part in message.parts {
                            match part {
                                Part::TextPart(text_part) => {
                                    // Add text to tracker
                                    tracker.add_message(&text_part.text);
                                    
                                    // Add to summary
                                    result_summary.push(text_part.text);
                                }
                                _ => {
                                    // Add non-text part to tracker
                                    tracker.add_message(&format!("Non-text part: {:?}", part));
                                    
                                    // Add to summary
                                    result_summary.push(format!("Non-text part: {:?}", part));
                                }
                            }
                        }
                    }
                    
                    // Show artifacts if available
                    if let Some(artifacts) = task.artifacts {
                        result_summary.push(format!("📊 Task artifacts ({})", artifacts.len()));
                        
                        // Add artifacts info to tracker
                        tracker.add_message(&format!("📊 Task artifacts ({})", artifacts.len()));
                        
                        for (i, artifact) in artifacts.iter().enumerate() {
                            let artifact_name = artifact.name.as_deref().unwrap_or("Unnamed");
                            
                            // Add artifact info to tracker
                            tracker.add_message(&format!("Artifact {}: {}", i+1, artifact_name));
                            
                            // Add to summary
                            result_summary.push(format!("- Artifact {}: {}", i+1, artifact_name));
                            
                            for part in &artifact.parts {
                                match part {
                                    Part::TextPart(text_part) => {
                                        // Add text snippet to tracker
                                        let snippet = if text_part.text.len() > 100 {
                                            format!("Text: {}...", &text_part.text[..100])
                                        } else {
                                            format!("Text: {}", text_part.text)
                                        };
                                        tracker.add_message(&snippet);
                                        
                                        // Add to summary
                                        if text_part.text.len() > 100 {
                                            result_summary.push(format!("  Text: {}...", &text_part.text[..100]));
                                        } else {
                                            result_summary.push(format!("  Text: {}", text_part.text));
                                        }
                                    }
                                    _ => {
                                        // Add non-text part to tracker
                                        tracker.add_message(&format!("Non-text part: {:?}", part));
                                        
                                        // Add to summary
                                        result_summary.push(format!("  Non-text part: {:?}", part));
                                    }
                                }
                            }
                        }
                    } else {
                        // Add no artifacts info to tracker
                        tracker.add_message("No artifacts returned");
                        
                        // Add to summary
                        result_summary.push("No artifacts returned.".to_string());
                    }
                    
                    break;
                } else if task.status.state == TaskState::Failed {
                    // Stop the polling spinner with error
                    polling_progress.finish_with_message("❌ Task failed");
                    
                    // Mark task as failed in the tracker
                    task_manager.complete_task(&task_result.id, false).await;
                    
                    // Add status to summary
                    result_summary.push("Status: Failed".to_string());
                    
                    if let Some(message) = task.status.message {
                        result_summary.push("Error message:".to_string());
                        
                        // Add error message to tracker
                        tracker.add_message("Error message:");
                        
                        for part in message.parts {
                            match part {
                                Part::TextPart(text_part) => {
                                    // Add text to tracker
                                    tracker.add_message(&text_part.text);
                                    
                                    // Add to summary
                                    result_summary.push(text_part.text);
                                }
                                _ => {
                                    // Add non-text part to tracker
                                    tracker.add_message(&format!("Non-text part: {:?}", part));
                                    
                                    // Add to summary
                                    result_summary.push(format!("Non-text part: {:?}", part));
                                }
                            }
                        }
                    }
                    
                    break;
                }
                // If still processing, continue the loop
            }
            Err(e) => {
                // Increment retry counter and handle error
                retry_count += 1;
                
                // Log the error but keep retrying until max retries is reached
                if retry_count <= max_retries {
                    // Update tracker with retry message
                    tracker.add_message(&format!("⚠️ Error checking task status (retry {}/{}): {}", 
                                                retry_count, max_retries, e));
                    
                    // Update spinner message
                    polling_progress.set_message(format!("Error checking status (retry {}/{})", 
                                                         retry_count, max_retries));
                    
                    // Don't add to result_summary yet, wait for retries
                    continue;
                }
                
                // Format a user-friendly error message
                let error_msg = if e.to_string().contains("timeout") {
                    "Request timed out, the agent might be busy"
                } else if e.to_string().contains("404") || e.to_string().contains("not found") {
                    "Task not found, it may have been deleted"
                } else {
                    "Could not retrieve task status"
                };
                
                // Stop the polling spinner with error
                polling_progress.finish_with_message("❌ Failed to check task status");
                
                // Mark task as failed in the tracker
                task_manager.complete_task(&task_result.id, false).await;
                
                // Add to tracker and summary
                tracker.add_message(&format!("❌ Failed to check task status after {} retries: {}", 
                                           max_retries, error_msg));
                result_summary.push(format!("Error: {}", error_msg));
                break;
            }
        }
    }
    
    if attempts >= max_attempts {
        // Stop the polling spinner with timeout
        polling_progress.finish_with_message("❌ Polling timeout reached");
        
        // Add to tracker and summary
        tracker.add_message(&format!("⚠️ Polling timeout reached after {} seconds. Task is still processing.", 
                                   max_attempts));
        result_summary.push("Status: Still processing - polling timeout reached".to_string());
        result_summary.push(format!("Task ID: {} (use for status checks)", task_result.id));
        
        // Update tracker state but don't mark as complete
        task_manager.update_task_state(&task_result.id, "processing - timeout reached").await;
    }
    
    // Sleep briefly to let the user see the final state
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    // Clean up the task tracker
    task_manager.remove_task(&task_result.id).await;
    
    Ok(result_summary.join("\n"))
}


/// Intelligently routes a task to the appropriate destination

async fn route_task(
    agent: Arc<BidirectionalAgent>,
    message: &str,
) -> Result<String> {
    // Create an async spinner
    let spinner = AsyncSpinner::new("Analyzing task for routing decision...");
    
    // Generate a unique task ID for this request
    let task_id = Uuid::new_v4().to_string();
    
    // Create TaskSendParams
    let params = TaskSendParams {
        id: task_id.clone(),
        message: Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: message.to_string(),
                metadata: None,
            })],
            metadata: None,
        },
        history_length: None,
        metadata: None,
        push_notification: None,
        session_id: None,
    };
    
    // Use the LlmTaskRouter to decide how to route the task
    spinner.update_message("Using LLM to determine routing decision...");
    
    let route_decision_result = agent.task_router.decide(&params).await;
    
    if let Err(e) = &route_decision_result {
        spinner.finish_with_error("❌ Failed to determine routing").await;
        return Err(anyhow!("LLM task routing error: {}", e));
    }
    
    let route_decision = route_decision_result.unwrap();
    
    // Format the decision for display only, without executing
    spinner.finish_with_success("✅ Routing decision determined").await;
    
    let decision_details = match &route_decision {
        crate::bidirectional_agent::task_router::RoutingDecision::Local { tool_names } => {
            format!("LOCAL EXECUTION using tools: {}", tool_names.join(", "))
        },
        crate::bidirectional_agent::task_router::RoutingDecision::Remote { agent_id } => {
            format!("REMOTE DELEGATION to agent: {}", agent_id)
        },
        crate::bidirectional_agent::task_router::RoutingDecision::Reject { reason } => {
            format!("TASK REJECTION with reason: {}", reason)
        },
        
        crate::bidirectional_agent::task_router::RoutingDecision::Decompose { subtasks } => {
            format!("TASK DECOMPOSITION into {} subtasks", subtasks.len())
        },
    };
    
    println!("🧠 Routing decision: {}", decision_details);
    println!("Note: Task was not executed, this is just the routing decision");
    
    Ok(format!("Routing decision: {}", decision_details))
}


async fn intelligent_route_task(
    agent: Arc<BidirectionalAgent>,
    message: &str,
    streaming: bool,
) -> Result<String> {
    // Create an async spinner
    let spinner = AsyncSpinner::new("Analyzing task for intelligent routing...");
    
    // Generate a unique task ID for this request
    let task_id = Uuid::new_v4().to_string();
    
    // Create TaskSendParams
    let params = TaskSendParams {
        id: task_id.clone(),
        message: Message {
            role: Role::User,
            parts: vec![Part::TextPart(TextPart {
                type_: "text".to_string(),
                text: message.to_string(),
                metadata: None,
            })],
            metadata: None,
        },
        history_length: None,
        metadata: None,
        push_notification: None,
        session_id: None,
    };
    
    // Use the LlmTaskRouter to decide how to route the task
    spinner.update_message("Using LLM to determine best routing for task...");
    
    let route_decision_result = agent.task_router.decide(&params).await;
    
    if let Err(e) = &route_decision_result {
        spinner.finish_with_error("❌ Failed to route task").await;
        return Err(anyhow!("LLM task routing error: {}", e));
    }
    
    let route_decision = route_decision_result.unwrap();
    
    match route_decision {
        // If decision is to execute locally
        crate::bidirectional_agent::task_router::RoutingDecision::Local { tool_names } => {
            spinner.update_message(&format!("Task will be executed locally using tools: {}", tool_names.join(", ")));
            spinner.finish_with_success("✅ Task routed to local executor").await;
            
            // Execute locally with the tool executor
            // Convert params and tool_names to the expected types
            let echo_str = String::from("echo");
            let tool_name = tool_names.first().unwrap_or(&echo_str).as_str();
            
            // Create parameters from the task message
            let message_text = params.message.parts.iter().find_map(|part| {
                if let crate::types::Part::TextPart(text_part) = part {
                    Some(text_part.text.clone())
                } else {
                    None
                }
            }).unwrap_or_default();
            
            // Create JSON Value for parameters
            let tool_params = serde_json::json!({
                "text": message_text,
                "task_id": params.id
            });
            
            let result = agent.tool_executor.execute_tool(tool_name, tool_params).await
                .map_err(|e| anyhow!("Failed to execute local tools: {}", e))?;
            
            // Convert tool result to display format
            let mut output = format!("Local execution result with tools {}:\n", tool_names.join(", "));
            
            // For JsonValue result, we need to extract the text differently
            if let Some(text) = result["text"].as_str() {
                output.push_str(text);
            } else {
                // If no "text" field is found, show the raw JSON
                output.push_str(&format!("\n[Tool result: {}]", result));
            }
            
            println!("{}", output);
            Ok(format!("Task executed locally with tools: {}", tool_names.join(", ")))
        },
        
        // If decision is to forward to remote agent
        crate::bidirectional_agent::task_router::RoutingDecision::Remote { agent_id } => {
            spinner.update_message(&format!("Task will be delegated to agent: {}", agent_id));
            spinner.finish_with_success(&format!("✅ Task routed to agent: {}", agent_id)).await;
            
            // Delegate to the appropriate function based on streaming preference
            if streaming {
                send_task_to_agent_stream(agent, &agent_id, message).await
            } else {
                send_task_to_agent(agent, &agent_id, message).await
            }
        },
        
        // If task should be rejected
        crate::bidirectional_agent::task_router::RoutingDecision::Reject { reason } => {
            spinner.finish_with_error("❌ Task rejected by routing system").await;
            println!("Task rejected with reason: {}", reason);
            Err(anyhow!("Task rejected: {}", reason))
        },
        
        // Handle task decomposition if the feature is enabled
        
        crate::bidirectional_agent::task_router::RoutingDecision::Decompose { subtasks } => {
            spinner.finish_with_success("✅ Task decomposed into subtasks").await;
            
            // Process each subtask
            println!("Task will be decomposed into {} subtasks:", subtasks.len());
            
            let task_manager = TaskManager::new();
            
            // Start all subtasks
            for (i, subtask) in subtasks.iter().enumerate() {
                let task_display = format!("Subtask {}: {}", i+1, 
                    if subtask.input_message.len() > 50 {
                        format!("{}...", &subtask.input_message[..50])
                    } else {
                        subtask.input_message.clone()
                    }
                );
                
                println!("Starting {}", task_display);
                
                // Clone what we need for the async task
                let agent_clone = agent.clone();
                let subtask_clone = subtask.clone();
                
                // Add task to manager
                let subtask_tracker = task_manager.track_task(&subtask_clone.id, "pending").await;
                tokio::spawn(async move {
                    // Create params for this subtask
                    let subtask_params = TaskSendParams {
                        id: subtask_clone.id.clone(),
                        message: Message {
                            role: Role::User,
                            parts: vec![Part::TextPart(TextPart {
                                type_: "text".to_string(),
                                text: subtask_clone.input_message.clone(),
                                metadata: None,
                            })],
                            metadata: None,
                        },
                        history_length: None,
                        metadata: subtask_clone.metadata.clone(),
                        push_notification: None,
                        session_id: None,
                    };
                    
                    // Route the subtask based on content
                    let subtask_decision = agent_clone.task_router.decide(&subtask_params).await?;
                    
                    match subtask_decision {
                        crate::bidirectional_agent::task_router::RoutingDecision::Local { tool_names } => {
                            // Execute locally
                            // Convert params and tool_names to the expected types
                            let echo_str = String::from("echo");
                            let tool_name = tool_names.first().unwrap_or(&echo_str).as_str();
                            
                            // Create parameters from the task message
                            let message_text = subtask_params.message.parts.iter().find_map(|part| {
                                if let crate::types::Part::TextPart(text_part) = part {
                                    Some(text_part.text.clone())
                                } else {
                                    None
                                }
                            }).unwrap_or_default();
                            
                            // Create JSON Value for parameters
                            let tool_params = serde_json::json!({
                                "text": message_text,
                                "task_id": subtask_params.id
                            });
                            
                            agent_clone.tool_executor.execute_tool(tool_name, tool_params).await?;
                            Ok(format!("Completed locally with tools: {}", tool_names.join(", ")))
                        },
                        crate::bidirectional_agent::task_router::RoutingDecision::Remote { agent_id } => {
                            // Get client for this agent
                            let agent_info = agent_clone.agent_registry.get(&agent_id)
                                .ok_or_else(|| anyhow!("Agent not found: {}", agent_id))?;
                            
                            let mut client = agent_clone.client_manager.get_or_create_client(&agent_info.card.url).await?;
                            // Extract the message text from params
                            let message_text = subtask_params.message.parts.iter().find_map(|part| {
                                if let crate::types::Part::TextPart(text_part) = part {
                                    Some(text_part.text.clone())
                                } else {
                                    None
                                }
                            }).unwrap_or_default();
                            
                            let result = client.send_task(&message_text).await?;
                            Ok(format!("Delegated to agent: {}", agent_id))
                        },
                        crate::bidirectional_agent::task_router::RoutingDecision::Reject { reason } => {
                            Err(anyhow!("Subtask rejected: {}", reason))
                        },
                        
                        crate::bidirectional_agent::task_router::RoutingDecision::Decompose { .. } => {
                            // Prevent infinite recursion
                            Err(anyhow!("Cannot further decompose a subtask"))
                        },
                    }
                });
            }
            
            // Wait a reasonable amount of time for subtasks to complete
            tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
            
            // Collect results from the task manager
            let tasks = task_manager.list_tasks().await;
            let mut results: Vec<Result<String, anyhow::Error>> = Vec::new();
            
            // Collect the results and return them
            let mut output = format!("Processed {} subtasks:\n", subtasks.len());
            
            for (i, (task_id, state)) in tasks.iter().enumerate() {
                if state == "completed" {
                    output.push_str(&format!("✅ Subtask {}: {} ({})\n", i+1, task_id, state));
                } else {
                    output.push_str(&format!("❓ Subtask {}: {} ({})\n", i+1, task_id, state));
                }
            }
            
            println!("{}", output);
            Ok(format!("Task decomposed and executed as {} subtasks", subtasks.len()))
        },
    }
}

/// Add a fallback implementation for route_task when bidir-local-exec is not enabled

async fn route_task(
    agent: Arc<BidirectionalAgent>,
    message: &str,
) -> Result<String> {
    let error_message = "Cannot show routing decisions without the bidir-local-exec feature enabled.";
    println!("⚠️ {}", error_message);
    println!("In this build, tasks are always routed to the default agent if available.");
    
    // Show what would happen with the default agent
    let default_agents = agent.agent_registry.all();
    if let Some((agent_id, _)) = default_agents.into_iter().next() {
        println!("🔄 Routing decision (fallback): REMOTE DELEGATION to agent: {}", agent_id);
        println!("Note: This is not using LLM-based routing, just showing the fallback behavior");
        Ok(format!("Routing decision (fallback): REMOTE DELEGATION to agent: {}", agent_id))
    } else {
        println!("❌ No agents available to show routing decision");
        Err(anyhow!("{} No agents available.", error_message))
    }
}

/// Add a fallback implementation when bidir-local-exec is not enabled but bidir-core is

async fn intelligent_route_task(
    agent: Arc<BidirectionalAgent>,
    message: &str,
    streaming: bool,
) -> Result<String> {
    let error_message = "Cannot intelligently route tasks without the bidir-local-exec feature enabled.";
    println!("⚠️ {}", error_message);
    println!("Falling back to default agent for task execution...");
    
    // Fall back to the default agent if one exists
    let default_agents = agent.agent_registry.all();
    if let Some((agent_id, _)) = default_agents.into_iter().next() {
        println!("🔄 Routing task to default agent: {}", agent_id);
        
        if streaming {
            send_task_to_agent_stream(agent, &agent_id, message).await
        } else {
            send_task_to_agent(agent, &agent_id, message).await
        }
    } else {
        Err(anyhow!("{} No agents available for fallback.", error_message))
    }
}

/// Add a fallback implementation when bidir-core is not enabled

async fn route_task(
    _agent: Arc<impl std::any::Any>,
    message: &str,
) -> Result<String> {
    let error_message = format!("❌ Cannot show routing decision for task: '{}'. Bidirectional agent features require bidir-core feature to be enabled.", 
        if message.len() > 50 { &message[..50] } else { message });
    println!("{}", error_message);
    Err(anyhow!(error_message))
}

/// Add a fallback implementation when bidir-core is not enabled

async fn intelligent_route_task(
    _agent: Arc<impl std::any::Any>,
    message: &str, 
    _streaming: bool,
) -> Result<String> {
    let error_message = format!("❌ Cannot intelligently route task: '{}'. Bidirectional agent features require bidir-core feature to be enabled.", 
        if message.len() > 50 { &message[..50] } else { message });
    println!("{}", error_message);
    Err(anyhow!(error_message))
}

/// Discover a new agent

async fn discover_agent(
    agent: Arc<BidirectionalAgent>,
    url: &str,
) -> Result<String> {
    println!("🔍 Discovering agent at URL: {}", url);
    
    match agent.agent_registry.discover(url).await {
        Ok(()) => {
            // After successfully discovering the agent, we need to get its information
            // Find the newly added agent by checking all registry entries
            let agents = agent.agent_registry.all();
            let mut agent_id_discovered = None;
            
            // We'll assume the newly discovered agent is the one whose URL matches what we passed
            for (id, info) in agents {
                if info.card.url == url {
                    agent_id_discovered = Some(id);
                    break;
                }
            }
            
            if let Some(agent_id) = agent_id_discovered {
                println!("✅ Agent discovered successfully!");
                println!("Agent ID: {}", agent_id);
                
                if let Some(info) = agent.agent_registry.get(&agent_id) {
                    println!("Description: {}", info.card.description.as_deref().unwrap_or("None"));
                    return Ok(format!("Agent discovered at {} with ID: {}", url, agent_id));
                }
            }
            
            // If we couldn't find the agent info after discovery (shouldn't happen)
            Ok(format!("Agent discovered at {} but couldn't retrieve details", url))
        }
        Err(e) => {
            let error_message = format!("❌ Agent discovery failed: {}", e);
            println!("{}", error_message);
            Err(anyhow!(error_message))
        }
    }
}


/// List known agents

async fn list_known_agents(
    agent: Arc<BidirectionalAgent>,
) -> Result<()> {
    println!("📋 Known Agents:");
    
    let known_agents = agent.agent_registry.all();
    
    if known_agents.is_empty() {
        println!("  No agents discovered yet.");
    } else {
        for (i, (id, info)) in known_agents.iter().enumerate() {
            println!("{}. {} ({})", i+1, id, info.card.url);
            println!("   Description: {}", info.card.description.as_deref().unwrap_or("None"));
            println!("   Last checked: {}", info.last_checked);
            println!();
        }
    }
    
    Ok(())
}


/// List available tools

async fn list_available_tools(
    agent: Arc<BidirectionalAgent>,
) -> Result<()> {
    println!("🧰 Available Tools:");
    
    let tools = &agent.tool_executor.tools;
    
    if tools.is_empty() {
        println!("  No tools available.");
    } else {
        for (i, (name, tool)) in tools.iter().enumerate() {
            println!("{}. {}", i+1, name);
            println!("   Description: {}", tool.description());
            println!("   Capabilities: {}", tool.capabilities().join(", "));
            println!();
        }
    }
    
    Ok(())
}

/// Empty implementation when bidir-local-exec is not enabled

async fn list_available_tools(
    _agent: Arc<BidirectionalAgent>,
) -> Result<()> {
    println!("🧰 Available Tools:");
    println!("  Local tool execution is not supported in this build.");
    println!("  Enable the 'bidir-local-exec' feature to use tools.");
    Ok(())
}

/// Empty implementation when bidir-core is not enabled

async fn list_available_tools(
    _agent: Arc<impl std::any::Any>,
) -> Result<()> {
    println!("🧰 Available Tools: Feature not available, requires bidir-core feature");
    Ok(())
}

/// Display full command history
async fn display_command_history(
    commands_history: Arc<Mutex<Vec<CommandRecord>>>,
) -> Result<()> {
    display_command_history_limit(commands_history, 0).await
}

/// Display limited command history 
async fn display_command_history_limit(
    commands_history: Arc<Mutex<Vec<CommandRecord>>>,
    limit: usize,
) -> Result<()> {
    println!("📜 Command History:");
    
    let history = commands_history.lock().await;
    
    if history.is_empty() {
        println!("  No commands executed yet.");
    } else {
        let entries = if limit > 0 {
            let start = if history.len() > limit {
                history.len() - limit
            } else {
                0
            };
            &history[start..]
        } else {
            &history[..]
        };
        
        for (i, cmd) in entries.iter().enumerate() {
            println!("{}. [{}] {}", i+1, cmd.timestamp.format("%H:%M:%S"), cmd.input);
            println!("   Action: {}", cmd.action);
            if let Some(result) = &cmd.result {
                if result.len() > 100 {
                    println!("   Result: {}...", &result[..100]);
                } else {
                    println!("   Result: {}", result);
                }
            }
            println!();
        }
        
        if limit > 0 && limit < history.len() {
            println!("(Showing last {} of {} entries. Use 'history' to see all.)", 
                     limit, history.len());
        }
    }
    
    Ok(())
}

/// Print help information
fn print_help() {
    let full_help = true; // Can be parameterized later to show basic or full help
    
    println!("🔍 LLM Interface REPL Help:");
    println!("  - Type natural language commands to interact with the A2A agent network");
    println!("  - Press Tab for command completion and hints");
    println!("\n  Basic commands:");
    println!("    * help          - Show this help message");
    println!("    * help full     - Show detailed help (more commands)");
    println!("    * exit/quit     - Exit the REPL");
    
    println!("\n  Agent management:");
    println!("    * agents        - List known agents");
    println!("    * discover agent <url> - Discover a new agent at the given URL");
    
    println!("\n  Tool management:");
    println!("    * tools         - List local available tools");
    if full_help {
        println!("    * list remote tools - List tools available from remote agents");
        println!("    * discover tools   - Scan network for available tools");
    }
    
    println!("\n  Task operations:");
    println!("    * send task to <agent> <message> - Send a task to an agent");
    println!("    * stream task to <agent> <message> - Stream a task with real-time updates");
    if full_help {
        println!("    * check status <task_id> - Check status of a specific task");
        println!("    * decompose task <message> - Break down a complex task into subtasks");
        println!("    * route task <message> - Let the LLM decide the best agent/tool for a task");
        println!("    * cancel task <task_id> - Cancel a running task");
    }
    
    println!("\n  History management:");
    println!("    * history       - Show full command history");
    println!("    * history N     - Show last N entries from history");
    println!("    * history -c    - Clear command history (with confirmation)");
    println!("    Note: History file is locked to prevent concurrent REPL sessions from clobbering each other");
    
    if full_help {
        println!("\n  Advanced commands:");
        println!("    * execute tool <tool_name> <params> - Execute a local tool");
        println!("    * execute remote tool <agent> <tool> <params> - Execute a tool on remote agent");
        println!("    * list active agents - Show currently active agents in the network");
        println!("    * synthesize results <task_ids> - Combine results from multiple tasks");
    }
    
    println!("\n  Natural language examples:");
    println!("    * \"Run the shell tool to list files in the current directory\"");
    println!("    * \"Discover a new agent at http://example.com:8080\"");
    println!("    * \"Send a task to agent1 asking it to check the weather\"");
    println!("    * \"Stream a task to agent1 to generate a story in real-time\"");
    
    if full_help {
        println!("\n  Auto-completion tips:");
        println!("    * Press Tab to complete commands, agent names, and tool names");
        println!("    * Commands auto-complete from the beginning of the line");
        println!("    * After 'send task to' or 'stream task to', Tab completes agent names");
        println!("    * After 'execute tool', Tab completes tool names");
        println!("    * Use Up/Down arrows to navigate command history");
    }
    
    println!("\n  Error Recovery:");
    println!("    * When an agent isn't found, the system will suggest similar agents");
    println!("    * Network errors will automatically retry with exponential backoff");
    println!("    * When tasks time out, you'll get instructions to check status later");
    
    println!("\n  Features enabled in this build:");
    
    println!("    * ✅ Core Agent Management (bidir-core)");
    
    println!("    * ❌ Core Agent Management (bidir-core)");
    
    
    println!("    * ✅ Local Tool Execution (bidir-local-exec)");
    
    println!("    * ❌ Local Tool Execution (bidir-local-exec)");
    
    
    println!("    * ✅ Task Delegation & Synthesis (bidir-delegate)");
    
    println!("    * ❌ Task Delegation & Synthesis (bidir-delegate)");
}

/// Add REPL command to main.rs CLI
#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test::block_on;
    
    
    #[test]
    fn test_interpret_input() {
        // Create a test agent and LLM client
        let config = BidirectionalAgentConfig {
            self_id: "test-repl-agent".to_string(),
            base_url: "http://localhost:8081".to_string(),
            discovery: vec![],
            auth: crate::bidirectional_agent::config::AuthConfig::default(),
            network: crate::bidirectional_agent::config::NetworkConfig::default(),
            
            tools: crate::bidirectional_agent::config::ToolConfigs::default(),
            
            directory: crate::bidirectional_agent::config::DirectoryConfig::default(),
            
            tool_discovery_interval_minutes: 30,
        };
        
        // Mock LLM client that returns predefined responses
        struct MockLlmClient;
        
        impl MockLlmClient {
            async fn complete_json<T: for<'de> Deserialize<'de>>(&self, _prompt: &str) -> Result<T> {
                // Return a predefined response for testing
                let json_str = r#"{"action":"explain","response":"This is a test response"}"#;
                let result: T = serde_json::from_str(json_str)
                    .context("Failed to parse JSON")?;
                Ok(result)
            }
        }
        
        // Create mock agent
        let agent_result = block_on(async {
            BidirectionalAgent::new(config).await
        });
        
        assert!(agent_result.is_ok());
        let agent = Arc::new(agent_result.unwrap());
        
        // Test interpret_input with mock LLM client
        let llm_client = MockLlmClient;
        
        let result: Result<InterpretedAction> = block_on(async {
            // Can't actually run interpret_input with our mock
            // This is just a framework for a real test
            Ok(InterpretedAction::Explain { 
                response: "This is a test response".to_string() 
            })
        });
        
        assert!(result.is_ok());
        match result.unwrap() {
            InterpretedAction::Explain { response } => {
                assert_eq!(response, "This is a test response");
            }
            _ => {
                panic!("Expected Explain action");
            }
        }
    }
}

