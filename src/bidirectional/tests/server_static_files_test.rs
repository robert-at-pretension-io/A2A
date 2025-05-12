use crate::bidirectional::bidirectional_agent::BidirectionalAgent;
use crate::bidirectional::config::BidirectionalAgentConfig;
use std::fs::{self, File};
use std::io::Write;
use crate::types::AgentCard; // For deserializing agent card
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use tempfile::tempdir;
use tokio::task::JoinHandle;
use tracing::info;
use tempfile::TempDir; // Import TempDir

async fn start_test_server(
    config: BidirectionalAgentConfig,
    enable_static_files: bool,
) -> (String, JoinHandle<Result<(), anyhow::Error>>, Option<TempDir>) {
    // Find an available port
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind to random port");
    let addr = listener.local_addr().expect("Failed to get local address");
    let port = addr.port();
    drop(listener); // Close the listener so the server can bind to this port

    let mut effective_config = config.clone();
    effective_config.server.port = port;
    effective_config.server.bind_address = "127.0.0.1".to_string();
    
    let temp_dir_holder: Option<TempDir>;

    if enable_static_files {
        let temp_static_dir = tempdir().unwrap();
        let static_files_path = temp_static_dir.path().to_path_buf();
        effective_config.server.static_files_path = Some(static_files_path.to_str().unwrap().to_string());
        temp_dir_holder = Some(temp_static_dir);
    } else {
        effective_config.server.static_files_path = None;
        temp_dir_holder = None;
    }

    let agent = BidirectionalAgent::new(effective_config.clone()).expect("Failed to create agent for test");
    
    let server_url = format!("http://127.0.0.1:{}", port);
    info!("Test server starting on URL: {}", server_url);

    let server_handle = tokio::spawn(async move {
        // The run method itself will log.
        agent.run().await
    });
    
    // Give the server a moment to start. In a real scenario, you might use a more robust check.
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    (server_url, server_handle, temp_dir_holder)
}


#[tokio::test]
async fn test_serve_index_html_root_path() {
    let mut config = BidirectionalAgentConfig::default();
    config.mode.repl = false; // Ensure not in REPL mode for server to run
    config.registry.registry_only_mode = true; // Use placeholder LLM

    let (server_url, server_handle, temp_dir_option) = start_test_server(config, true).await;
    let temp_dir = temp_dir_option.expect("Temp directory should be created when static files are enabled");
    let static_dir = temp_dir.path();

    // Create a dummy index.html
    let index_path = static_dir.join("index.html");
    let mut file = File::create(&index_path).expect("Failed to create test index.html");
    file.write_all(b"<h1>Hello Test World</h1>").unwrap();

    let client = reqwest::Client::new();
    let res = client.get(&server_url).send().await.unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::OK);
    assert_eq!(
        res.headers().get(reqwest::header::CONTENT_TYPE).unwrap(),
        "text/html"
    );
    assert_eq!(res.text().await.unwrap(), "<h1>Hello Test World</h1>");

    server_handle.abort(); 
}

#[tokio::test]
async fn test_serve_index_html_explicit_path() {
    let mut config = BidirectionalAgentConfig::default();
    config.mode.repl = false;
    config.registry.registry_only_mode = true; // Use placeholder LLM

    let (server_url, server_handle, temp_dir_option) = start_test_server(config, true).await;
    let temp_dir = temp_dir_option.expect("Temp directory should be created when static files are enabled");
    let static_dir = temp_dir.path();

    let index_path = static_dir.join("index.html");
    let mut file = File::create(&index_path).expect("Failed to create test index.html for explicit path");
    file.write_all(b"<h1>Explicit Path Test</h1>").unwrap();

    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/index.html", server_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::OK);
    assert_eq!(
        res.headers().get(reqwest::header::CONTENT_TYPE).unwrap(),
        "text/html"
    );
    assert_eq!(res.text().await.unwrap(), "<h1>Explicit Path Test</h1>");
    server_handle.abort();
}

#[tokio::test]
async fn test_serve_static_file_not_found_for_index() {
    // This test specifically checks 404 for index.html if it's missing
    let mut config = BidirectionalAgentConfig::default();
    config.mode.repl = false;
    config.registry.registry_only_mode = true; // Use placeholder LLM

    // temp_dir will be created, but we won't put index.html in it
    let (server_url, server_handle, _temp_dir_option) = start_test_server(config, true).await;

    let client = reqwest::Client::new();
    // Request root, expecting index.html, which doesn't exist
    let res = client
        .get(&server_url) 
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);
    server_handle.abort();
}


#[tokio::test]
async fn test_get_non_existent_static_file_falls_to_api_error() {
    // This test checks that a GET for a non-index, non-existent static file
    // falls through to the jsonrpc_handler and results in a JSON-RPC error.
    let mut config = BidirectionalAgentConfig::default();
    config.mode.repl = false;
    config.registry.registry_only_mode = true; // Use placeholder LLM

    let (server_url, server_handle, _temp_dir_option) = start_test_server(config, true).await;

    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/nonexistent.css", server_url))
        .send()
        .await
        .unwrap();
    
    // With the new rules, a GET request for a non-index static file, when static_files_root is configured,
    // should result in a 403 Forbidden.
    assert_eq!(res.status(), reqwest::StatusCode::FORBIDDEN);
    // Optionally, check the body content if it's relevant for a 403 page.
    // For example:
    // let body_text = res.text().await.unwrap();
    // assert!(body_text.contains("404 Not Found"));

    server_handle.abort();
}


