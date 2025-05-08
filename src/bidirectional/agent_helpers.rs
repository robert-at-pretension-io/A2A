use crate::bidirectional::bidirectional_agent::BidirectionalAgent;
use crate::server::handlers::jsonrpc_handler;
// server_run_server is no longer used directly by this new run_server
// use crate::server::run_server as server_run_server; 
use crate::types::{
    AgentCapabilities, AgentCard, Message, Part, Role, Task, TaskState, TaskStatus, TextPart,
};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use mime_guess;
use std::convert::Infallible;
use std::net::SocketAddr;
// PathBuf is likely already imported, but ensure it is.
// use std::path::PathBuf;
use tokio::fs::File as TokioFile;
use tokio_util::codec::{BytesCodec, FramedRead};
use futures_util::TryStreamExt;
use dashmap::DashMap;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use chrono::Utc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, trace, warn};
use uuid::Uuid;

/// A structure to store outgoing requests and their responses
/// This implements "rolling memory" that only remembers requests this agent
/// makes to other agents, and the responses it receives.
pub struct RollingMemory {
    /// Map of task IDs to tasks
    tasks: DashMap<String, Task>,
    /// Queue of task IDs in order of addition (oldest first)
    /// Protected by a Mutex for thread safety
    task_queue: Arc<Mutex<VecDeque<String>>>,
    /// Maximum number of tasks to remember
    max_tasks: usize,
    /// Map of task IDs to timestamp of when they were added
    task_timestamps: DashMap<String, Instant>,
    /// Maximum age of tasks to remember (in seconds)
    max_age: u64,
}

impl Default for RollingMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl RollingMemory {
    /// Create a new RollingMemory with default settings
    pub fn new() -> Self {
        Self {
            tasks: DashMap::new(),
            task_queue: Arc::new(Mutex::new(VecDeque::new())),
            max_tasks: 50, // Default to remembering last 50 tasks
            task_timestamps: DashMap::new(),
            max_age: 24 * 60 * 60, // Default to 24 hours
        }
    }

    /// Create a new RollingMemory with custom settings
    pub fn with_limits(max_tasks: usize, max_age_hours: u64) -> Self {
        Self {
            tasks: DashMap::new(),
            task_queue: Arc::new(Mutex::new(VecDeque::new())),
            max_tasks,
            task_timestamps: DashMap::new(),
            max_age: max_age_hours * 60 * 60, // Convert hours to seconds
        }
    }

    /// Add a task to the memory
    pub fn add_task(&mut self, task: Task) {
        let task_id = task.id.clone();

        // Store the task
        self.tasks.insert(task_id.clone(), task);

        // Add to queue and timestamp
        if let Ok(mut queue) = self.task_queue.lock() {
            queue.push_back(task_id.clone());
        } else {
            warn!("Failed to lock task_queue for adding task: {}", task_id);
        }
        self.task_timestamps.insert(task_id, Instant::now());

        // Enforce size limit
        self.prune_by_size();

        // Enforce age limit
        self.prune_by_age();
    }

    /// Update an existing task in the memory
    pub fn update_task(&mut self, task: Task) -> bool {
        let task_id = task.id.clone();

        // Only update if we're already tracking this task
        if self.tasks.contains_key(&task_id) {
            self.tasks.insert(task_id, task);
            true
        } else {
            false
        }
    }

    /// Get a task by ID
    pub fn get_task(&self, task_id: &str) -> Option<Task> {
        self.tasks.get(task_id).map(|t| t.clone())
    }

    /// Get all tasks in memory
    pub fn get_all_tasks(&self) -> Vec<Task> {
        self.tasks.iter().map(|t| t.clone()).collect()
    }

    /// Get tasks in chronological order (oldest first)
    pub fn get_tasks_chronological(&self) -> Vec<Task> {
        let mut tasks = Vec::new();

        // Lock the queue to read task IDs
        if let Ok(queue) = self.task_queue.lock() {
            for task_id in queue.iter() {
                if let Some(task) = self.tasks.get(task_id) {
                    tasks.push(task.clone());
                }
            }
        } else {
            warn!("Failed to lock task_queue for get_tasks_chronological");
        }

        tasks
    }

