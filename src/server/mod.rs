pub mod handlers;
pub mod repositories;
pub mod services;

#[cfg(test)]
pub mod tests;

// Local modules for functionality previously from bidirectional_agent
pub mod agent_registry;
pub mod client_manager;
pub mod task_flow;
pub mod task_router;
pub mod tool_executor;

pub mod error; // Make the error module public

use crate::types;
use hyper::{Body, Request, Response, Server, StatusCode};
use hyper::service::{make_service_fn, service_fn};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use serde_json::{json, Value};
use uuid::Uuid;

pub use error::ServerError;
use crate::server::handlers::jsonrpc_handler;
use crate::server::repositories::task_repository::InMemoryTaskRepository;
use crate::server::services::task_service::TaskService;
use crate::server::services::streaming_service::StreamingService;
use crate::server::services::notification_service::NotificationService;
use tokio::task::JoinHandle; // Add JoinHandle import
use tokio_util::sync::CancellationToken; // Add CancellationToken import

// Use our local component types
use crate::server::{
    task_router::TaskRouter,
    tool_executor::ToolExecutor,
    client_manager::ClientManager,
    agent_registry::AgentRegistry,
    task_flow::TaskFlow,
};


/// Runs the A2A server on the specified port, accepting pre-built services.
/// Returns a JoinHandle for the server task.
pub async fn run_server(
    port: u16,
    bind_address: &str, // Add bind address parameter
    task_service: Arc<TaskService>, // Accept pre-built TaskService
    streaming_service: Arc<StreamingService>, // Accept pre-built StreamingService
    notification_service: Arc<NotificationService>, // Accept pre-built NotificationService
    shutdown_token: CancellationToken, // Add shutdown token
) -> Result<JoinHandle<()>, Box<dyn std::error::Error + Send + Sync>> {

    // Create service function using the provided services
    let service = make_service_fn(move |_| {
        let task_svc = task_service.clone();
        let stream_svc = streaming_service.clone();
        let notif_svc = notification_service.clone();
        
        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                jsonrpc_handler(
                    req,
                    task_svc.clone(),
                    stream_svc.clone(),
                    notif_svc.clone()
                )
            }))
        }
    });

    // Create the server address
    let addr_str = format!("{}:{}", bind_address, port);
    let addr: SocketAddr = addr_str.parse()
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


/// Creates and returns the agent card for this server
// TODO: This should likely move or accept config to generate dynamic URL/skills
pub fn create_agent_card() -> serde_json::Value {
    // Using a serde_json::Value instead of the typed AgentCard
    // to ensure all required fields are present and properly formatted
    json!({
        "name": "A2A Test Suite Reference Server",
        "description": "A reference implementation of the A2A protocol for testing",
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
        "version": "1.0.0",
        "url": "http://localhost:8081",
        "skills": []
    })
}