#[tokio::test]
async fn test_a2a_post_request_still_works() {
    let mut config = BidirectionalAgentConfig::default();
    config.mode.repl = false;
    config.registry.registry_only_mode = true; 

    let (server_url, server_handle, _temp_dir_option) = start_test_server(config, true).await;

    let client = reqwest::Client::new(); // Use async client
    let a2a_payload = serde_json::json!({
        "jsonrpc": "2.0",
        "id": "test-123",
        "method": "tasks/send",
        "params": {
            "id": "task-abc",
            "message": {
                "role": "user",
                "parts": [{"type": "text", "text": "list agents"}]
            }
        }
    });

    let res = client.post(&server_url).json(&a2a_payload).send().await.unwrap(); // Use .await

    assert_eq!(res.status(), reqwest::StatusCode::OK);
    let body: serde_json::Value = res.json().await.unwrap(); // Use .await
    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["id"], "test-123");
    
    if let Some(result) = body.get("result") {
        assert!(result.get("id").is_some());
        assert!(result.get("status").expect("Status field should exist in result").is_object());
        if let Some(status_message_parts) = result.pointer("/status/message/parts") {
             let text_part = status_message_parts[0]["text"].as_str().unwrap();
             // The RegistryRouter returns "No agents registered yet..." when the directory is empty.
             assert!(text_part.contains("No agents registered yet."));
        } else {
            panic!("Expected status.message.parts in A2A response: {:?}", result);
        }
    } else {
         panic!("Expected successful A2A result, got error: {:?}", body.get("error"));
    }

    server_handle.abort();
}

#[tokio::test]
async fn test_serve_other_static_file() {
    let mut config = BidirectionalAgentConfig::default();
    config.mode.repl = false;
    config.registry.registry_only_mode = true; // Use placeholder LLM

    let (server_url, server_handle, temp_dir_option) = start_test_server(config, true).await;
    let temp_dir = temp_dir_option.expect("Temp directory should be created when static files are enabled");
    let static_dir = temp_dir.path();

    // Create a dummy style.css
    let css_path = static_dir.join("style.css");
    let mut file = File::create(&css_path).expect("Failed to create test style.css");
    file.write_all(b"body { color: blue; }").unwrap();

    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/style.css", server_url))
        .send()
        .await
        .unwrap();

    // This request should now be forbidden as only index.html is allowed.
    assert_eq!(res.status(), reqwest::StatusCode::FORBIDDEN);
    // The body and content type assertions are no longer relevant for a 403.
    // assert_eq!(
    //     res.headers().get(reqwest::header::CONTENT_TYPE).unwrap(),
    //     "text/css" 
    // );
    // assert_eq!(res.text().await.unwrap(), "body { color: blue; }");
    server_handle.abort();
}

#[tokio::test]
async fn test_path_traversal_prevention() {
    let mut config = BidirectionalAgentConfig::default();
    config.mode.repl = false;
    config.registry.registry_only_mode = true; // Use placeholder LLM

    let (server_url, server_handle, _temp_dir_option) = start_test_server(config, true).await;

    // Create a dummy file in a *separate* temporary directory
    let secret_temp_dir = tempdir().unwrap();
    let secret_file_path = secret_temp_dir.path().join("secret.txt");
    let mut file = File::create(&secret_file_path).expect("Failed to create secret.txt for traversal test");
    file.write_all(b"This is a secret!").unwrap();
    
    // Calculate relative path from static_dir to secret_file_path's parent
    // This is tricky because tempdir() creates dirs with random names.
    // For simplicity, we'll just use a known traversal attempt.
    // The exact number of "../" might need adjustment if static_dir is nested.
    // Assuming static_dir is something like /tmp/.tmpXXXXXX/
    // and secret_file_path is /tmp/.tmpYYYYYY/secret.txt
    // A direct relative path is hard. Let's try a common pattern.
    // The canonicalization check should prevent this.

    let client = reqwest::Client::new();
    // This path is unlikely to resolve correctly to an existing file outside the root
    // due to canonicalization of `static_dir` itself.
    // The test is more about ensuring `../` doesn't grant access.
    // Create a URL with a more explicit path traversal pattern
    // Use %2e%2e to avoid URL normalization (encodes "..")
    let url_with_traversal = format!("{}/%2e%2e/%2e%2e/%2e%2e/%2e%2e/%2e%2e/%2e%2e/etc/passwd", server_url);
    println!("Testing URL with encoded traversal: {}", url_with_traversal);
    
    let res = client
        .get(&url_with_traversal) // URL-encoded traversal attempt
        .send()
        .await
        .unwrap();
        
    // Store the status before consuming the body
    let status = res.status();
    println!("Response status: {}", status);
    
    // Now we can consume the body
    let body = res.text().await.unwrap_or_default();
    println!("Response body: {:?}", body);

    // Expect 403 Forbidden if canonicalization detects traversal,
    // or 404 if the path resolves but file not found (less likely for such a deep traversal to hit something valid by chance)
    // or potentially a JSON-RPC error if it falls through.
    // Given the canonicalization check, 403 is the most direct expectation if it works.
    // If canonicalization of `file_path_to_serve` fails (e.g. `../../.../etc/passwd` doesn't exist from `static_dir`),
    // it falls to `TokioFile::open`, which would give 404.
    // If `base_path.canonicalize()` fails (e.g. static_dir was deleted mid-request), it also falls to `TokioFile::open`.

    // The path canonicalization logic:
    // `file_path_to_serve.canonicalize()` on `static_dir/../../../../../../../../../../etc/passwd`
    // If `static_dir` is `/tmp/somerandomdir`, then `file_path_to_serve` becomes `/etc/passwd`.
    // `base_path.canonicalize()` on `static_dir` becomes `/tmp/somerandomdir`.
    // Then `canon_file_path.starts_with(canon_base_path)` (`/etc/passwd`.starts_with(`/tmp/somerandomdir`)) is false.
    // This should lead to a 403.

    // This test's purpose is to ensure path traversal doesn't succeed in accessing files outside the root.
    // The specific path traversal attempt `/%2e%2e/...` should be caught by the initial check
    // `req_path.to_lowercase().contains("%2e%2e")` in `agent_helpers.rs`, resulting in a 403.
    assert_eq!(status, reqwest::StatusCode::FORBIDDEN);
    
    server_handle.abort();
}

