pub mod handlers;
pub mod repositories;
pub mod services;

#[cfg(test)]
pub mod tests;

// Local modules for functionality previously from bidirectional_agent
pub mod agent_registry;
pub mod client_manager;
pub mod task_router;
pub mod tool_executor;

pub mod error; // Make the error module public

use hyper::service::{make_service_fn, service_fn};
use hyper::{header, Body, Request, Response, Server, StatusCode};
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::server::handlers::jsonrpc_handler;
use crate::server::services::notification_service::NotificationService;
use crate::server::services::streaming_service::StreamingService;
use crate::server::services::task_service::TaskService;
pub use error::ServerError;
use tokio::task::JoinHandle; // Add JoinHandle import
use tokio_util::sync::CancellationToken; // Add CancellationToken import

// Use our local component types

/// Runs the A2A server on the specified port, accepting pre-built services.
/// Returns a JoinHandle for the server task.
pub async fn run_server(
    port: u16,
    bind_address: &str,                             // Add bind address parameter
    task_service: Arc<TaskService>,                 // Accept pre-built TaskService
    streaming_service: Arc<StreamingService>,       // Accept pre-built StreamingService
    notification_service: Arc<NotificationService>, // Accept pre-built NotificationService
    shutdown_token: CancellationToken,              // Add shutdown token
    agent_card: Option<serde_json::Value>,          // Optional custom agent card
) -> Result<JoinHandle<()>, Box<dyn std::error::Error + Send + Sync>> {
    // Create service function using the provided services
    let service = make_service_fn(move |_| {
        let task_svc = task_service.clone();
        let stream_svc = streaming_service.clone();
        let notif_svc = notification_service.clone();
        // Clone the custom agent card or use the default
        let agent_card_clone = agent_card.clone();

        async move {
            Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                let task_svc = task_svc.clone();
                let stream_svc = stream_svc.clone();
                let notif_svc = notif_svc.clone();
                let card = agent_card_clone.clone();

                async move {
                    // For agent card requests, check if we have a custom one first
                    if req.uri().path() == "/.well-known/agent.json" && card.is_some() {
                        return Ok(Response::builder()
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, "application/json")
                            .body(Body::from(
                                serde_json::to_string(&card.as_ref().unwrap()).unwrap(),
                            ))
                            .unwrap());
                    }

                    // Otherwise, use the standard handler
                    jsonrpc_handler(req, task_svc, stream_svc, notif_svc).await
                }
            }))
        }
    });

    // Create the server address
    let addr_str = format!("{}:{}", bind_address, port);
    let addr: SocketAddr = addr_str
        .parse()
        .map_err(|e| format!("Invalid bind address '{}': {}", addr_str, e))?;

    // Create the server with graceful shutdown
    let server = Server::bind(&addr).serve(service);
    let server_with_shutdown = server.with_graceful_shutdown(async move {
        shutdown_token.cancelled().await;
        println!("🔌 Server shutdown initiated...");
    });

    println!("🔌 A2A Server running at http://{}", addr);

    // Start server in a task and return the handle
    let handle = tokio::spawn(async move {
        if let Err(e) = server_with_shutdown.await {
            eprintln!("❌ Server error: {}", e);
        }
        println!("🔌 Server shutdown complete.");
    });

    Ok(handle)
}

/// Creates and returns the agent card for this server with dynamic configuration
/// 
/// # Parameters
/// 
/// * `name` - Optional name for the agent (defaults to "A2A Test Suite Reference Server")
/// * `description` - Optional description for the agent
/// * `url` - Optional URL where the agent is hosted (defaults to "http://localhost:8081")
/// * `version` - Optional version string (defaults to "1.0.0")
/// * `skills` - Optional array of skill objects to include in the agent card
/// 
/// # Example (not run in doctests)
/// 
/// ```ignore
/// let skills = serde_json::json!([
///     {
///         "id": "translate",
///         "name": "Translate",
///         "description": "Translates text between languages",
///         "tags": ["translation", "language"],
///         "input_modes": ["text"],
///         "output_modes": ["text"]
///     }
/// ]);
/// 
/// // Import the function first
/// use a2a_test_suite::server::create_agent_card;
/// 
/// let card = create_agent_card(
///     Some("Translator Agent"),
///     Some("An agent that can translate text between languages"),
///     Some("https://translator.example.com"),
///     Some("2.0.0"),
///     Some(skills)
/// );
/// ```
pub fn create_agent_card(
    name: Option<&str>,
    description: Option<&str>,
    url: Option<&str>,
    version: Option<&str>,
    skills: Option<serde_json::Value>,
) -> serde_json::Value {
    // Set default values or use provided ones
    let agent_name = name.unwrap_or("A2A Test Suite Reference Server");
    let agent_description = description.unwrap_or("A reference implementation of the A2A protocol for testing");
    let agent_url = url.unwrap_or("http://localhost:8081");
    let agent_version = version.unwrap_or("1.0.0");
    
    // Default skills if none provided
    let agent_skills = skills.unwrap_or_else(|| {
        json!([
            {
                "id": "echo",
                "name": "Echo",
                "description": "Echoes back the input text",
                "tags": ["echo", "text_manipulation"],
                "input_modes": ["text"],
                "output_modes": ["text"]
            }
        ])
    });
    
    // Using a serde_json::Value to ensure all required fields are present and properly formatted
    let mut card = json!({
        "name": agent_name,
        "description": agent_description,
        "provider": null,
        "authentication": null,
        "capabilities": {
            "streaming": true,
            "push_notifications": true,
            "state_transition_history": true
        },
        "default_input_modes": ["text"],
        "default_output_modes": ["text"],
        "documentation_url": null,
        "version": agent_version,
        "url": agent_url,
        "skills": agent_skills
    });
    
    // Debug capabilities - this shouldn't be necessary but helps troubleshoot
    // Ensure all capabilities are explicitly set to true
    if let Some(capabilities) = card.get_mut("capabilities") {
        if let Some(obj) = capabilities.as_object_mut() {
            obj.insert("streaming".to_string(), json!(true));
            obj.insert("push_notifications".to_string(), json!(true));
            obj.insert("state_transition_history".to_string(), json!(true));
        }
    }
    
    card
}
