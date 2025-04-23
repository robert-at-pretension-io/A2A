pub mod handlers;
pub mod repositories;
pub mod services;

#[cfg(test)]
pub mod tests;

mod error;

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

/// Runs the A2A server on the specified port
pub async fn run_server(port: u16) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create repositories
    let task_repository = Arc::new(InMemoryTaskRepository::new());
    
    // Create services
    let task_service = Arc::new(TaskService::new(task_repository.clone()));
    let streaming_service = Arc::new(StreamingService::new(task_repository.clone()));
    let notification_service = Arc::new(NotificationService::new(task_repository.clone()));
    
    // Create service function
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
    
    // Create the server
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let server = Server::bind(&addr).serve(service);
    
    println!("A2A Server running at http://{}", addr);
    server.await?;
    
    Ok(())
}

/// Creates and returns the agent card for this server
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