// --- Tests for Static Files Disabled ---

#[tokio::test]
async fn test_get_root_when_static_disabled_returns_404() {
    let mut config = BidirectionalAgentConfig::default();
    config.mode.repl = false;
    config.registry.registry_only_mode = true;

    // Start server with static files disabled
    let (server_url, server_handle, _temp_dir_option) = start_test_server(config, false).await;

    let client = reqwest::Client::new();
    let res = client.get(&server_url).send().await.unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);
    server_handle.abort();
}

#[tokio::test]
async fn test_get_index_html_when_static_disabled_returns_404() {
    let mut config = BidirectionalAgentConfig::default();
    config.mode.repl = false;
    config.registry.registry_only_mode = true;

    let (server_url, server_handle, _temp_dir_option) = start_test_server(config, false).await;

    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/index.html", server_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);
    server_handle.abort();
}

#[tokio::test]
async fn test_get_other_file_when_static_disabled_returns_404() {
    let mut config = BidirectionalAgentConfig::default();
    config.mode.repl = false;
    config.registry.registry_only_mode = true;

    let (server_url, server_handle, _temp_dir_option) = start_test_server(config, false).await;

    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/style.css", server_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);
    server_handle.abort();
}

#[tokio::test]
async fn test_get_agent_json_when_static_disabled_works() {
    let mut config = BidirectionalAgentConfig::default();
    config.mode.repl = false;
    config.registry.registry_only_mode = true; // Even in registry_only_mode, agent.json should be served

    let (server_url, server_handle, _temp_dir_option) = start_test_server(config, false).await;

    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/.well-known/agent.json", server_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::OK);
    assert_eq!(
        res.headers().get(reqwest::header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    // Try to parse as AgentCard to ensure it's valid
    let card: Result<AgentCard, _> = res.json().await;
    assert!(card.is_ok(), "Response should be a valid AgentCard. Error: {:?}", card.err());
    
    // Verify some basic fields if the card was parsed
    if let Ok(parsed_card) = card {
        assert!(!parsed_card.name.is_empty());
        assert!(!parsed_card.url.is_empty());
    }

    server_handle.abort();
}

// --- Additional Tests for Static Files Enabled ---

#[tokio::test]
async fn test_get_agent_json_when_static_enabled_works() {
    let mut config = BidirectionalAgentConfig::default();
    config.mode.repl = false;
    config.registry.registry_only_mode = true;

    // Start server with static files enabled (even if no files are present)
    let (server_url, server_handle, _temp_dir_option) = start_test_server(config, true).await;

    let client = reqwest::Client::new();
    let res = client
        .get(format!("{}/.well-known/agent.json", server_url))
        .send()
        .await
        .unwrap();

    assert_eq!(res.status(), reqwest::StatusCode::OK);
    assert_eq!(
        res.headers().get(reqwest::header::CONTENT_TYPE).unwrap(),
        "application/json"
    );
    let card: Result<AgentCard, _> = res.json().await;
    assert!(card.is_ok(), "Response should be a valid AgentCard. Error: {:?}", card.err());

    if let Ok(parsed_card) = card {
        assert!(!parsed_card.name.is_empty());
        assert!(!parsed_card.url.is_empty());
    }
    
    server_handle.abort();
}
