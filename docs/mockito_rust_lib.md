I'll look up the official documentation for the latest version of the mockito Rust crate and provide you with comprehensive technical documentation based on authoritative sources.

Let me check the latest documentation for the mockito crate on docs.rs:

Let me get some additional documentation to ensure I have the most comprehensive information:

Let me check the GitHub repository to get the complete picture:

Let me check the latest release information to confirm the current version:

Now I'll compile all this information into a comprehensive technical documentation for the Rust mockito library in its latest version (1.7.0):

# Mockito Rust Documentation (v1.7.0)

## Overview

Mockito is a powerful HTTP mocking library for Rust designed for integration testing and offline development. It runs local HTTP servers which create, deliver, and remove mocks as needed. This documentation covers the latest version (1.7.0) released on March 4, 2025.

## Key Features

- HTTP/1 and HTTP/2 support
- Parallel test execution
- Comprehensive request matching (regex, JSON, query parameters, etc.)
- Request verification (spy functionality)
- Multiple host simulation
- Synchronous and asynchronous interfaces
- Detailed error reporting with colored diffs
- Simple, intuitive API

## Installation

Add mockito to your `Cargo.toml`:

```toml
[dependencies]
mockito = "1.7.0"
```

For test-only usage (recommended):

```toml
[dev-dependencies]
mockito = "1.7.0"
```

## Getting Started

### Basic Usage

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_something() {
        // Request a new server from the pool
        let mut server = mockito::Server::new();
        
        // Get server address information
        let host = server.host_with_port();
        let url = server.url();
        
        // Create a mock
        let mock = server.mock("GET", "/hello")
            .with_status(201)
            .with_header("content-type", "text/plain")
            .with_header("x-api-key", "1234")
            .with_body("world")
            .create();
        
        // Any requests to GET /hello will now respond with:
        // - Status: 201
        // - Headers: content-type: text/plain, x-api-key: 1234
        // - Body: world
        
        // Verify the mock was called as expected
        mock.assert();
    }
}
```

### Multiple Different Responses for the Same Endpoint

```rust
#[test]
fn test_different_responses() {
    let mut server = mockito::Server::new();
    
    server.mock("GET", "/greetings")
        .match_header("content-type", "application/json")
        .match_body(mockito::Matcher::PartialJsonString(
            "{\"greeting\": \"hello\"}".to_string()
        ))
        .with_body("hello json")
        .create();
    
    server.mock("GET", "/greetings")
        .match_header("content-type", "application/text")
        .match_body(mockito::Matcher::Regex(
            "greeting=hello".to_string()
        ))
        .with_body("hello text")
        .create();
    
    // JSON and Text requests will receive different responses
}
```

### Simulating Multiple Hosts

```rust
#[test]
fn test_multiple_hosts() {
    let mut twitter = mockito::Server::new();
    let mut github = mockito::Server::new();
    
    // These mocks will be available at twitter.url()
    let twitter_mock = twitter.mock("GET", "/api").create();
    
    // These mocks will be available at github.url()
    let github_mock = github.mock("GET", "/api").create();
    
    // Use twitter.url() and github.url() to configure clients
}
```

## Server Management

### Server Creation

Mockito provides different ways to create a server:

```rust
// From the server pool (preferred for tests)
let mut server = mockito::Server::new();

// Custom configuration
let opts = mockito::ServerOpts {
    host: "0.0.0.0",
    port: 1234,
    assert_on_drop: true,
    ..Default::default()
};
let mut server = mockito::Server::new_with_opts(opts);
```

### Available Server Methods

```rust
// Get the host and port as a string (e.g., "127.0.0.1:1234")
server.host_with_port()

// Get the full URL (e.g., "http://127.0.0.1:1234")
server.url()

// Get the raw socket address
server.socket_address()

// Reset all mocks on this server
server.reset()
```

### Server Options

The `ServerOpts` struct allows customizing:

- `host`: Hostname to bind to (default: "127.0.0.1")
- `port`: Port to bind to (default: random free port)
- `assert_on_drop`: Automatically call `Mock::assert()` before dropping a mock (default: false)

## Mock Creation and Configuration

### Basic Mock Setup

```rust
let mock = server.mock("GET", "/path")
    .with_status(200)
    .with_header("content-type", "application/json")
    .with_body("{\"message\": \"success\"}")
    .create();
```

### Response Configuration

```rust
// Set response status code (defaults to 200)
.with_status(201)

// Add response headers
.with_header("content-type", "application/json")

// Set a dynamic header based on the request
.with_header_from_request("x-request-id", |request| {
    // Extract and modify information from the request
    request.headers.get("x-correlation-id").map(|v| v.to_str().unwrap_or(""))
        .unwrap_or("").to_string()
})

// Set response body as a string
.with_body("Hello, world!")

// Set response body from a file
.with_body_from_file("tests/data/response.json")

// Set chunked response body
.with_chunked_body(vec!["chunk1".into(), "chunk2".into()])

// Add delay to response
.with_delay(std::time::Duration::from_millis(100))
```

## Request Matching

Mockito provides several ways to match incoming requests to the appropriate mock response.

### Path and Query Matching

```rust
// Exact path matching
server.mock("GET", "/hello")

// Exact path and query
server.mock("GET", "/hello?world=1")

// Path with regex
server.mock("GET", mockito::Matcher::Regex(r"^/hello/\d+$".to_string()))

// Match any path
server.mock("GET", mockito::Matcher::Any)
```

### Query Parameter Matching

```rust
// Match specific URL-encoded query parameter
.match_query(mockito::Matcher::UrlEncoded(
    "greeting".into(),
    "good day".into()
))