    /// Remove tasks that exceed the size limit
    fn prune_by_size(&mut self) {
        // Lock the queue for modification
        if let Ok(mut queue) = self.task_queue.lock() {
            while queue.len() > self.max_tasks {
                if let Some(oldest_id) = queue.pop_front() {
                    self.tasks.remove(&oldest_id);
                    self.task_timestamps.remove(&oldest_id);
                }
            }
        } else {
            warn!("Failed to lock task_queue for prune_by_size");
        }
    }

    /// Remove tasks that exceed the age limit
    fn prune_by_age(&mut self) {
        let now = Instant::now();
        let max_age_duration = Duration::from_secs(self.max_age);

        // Collect IDs to remove
        let mut ids_to_remove = Vec::new();
        for entry in self.task_timestamps.iter() {
            let task_id = entry.key();
            let timestamp = entry.value();

            if now.duration_since(*timestamp) > max_age_duration {
                ids_to_remove.push(task_id.clone());
            }
        }

        // Remove the expired tasks
        for id in ids_to_remove {
            self.tasks.remove(&id);
            self.task_timestamps.remove(&id);

            // Also remove from the queue
            if let Ok(mut queue) = self.task_queue.lock() {
                if let Some(pos) = queue.iter().position(|x| x == &id) {
                    queue.remove(pos);
                }
            }
        }
    }

    /// Clear all memory
    pub fn clear(&mut self) {
        self.tasks.clear();
        if let Ok(mut queue) = self.task_queue.lock() {
            queue.clear();
        }
        self.task_timestamps.clear();
    }
}

/// Helper function to create a Message from an agent with the given text
pub fn agent_message(text: impl Into<String>) -> Message {
    let text = text.into();
    Message {
        role: Role::Agent,
        parts: vec![Part::TextPart(TextPart {
            type_: "text".to_string(),
            text,
            metadata: None,
        })],
        metadata: None,
    }
}

/// Create a task with the specified ID and initial state
pub fn create_task(id: &str, state: TaskState, message: Option<String>) -> Task {
    Task {
        id: id.to_string(),
        status: TaskStatus {
            state,
            timestamp: Some(Utc::now()),
            message: message.map(agent_message),
        },
        artifacts: None,
        history: None,
        metadata: None,
        session_id: None,
    }
}

// Extension trait for Message to add convenience methods
pub trait MessageExt {
    fn agent(text: impl Into<String>) -> Message;
}

// Implement the extension trait for Message
impl MessageExt for Message {
    fn agent(text: impl Into<String>) -> Message {
        agent_message(text)
    }
}

/// Update a task's status with a new state, optional message and optional metadata
pub fn update_task_status(
    task: &mut Task,
    state: TaskState,
    message: Option<String>,
    metadata: Option<serde_json::Map<String, serde_json::Value>>,
) {
    task.status.state = state;
    task.status.timestamp = Some(Utc::now());

    if let Some(msg_text) = message {
        let mut msg = agent_message(msg_text);
        if let Some(meta) = metadata {
            msg.metadata = Some(meta);
        }
        task.status.message = Some(msg);
    }
}

/// Safely extract text content from a task's status message
pub fn extract_status_message(task: &Task) -> Option<String> {
    task.status.message.as_ref().and_then(|message| {
        let texts: Vec<String> = message
            .parts
            .iter()
            .filter_map(|part| match part {
                Part::TextPart(tp) => Some(tp.text.clone()),
                _ => None,
            })
            .collect();

        if texts.is_empty() {
            None
        } else {
            Some(texts.join("\n"))
        }
    })
}

