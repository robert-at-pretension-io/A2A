use hyper::{Body, Method, Request, StatusCode};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;

use crate::server::repositories::task_repository::InMemoryTaskRepository;
use crate::server::services::notification_service::NotificationService;
use crate::server::services::streaming_service::StreamingService;
use crate::server::services::task_service::TaskService;
use crate::server::run_server;

#[tokio::test]
#[ignore = "CORS headers are now handled by Nginx, not by the server"]
async fn test_cors_headers() {
    // Note: This test is now ignored because CORS headers are added by Nginx in production
    // and not directly in the code.

    // Shared state
    let task_repo = Arc::new(InMemoryTaskRepository::new());
    let streaming_service = Arc::new(StreamingService::new(task_repo.clone()));
    let notification_service = Arc::new(NotificationService::new(task_repo.clone()));
    let task_service = Arc::new(TaskService::standalone(task_repo.clone()));

    // Server setup with unique port
    let port = 9876;
    let shutdown_token = CancellationToken::new();
    let token_clone = shutdown_token.clone();

    // Start the server
    let server_handle = run_server(
        port,
        "127.0.0.1",
        task_service,
        streaming_service,
        notification_service,
        shutdown_token,
        None,
    )
    .await
    .unwrap();

    // Create a hyper client
    let client = hyper::Client::new();

    // Basic functionality testing - we no longer check for CORS headers
    // since they're now handled by Nginx

    // Test: Agent card request
    let agent_card_req = Request::builder()
        .method(Method::GET)
        .uri(format!("http://127.0.0.1:{}/.well-known/agent.json", port))
        .body(Body::empty())
        .unwrap();

    let agent_card_resp = client.request(agent_card_req).await.unwrap();

    // Verify status and content type
    assert_eq!(agent_card_resp.status(), StatusCode::OK);
    assert!(agent_card_resp.headers().contains_key("content-type"));

    // Shutdown the server
    token_clone.cancel();
    server_handle.await.unwrap();
}