// Match multiple URL-encoded query parameters
.match_query(mockito::Matcher::AllOf(vec![
    mockito::Matcher::UrlEncoded("hello".into(), "world".into()),
    mockito::Matcher::UrlEncoded("greeting".into(), "good day".into())
]))

// Match query with regex
.match_query(mockito::Matcher::Regex("hello=world".into()))

// Match any query
.match_query(mockito::Matcher::Any)
```

### Header Matching

```rust
// Exact header match (case-insensitive for header name)
.match_header("content-type", "application/json")

// Header with regex pattern
.match_header("content-type", mockito::Matcher::Regex(r".*json.*".to_string()))

// Match any header value (header must exist)
.match_header("content-type", mockito::Matcher::Any)

// Match when header is missing
.match_header("authorization", mockito::Matcher::Missing)
```

### Body Matching

```rust
// Exact body match
.match_body("hello")

// Regex body match
.match_body(mockito::Matcher::Regex("hello".to_string()))

// JSON exact match
.match_body(mockito::Matcher::Json(serde_json::json!({
    "hello": "world"
})))

// JSON string exact match
.match_body(mockito::Matcher::JsonString(r#"{"hello": "world"}"#.to_string()))

// Partial JSON match
.match_body(mockito::Matcher::PartialJson(serde_json::json!({
    "hello": "world"
})))

// Partial JSON string match
.match_body(mockito::Matcher::PartialJsonString(r#"{"hello": "world"}"#.to_string()))
```

### Composite Matchers

```rust
// Match any of the provided matchers
.match_body(mockito::Matcher::AnyOf(vec![
    mockito::Matcher::Exact("hello=world".to_string()),
    mockito::Matcher::JsonString(r#"{"hello": "world"}"#.to_string()),
]))

// Match all of the provided matchers
.match_body(mockito::Matcher::AllOf(vec![
    mockito::Matcher::Regex("hello".to_string()),
    mockito::Matcher::Regex("world".to_string()),
]))
```

### Custom Request Matching

```rust
// Access the full request object for custom matching logic
.match_request(|request| {
    request.has_header("x-test") && 
    request.utf8_lossy_body().unwrap().contains("hello")
})
```

## Verification

Mockito can verify that your mocks were called as expected:

```rust
// Verify mock was called exactly once (default)
mock.assert();

// Verify mock was called exactly n times
mock.expect(3).create();
mock.assert();

// Verify mock was called at least n times
mock.expect_at_least(2).create();
mock.assert();

// Verify mock was called at most n times
mock.expect_at_most(4).create();
mock.assert();

// Check if mock matched without panicking (returns bool)
if mock.matched() {
    // Mock was called correctly
} else {
    // Mock wasn't called or was called too many times
}
```

If a verification fails, a detailed error message is shown, including a colored diff of the last unmatched request.

## Async Support

Mockito provides async equivalents for most operations:

```rust
#[tokio::test]
async fn test_async() {
    let mut server = mockito::Server::new_async().await;
    
    let mock = server.mock("GET", "/hello")
        .with_body("world")
        .create_async().await;
    
    // Make async HTTP requests here
    
    mock.assert_async().await;
}
```

Available async methods:
- `Server::new_async()`
- `Server::new_with_opts_async()`
- `Mock::create_async()`
- `Mock::assert_async()`
- `Mock::matched_async()`
- `Mock::remove_async()`

## Mock Lifetime and Cleanup

Mocks are available throughout the lifetime of their server. When a server goes out of scope, all associated mocks are removed.

```rust
// Manual cleanup of individual mocks
mock.remove();

// Reset all mocks on a server
server.reset();

// Automatic cleanup (mocks are removed when server is dropped)
{
    let mut server = mockito::Server::new();
    let mock = server.mock("GET", "/").create();
    // Mock active here
}
// Server and mock are gone here, requests will fail
```

With `assert_on_drop: true` server option, mocks will automatically call `assert()` before being dropped.

## Debugging

Enable debug logs for troubleshooting:

```rust
#[test]
fn test_with_debug() {
    let _ = env_logger::try_init();
    // Test code here
}
```

Run tests with:

```
RUST_LOG=mockito=debug cargo test
```

## Standalone Server

Run a standalone mock server:

```rust
fn main() {
    let opts = mockito::ServerOpts {
        host: "0.0.0.0",
        port: 1234,
        ..Default::default()
    };
    let mut server = mockito::Server::new_with_opts(opts);
    let _m = server.mock("GET", "/")
        .with_body("hello world")
        .create();
    
    // Keep server running
    loop {}
}
```

## Technical Requirements

- Minimum supported Rust version: 1.70.0
- The server pool is limited to 20 servers for macOS targets to prevent hitting file descriptor limits

## Example Integration with Popular HTTP Clients

### reqwest

```rust
#[tokio::test]
async fn test_with_reqwest() {
    let mut server = mockito::Server::new_async().await;
    
    let mock = server.mock("GET", "/users")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"users": [{"id": 1, "name": "John"}]}"#)
        .create_async().await;
    
    let client = reqwest::Client::new();
    let response = client.get(&format!("{}/users", server.url()))
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    
    assert_eq!(response["users"][0]["name"], "John");
    mock.assert_async().await;
}
```

## Recent Changes in 1.7.0

- Made parking_lot optional
- Updated rand crate to v0.9
- Allowed colored crate v3

## Additional Resources

- Full API documentation: [docs.rs/mockito](https://docs.rs/mockito)
- Source code: [github.com/lipanski/mockito](https://github.com/lipanski/mockito)