/// Extract all text content from a message
pub fn extract_text_from_message(message: &Message) -> String {
    message
        .parts
        .iter()
        .filter_map(|part| match part {
            Part::TextPart(tp) => Some(tp.text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract text from the last user message in a task's history
pub fn extract_last_user_message(task: &Task) -> Option<String> {
    task.history.as_ref().and_then(|history| {
        history
            .iter()
            .filter(|msg| msg.role == Role::User)
            .next_back()
            .map(extract_text_from_message)
    })
}

/// Check if a task has been delegated from another agent
pub fn is_delegated_task(task: &Task) -> bool {
    task.metadata.as_ref().is_some_and(|md| {
        md.get("delegated_from").is_some()
            || md.get("remote_agent_id").is_some()
            || md.get("source_agent_id").is_some()
    })
}

/// Get the URL of the agent's current client
pub fn client_url(agent: &BidirectionalAgent) -> Option<String> {
    agent.client_config.target_url.clone()
}

/// Ensure the agent has an active session, creating one if needed
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub async fn ensure_session(agent: &mut BidirectionalAgent) {
    debug!("Ensuring agent has an active session.");
    if agent.current_session_id.is_none() {
        debug!("No current session ID, creating new session.");
        let new_session_id = create_new_session(agent);
        debug!(session_id = %new_session_id, "Created new session.");
    } else {
        trace!(session_id = ?agent.current_session_id, "Using existing session.");
    }
}

/// Create a new session for the agent
pub fn create_new_session(agent: &mut BidirectionalAgent) -> String {
    debug!("Creating new session.");
    let session_id = format!("session-{}", Uuid::new_v4());
    agent.current_session_id = Some(session_id.clone());

    // Initialize empty task list for the session
    if !agent.session_tasks.contains_key(&session_id) {
        debug!(session_id = %session_id, "Initializing empty task list for new session.");
        agent.session_tasks.insert(session_id.clone(), Vec::new());
    }

    session_id
}

/// Save a task to the agent's history
#[instrument(skip(agent, task), fields(agent_id = %agent.agent_id, task_id = %task.id))]
pub async fn save_task_to_history(agent: &BidirectionalAgent, task: Task) -> Result<()> {
    debug!("Saving task to agent history.");

    // Ensure we have a session ID from the task or the current session
    let session_id = if let Some(task_session_id) = &task.session_id {
        debug!(task_session_id = %task_session_id, "Using task's session ID.");
        task_session_id.clone()
    } else if let Some(current_session_id) = &agent.current_session_id {
        debug!(current_session_id = %current_session_id, "Using agent's current session ID.");
        current_session_id.clone()
    } else {
        error!("No session ID available for task history.");
        return Err(anyhow!("No session ID available for task history"));
    };

    // Add task ID to session's task list if it's not already there
    let mut tasks = agent.session_tasks.entry(session_id.clone()).or_default();
    if !tasks.contains(&task.id) {
        debug!(task_id = %task.id, session_id = %session_id, "Adding task ID to session task list.");
        tasks.push(task.id.clone());
    }

    Ok(())
}

/// Get all tasks in the current session
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub async fn get_current_session_tasks(agent: &BidirectionalAgent) -> Result<Vec<Task>> {
    if let Some(session_id) = &agent.current_session_id {
        debug!(session_id = %session_id, "Fetching tasks for current session.");

        if let Some(task_ids) = agent.session_tasks.get(session_id) {
            debug!(
                task_count = task_ids.value().len(),
                "Found task IDs in session."
            );
            let mut tasks = Vec::new();

            // Clone the task_ids to avoid borrowing issues
            let task_id_vec = task_ids.value().clone();

            for task_id in task_id_vec {
                match agent
                    .task_service
                    .get_task(crate::types::TaskQueryParams {
                        id: task_id.clone(),
                        history_length: None,
                        metadata: None,
                    })
                    .await
                {
                    Ok(task) => {
                        debug!(task_id = %task_id, "Retrieved task successfully.");
                        tasks.push(task);
                    }
                    Err(e) => {
                        warn!(task_id = %task_id, error = %e, "Failed to retrieve task. Skipping.");
                        // Don't fail for individual task lookup errors, just skip
                    }
                }
            }

            Ok(tasks)
        } else {
            debug!(session_id = %session_id, "No tasks found for session ID.");
            Ok(Vec::new())
        }
    } else {
        error!("No current session ID available.");
        Err(anyhow!("No current session"))
    }
}

/// Extract text from a task (status message or artifacts)
pub fn extract_text_from_task(agent: &BidirectionalAgent, task: &Task) -> String {
    debug!(task_id = %task.id, "Extracting text from task.");

    // Try to get text from status message first
    if let Some(ref status_msg) = task.status.message {
        let text = status_msg
            .parts
            .iter()
            .filter_map(|p| match p {
                Part::TextPart(tp) => Some(tp.text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        if !text.is_empty() {
            debug!(task_id = %task.id, "Extracted text from task status message.");
            return text;
        }
    }

    // If no status message text, try to get text from artifacts
    if let Some(ref artifacts) = task.artifacts {
        if !artifacts.is_empty() {
            debug!(task_id = %task.id, artifact_count = artifacts.len(), "Extracting text from task artifacts.");
            let mut texts = Vec::new();

            for artifact in artifacts {
                for part in &artifact.parts {
                    if let Part::TextPart(tp) = part {
                        texts.push(tp.text.as_str());
                    }
                }
            }

            if !texts.is_empty() {
                let result = texts.join("\n");
                debug!(task_id = %task.id, "Successfully extracted text from artifacts.");
                return result;
            }
        }
    }

    // Default response if no text found
    debug!(task_id = %task.id, "No text found in task. Returning default message.");
    "No response text available.".to_string()
}

/// Run the agent server
#[instrument(skip(agent), fields(agent_id = %agent.agent_id, port = %agent.port, bind_address = %agent.bind_address))]
pub async fn run_server(agent: &BidirectionalAgent) -> Result<()> {
    let addr = SocketAddr::new(agent.bind_address.parse()?, agent.port);
    info!("🚀 Server starting on http://{}", addr);

    let task_service_arc = agent.task_service.clone();
    let streaming_service_arc = agent.streaming_service.clone();
    let notification_service_arc = agent.notification_service.clone();
    let static_files_root_arc = agent.static_files_root.clone();
    // agent_card is created per request by jsonrpc_handler if needed for /.well-known/agent.json

    let make_svc = make_service_fn(move |_conn| {
        let task_service = task_service_arc.clone();
        let streaming_service = streaming_service_arc.clone();
        let notification_service = notification_service_arc.clone();
        let static_files_root = static_files_root_arc.clone();

        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                let task_service_req = task_service.clone();
                let streaming_service_req = streaming_service.clone();
                let notification_service_req = notification_service.clone();
                let static_files_root_req = static_files_root.clone();
                // Clone agent details needed for the card
                let agent_card_struct = agent.create_agent_card(); // Use the agent's own method

                async move {
                    let req_path = req.uri().path().to_string();
                    let req_method = req.method().clone();
                    info!(method = %req_method, path = %req_path, "Incoming HTTP request");

                    // --- Agent Card Handling ---
                    // Serve the agent's specific card if requested
                    if req_method == Method::GET && req_path == "/.well-known/agent.json" {
                        debug!("Serving agent card for /.well-known/agent.json");
                        match serde_json::to_string(&agent_card_struct) {
                            Ok(json_body) => {
                                return Ok(Response::builder()
                                    .status(StatusCode::OK)
                                    .header(hyper::header::CONTENT_TYPE, "application/json")
                                    .body(Body::from(json_body))
                                    .unwrap());
                            }
                            Err(e) => {
                                error!(error = %e, "Failed to serialize agent card");
                                return Ok(Response::builder()
                                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                                    .body(Body::from("Internal Server Error: Failed to serialize agent card"))
                                    .unwrap());
                            }
                        }
                    }

                    // --- Static File Handling (index.html only) ---
                    // Try to serve static index.html if configured and requested.
                    if req_method == Method::GET { // Only handle GET for static files
                        if let Some(base_path) = static_files_root_req {
                            if req_path == "/" || req_path == "/index.html" {
                                let mut file_path_to_serve = base_path.clone();

                                // Check for path traversal attempts before any path manipulation
                                if req_path.contains("..") || req_path.to_lowercase().contains("%2e%2e") {
                                    warn!(path = %req_path, "Path traversal attempt detected in request path.");
                                    let mut response = Response::new(Body::from("403 Forbidden"));
                                    *response.status_mut() = StatusCode::FORBIDDEN;
                                    return Ok(response);
                                }

                                file_path_to_serve.push("index.html");

                                let canon_base_path = match base_path.canonicalize() {
                                    Ok(p) => p,
                                    Err(e) => {
                                        error!(path = %base_path.display(), error = %e, "Failed to canonicalize static base path. Check server configuration.");
                                        let mut response = Response::new(Body::from("500 Internal Server Error: Invalid static path configuration"));
                                        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                                        return Ok(response);
                                    }
                                };

                                match file_path_to_serve.canonicalize() {
                                    Ok(canon_file_to_serve) => {
                                        if !canon_file_to_serve.starts_with(&canon_base_path) {
                                            warn!(
                                                requested_path = %file_path_to_serve.display(),
                                                canonical_requested = %canon_file_to_serve.display(),
                                                canonical_base = %canon_base_path.display(),
                                                "Path traversal attempt detected."
                                            );
                                            let mut response = Response::new(Body::from("403 Forbidden"));
                                            *response.status_mut() = StatusCode::FORBIDDEN;
                                            return Ok(response);
                                        }

                                        debug!(file_path = %canon_file_to_serve.display(), "Attempting to serve canonical static file (index.html)");
                                        match TokioFile::open(&canon_file_to_serve).await {
                                            Ok(file) => {
                                                let stream = FramedRead::new(file, BytesCodec::new());
                                                let body = Body::wrap_stream(stream.map_ok(|bytes| bytes.freeze()));
                                                let mime_type = mime_guess::from_path(&canon_file_to_serve).first_or_octet_stream();
                                                match Response::builder()
                                                    .status(StatusCode::OK)
                                                    .header(hyper::header::CONTENT_TYPE, mime_type.as_ref())
                                                    .body(body)
                                                {
                                                    Ok(res) => {
                                                        info!(path = %req_path, "Successfully served static file (index.html)");
                                                        return Ok(res);
                                                    }
                                                    Err(e) => {
                                                        error!(path = %req_path, error = %e, "Error building response for static file (index.html)");
                                                        let mut response = Response::new(Body::from("500 Internal Server Error"));
                                                        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                                                        return Ok(response);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error!(canonical_path = %canon_file_to_serve.display(), error = %e, "Error opening canonical static file (index.html, permissions issue?)");
                                                let mut response = Response::new(Body::from("500 Internal Server Error: Could not open file"));
                                                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                                                return Ok(response);
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        warn!(path = %req_path, original_path = %file_path_to_serve.display(), error = %e, "Static file index.html not found or error during canonicalization");
                                        // Fall through to A2A handler or 404 below
                                    }
                                }
                            } else {
                                // It's a GET request for a path other than "/" or "/index.html" (e.g., "/style.css")
                                // and static_files_root IS configured. We must block this.
                                warn!(path = %req_path, "Access to non-index static file denied.");
                                let mut response = Response::new(Body::from(format!("403 Forbidden: Access to {} is not allowed", req_path)));
                                *response.status_mut() = StatusCode::FORBIDDEN;
                                return Ok(response);
                            }
                        }
                        // If static_files_root is None, or if it's a GET for a non-index path,
                        // we fall through to the A2A handler or 404 logic.
                    }


                    // --- A2A JSON-RPC Handling ---
                    // If it wasn't an agent card request or a handled static file request,
                    // pass it to the generic JSON-RPC handler.
                    debug!(method = %req_method, path = %req_path, "Passing request to A2A JSON-RPC handler");
                    jsonrpc_handler(
                        req,
                            task_service_req,
                            streaming_service_req,
                            notification_service_req,
                        )
                        .await;
                    }

                    // 2. At this point, it's a GET request for something other than /.well-known/agent.json.
                    //    Try to serve static index.html if configured and requested.
                    if let Some(base_path) = static_files_root_req {
                        if req_path == "/" || req_path == "/index.html" {
                            let mut file_path_to_serve = base_path.clone();

                            // Check for path traversal attempts before any path manipulation
                            if req_path.contains("..") || req_path.to_lowercase().contains("%2e%2e") {
                                warn!(path = %req_path, "Path traversal attempt detected in request path.");
                                let mut response = Response::new(Body::from("403 Forbidden"));
                                *response.status_mut() = StatusCode::FORBIDDEN;
                                return Ok(response);
                            }
                            
                            file_path_to_serve.push("index.html");
                            
                            let canon_base_path = match base_path.canonicalize() {
                                Ok(p) => p,
                                Err(e) => {
                                    error!(path = %base_path.display(), error = %e, "Failed to canonicalize static base path. Check server configuration.");
                                    let mut response = Response::new(Body::from("500 Internal Server Error: Invalid static path configuration"));
                                    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                                    return Ok(response);
                                }
                            };

                            match file_path_to_serve.canonicalize() {
                                Ok(canon_file_to_serve) => {
                                    if !canon_file_to_serve.starts_with(&canon_base_path) {
                                        warn!(
                                            requested_path = %file_path_to_serve.display(),
                                            canonical_requested = %canon_file_to_serve.display(),
                                            canonical_base = %canon_base_path.display(),
                                            "Path traversal attempt detected."
                                        );
                                        let mut response = Response::new(Body::from("403 Forbidden"));
                                        *response.status_mut() = StatusCode::FORBIDDEN;
                                        return Ok(response);
                                    }

                                    debug!(file_path = %canon_file_to_serve.display(), "Attempting to serve canonical static file (index.html)");
                                    match TokioFile::open(&canon_file_to_serve).await {
                                        Ok(file) => {
                                            let stream = FramedRead::new(file, BytesCodec::new());
                                            let body = Body::wrap_stream(stream.map_ok(|bytes| bytes.freeze()));
                                            let mime_type = mime_guess::from_path(&canon_file_to_serve).first_or_octet_stream();
                                            match Response::builder()
                                                .status(StatusCode::OK)
                                                .header(hyper::header::CONTENT_TYPE, mime_type.as_ref())
                                                .body(body)
                                            {
                                                Ok(res) => {
                                                    info!(path = %req_path, "Successfully served static file (index.html)");
                                                    return Ok(res);
                                                }
                                                Err(e) => {
                                                    error!(path = %req_path, error = %e, "Error building response for static file (index.html)");
                                                    let mut response = Response::new(Body::from("500 Internal Server Error"));
                                                    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                                                    return Ok(response);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error!(canonical_path = %canon_file_to_serve.display(), error = %e, "Error opening canonical static file (index.html, permissions issue?)");
                                            let mut response = Response::new(Body::from("500 Internal Server Error: Could not open file"));
                                            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                                            return Ok(response);
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!(path = %req_path, original_path = %file_path_to_serve.display(), error = %e, "Static file index.html not found or error during canonicalization");
                                    let mut response = Response::new(Body::from(format!("404 Not Found: {}", req_path)));
                                    *response.status_mut() = StatusCode::NOT_FOUND;
                                    return Ok(response);
                                }
                            }
                        } else {
                            // It's a GET request for a path other than "/" or "/index.html" (e.g., "/style.css")
                            // and static_files_root IS configured. We must block this.
                            warn!(path = %req_path, "Access to non-index static file denied.");
                            let mut response = Response::new(Body::from(format!("403 Forbidden: Access to {} is not allowed", req_path)));
                            *response.status_mut() = StatusCode::FORBIDDEN;
                            return Ok(response);
                        }
                    } else {
                        // static_files_root is NOT configured.
                        // It's a GET request for something other than /.well-known/agent.json.
                        // Since no static files are configured, any such GET should be a 404.
                        warn!(path = %req_path, "Static file serving not configured, GET request denied.");
                        let mut response = Response::new(Body::from(format!("404 Not Found: {}", req_path)));
                        *response.status_mut() = StatusCode::NOT_FOUND;
                        return Ok(response);
                    }
                }
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);
    info!("A2A Agent server (and static file server if configured) listening on http://{}", addr);

    // Setup graceful shutdown for the Hyper server
    let token = CancellationToken::new(); // Used for bidirectional_agent's own server_run_server
                                          // For this hyper server, we'll use a signal handler
    let graceful = server.with_graceful_shutdown(async {
        tokio::signal::ctrl_c().await.expect("failed to install CTRL+C signal handler");
        info!("CTRL+C received, shutting down server...");
        token.cancel(); // If this token is used by other parts
    });

    if let Err(e) = graceful.await {
        error!(error = %e, "Server error");
        return Err(e.into());
    }
    info!("Server shut down gracefully.");
    Ok(())
}

/// Send a task to a remote agent
#[instrument(skip(agent, message), fields(agent_id = %agent.agent_id, message_len = message.len()))]
pub async fn send_task_to_remote(agent: &mut BidirectionalAgent, message: &str) -> Result<Task> {
    debug!("Sending task to remote agent.");

    // Ensure we have a session first (before borrowing client)
    ensure_session(agent).await;

    // Get the session ID to avoid borrowing issues
    let session_id = agent.current_session_id.clone();

    // Check if client is available
    let client = match &mut agent.client {
        Some(client) => client,
        None => {
            error!("No remote agent connected. Cannot send task.");
            return Err(anyhow!(
                "Not connected to any remote agent. Use :connect URL first"
            ));
        }
    };

    // Send the task using the client's method (which expects text and session_id)
    info!("Sending task to remote agent. Awaiting response...");
    match client.send_task(message, session_id.clone()).await {
        Ok(task) => {
            debug!(task_id = %task.id, status = ?task.status.state, "Task sent successfully to remote agent.");

            // Store task locally for history tracking
            if let Some(session_id) = &session_id {
                agent
                    .session_tasks
                    .entry(session_id.clone())
                    .or_default()
                    .push(task.id.clone());
                debug!(task_id = %task.id, session_id = %session_id, "Added remote task to session history.");
            }

            // Add task to rolling memory - this is for "agent memory" of outgoing requests
            debug!(task_id = %task.id, "Adding outgoing task to rolling memory.");
            agent.rolling_memory.add_task(task.clone());

            Ok(task)
        }
        Err(e) => {
            error!(error = %e, "Failed to send task to remote agent.");
            Err(anyhow!("Failed to send task to remote agent: {}", e))
        }
    }
}

/// Get the agent card from a remote agent
#[instrument(skip(agent), fields(agent_id = %agent.agent_id))]
pub async fn get_remote_agent_card(agent: &mut BidirectionalAgent) -> Result<AgentCard> {
    debug!("Getting agent card from remote agent.");

    // Check if client is available
    let client = match &mut agent.client {
        Some(client) => client,
        None => {
            error!("No remote agent connected. Cannot get card.");
            return Err(anyhow!(
                "Not connected to any remote agent. Use :connect URL first"
            ));
        }
    };

    // Get the agent card
    info!("Requesting agent card from remote agent...");
    match client.get_agent_card().await {
        Ok(card) => {
            debug!(card_name = %card.name, "Successfully retrieved agent card.");
            Ok(card)
        }
        Err(e) => {
            error!(error = %e, "Failed to get agent card from remote agent.");
            Err(anyhow!("Failed to get agent card: {}", e))
        }
    }
}

/// Create an agent card for this agent instance
pub fn create_agent_card(agent: &BidirectionalAgent) -> AgentCard {
    debug!(agent_id = %agent.agent_id, "Creating agent card.");

    let url = format!("http://{}:{}", agent.bind_address, agent.port);
    
    // For consistent testing, create the agent card manually
    // This ensures that all capabilities are properly set
    debug!("Creating agent card with manual construction.");
    return AgentCard {
        name: agent.agent_name.clone(),
        description: Some(format!("Bidirectional A2A Agent (ID: {})", agent.agent_id)),
        url,
        version: crate::bidirectional::bidirectional_agent::AGENT_VERSION.to_string(),
        capabilities: AgentCapabilities {
            streaming: true,
            push_notifications: true,
            state_transition_history: true,
        },
        skills: vec![],
        default_input_modes: vec!["text".to_string()],
        default_output_modes: vec!["text".to_string()],
        // Fields that must be provided but we don't care about
        authentication: None,
        documentation_url: None,
        provider: None,
    };
    
    // The following code is no longer used but kept for reference
    /*
    // Use the common create_agent_card function from server module
    let card_json = crate::server::create_agent_card(
        Some(&agent.agent_name),
        Some(&format!("Bidirectional A2A Agent (ID: {})", agent.agent_id)),
        Some(&url),
        Some(crate::bidirectional::bidirectional_agent::AGENT_VERSION),
        None // Use default skills
    );
    
    // Convert the JSON Value back to an AgentCard
    match serde_json::from_value(card_json) {
        Ok(card) => card,
        Err(e) => {
            // If there's an error, fall back to manual creation
            error!(error = %e, "Failed to convert agent card from JSON. Using manual fallback.");
    */
    // Rest of the implementation has been moved above
}
