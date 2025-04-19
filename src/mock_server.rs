use hyper::{Body, Request, Response, Server, StatusCode};
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::service::{make_service_fn, service_fn};
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;
use tokio::time::{sleep, Duration};
use futures_util::stream::{self, StreamExt};
use serde_json::{json, Value};
use crate::types::{AgentCard, AgentSkill, AgentCapabilities, AgentAuthentication, 
                  PushNotificationConfig, TaskPushNotificationConfig, AuthenticationInfo};

// Mock handlers for A2A endpoints
async fn handle_a2a_request(req: Request<Body>) -> Result<Response<Body>, Infallible> {
    // Check if this is a request for agent card
    // Check for Accept header to see if client wants SSE
    let accept_header = req.headers().get("Accept")
                           .and_then(|h| h.to_str().ok())
                           .unwrap_or("");
                           
    if req.uri().path() == "/.well-known/agent.json" {
        // Create the agent card using the proper types
        let skill = AgentSkill {
            id: "test-skill-1".to_string(),
            name: "Echo".to_string(),
            description: Some("Echoes back any message sent".to_string()),
            tags: None,
            examples: None,
            input_modes: None,
            output_modes: None,
        };
        
        let capabilities = AgentCapabilities {
            streaming: true,
            push_notifications: true,
            state_transition_history: true,
        };
        
        let authentication = AgentAuthentication {
            schemes: vec!["None".to_string()],
            credentials: None,
        };
        
        let agent_card = AgentCard {
            name: "Mock A2A Server".to_string(),
            description: Some("A mock server for testing A2A protocol clients".to_string()),
            url: "http://localhost:8080".to_string(),
            provider: None,
            version: "0.1.0".to_string(),
            documentation_url: None,
            capabilities,
            authentication: Some(authentication),
            default_input_modes: vec!["text/plain".to_string()],
            default_output_modes: vec!["text/plain".to_string()],
            skills: vec![skill],
        };
        
        let json = serde_json::to_string(&agent_card).unwrap();
        return Ok(Response::new(Body::from(json)));
    }
    
    // Handle JSON-RPC requests
    let body_bytes = hyper::body::to_bytes(req.into_body()).await.unwrap();
    let request: Value = match serde_json::from_slice(&body_bytes) {
        Ok(req) => req,
        Err(e) => {
            let error_response = json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {
                    "code": -32700,
                    "message": format!("Parse error: {}", e),
                }
            });
            let json = serde_json::to_string(&error_response).unwrap();
            return Ok(Response::new(Body::from(json)));
        }
    };
    
    // Check method to determine response
    if let Some(method) = request.get("method").and_then(|m| m.as_str()) {
        match method {
            "tasks/send" => {
                let task_id = "mock-task-123".to_string();
                
                // Create a SendTaskResponse with a Task
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "id": task_id,
                        "sessionId": "mock-session-456",
                        "status": {
                            "state": "submitted",
                            "timestamp": chrono::Utc::now().to_rfc3339()
                        }
                    }
                });
                
                let json = serde_json::to_string(&response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "tasks/get" => {
                // Extract task ID from request params
                let task_id_opt = request.get("params").and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let task_id = match task_id_opt {
                    Some(id) => id.to_string(), // Clone the string to avoid borrowing request
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing task id"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Return a mock task
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "id": task_id,
                        "sessionId": "mock-session-456",
                        "status": {
                            "state": "completed", 
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                            "message": {
                                "role": "agent",
                                "parts": [
                                    {
                                        "type": "text",
                                        "text": "Task completed successfully!"
                                    }
                                ]
                            }
                        },
                        "artifacts": []
                    }
                });
                
                let json = serde_json::to_string(&response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "tasks/cancel" => {
                // Extract task ID from request params
                let task_id_opt = request.get("params").and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let task_id = match task_id_opt {
                    Some(id) => id.to_string(), // Clone the string to avoid borrowing request
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing task id"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Return success response
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "id": task_id
                    }
                });
                
                let json = serde_json::to_string(&response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "tasks/sendSubscribe" => {
                // This is a streaming endpoint, respond with SSE
                let id = request.get("id").unwrap_or(&json!(null)).clone();
                let task_id = format!("stream-task-{}", chrono::Utc::now().timestamp_millis());
                
                // Create a streaming channel
                let (tx, rx) = mpsc::channel::<String>(32);
                
                // Spawn a task to generate streaming events
                tokio::spawn(async move {
                    // Initial working status
                    let status_update = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "id": task_id,
                            "sessionId": "stream-session-1",
                            "status": {
                                "state": "working",
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            },
                            "final": false
                        }
                    });
                    
                    // Send status update
                    let _ = tx.send(format!("data: {}\n\n", status_update.to_string())).await;
                    sleep(Duration::from_millis(500)).await;
                    
                    // Simulate content streaming 
                    let content_parts = vec![
                        "This is ", "the first ", "part of ", "the streaming ", 
                        "response from ", "the mock A2A server.\n\n",
                        "Notice how ", "the text ", "is chunked ", "into multiple ",
                        "SSE events!"
                    ];
                    
                    for (i, part) in content_parts.iter().enumerate() {
                        let is_last = i == content_parts.len() - 1;
                        
                        let artifact_update = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "artifact": {
                                    "parts": [{
                                        "type": "text",
                                        "text": part
                                    }],
                                    "index": 0,
                                    "append": i > 0,
                                    "lastChunk": is_last
                                }
                            }
                        });
                        
                        // Send artifact content chunk
                        let _ = tx.send(format!("data: {}\n\n", artifact_update.to_string())).await;
                        sleep(Duration::from_millis(300)).await;
                    }
                    
                    // Final completed status update
                    let final_update = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "id": task_id,
                            "sessionId": "stream-session-1",
                            "status": {
                                "state": "completed",
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            },
                            "final": true
                        }
                    });
                    
                    // Send final status update
                    let _ = tx.send(format!("data: {}\n\n", final_update.to_string())).await;
                });
                
                // Create a streaming response body by mapping the receiver to Result<String, Infallible>
                let mapped_stream = tokio_stream::wrappers::ReceiverStream::new(rx)
                    .map(|chunk| Ok::<_, Infallible>(chunk));
                
                // Wrap the mapped stream
                let stream_body = Body::wrap_stream(mapped_stream);
                
                // Create a Server-Sent Events response
                let mut response = Response::new(stream_body);
                response.headers_mut().insert(
                    CONTENT_TYPE, 
                    HeaderValue::from_static("text/event-stream")
                );
                
                return Ok(response);
            },
            "tasks/resubscribe" => {
                // Extract task ID from request params
                let task_id_opt = request.get("params").and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let task_id = match task_id_opt {
                    Some(id) => id.to_string(), // Clone the string to avoid borrowing request
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing task id"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                let id = request.get("id").unwrap_or(&json!(null)).clone();
                
                // Create a streaming channel
                let (tx, rx) = mpsc::channel::<String>(32);
                
                // Spawn a task to generate streaming events for resubscribe
                tokio::spawn(async move {
                    // Initial status update - pick up where we left off
                    let status_update = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "id": task_id,
                            "sessionId": "stream-session-1",
                            "status": {
                                "state": "working",
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            },
                            "final": false
                        }
                    });
                    
                    // Send status update
                    let _ = tx.send(format!("data: {}\n\n", status_update.to_string())).await;
                    sleep(Duration::from_millis(300)).await;
                    
                    // Send some continuation content
                    let content = "Continuing from where we left off... here's some more content!";
                    
                    let artifact_update = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "artifact": {
                                "parts": [{
                                    "type": "text",
                                    "text": content
                                }],
                                "index": 0,
                                "append": true,
                                "lastChunk": true
                            }
                        }
                    });
                    
                    // Send artifact content
                    let _ = tx.send(format!("data: {}\n\n", artifact_update.to_string())).await;
                    sleep(Duration::from_millis(300)).await;
                    
                    // Final completed status update
                    let final_update = json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "result": {
                            "id": task_id,
                            "sessionId": "stream-session-1",
                            "status": {
                                "state": "completed",
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            },
                            "final": true
                        }
                    });
                    
                    // Send final status update
                    let _ = tx.send(format!("data: {}\n\n", final_update.to_string())).await;
                });
                
                // Create a streaming response body by mapping the receiver to Result<String, Infallible>
                let mapped_stream = tokio_stream::wrappers::ReceiverStream::new(rx)
                    .map(|chunk| Ok::<_, Infallible>(chunk));
                
                // Wrap the mapped stream
                let stream_body = Body::wrap_stream(mapped_stream);
                
                // Create a Server-Sent Events response
                let mut response = Response::new(stream_body);
                response.headers_mut().insert(
                    CONTENT_TYPE, 
                    HeaderValue::from_static("text/event-stream")
                );
                
                return Ok(response);
            },
            "tasks/pushNotification/set" => {
                // Extract task ID and push notification config from request params
                let task_id_opt = request.get("params").and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let task_id = match task_id_opt {
                    Some(id) => id.to_string(),
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing task id"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Check for the push notification config
                let push_config = match request.get("params").and_then(|p| p.get("pushNotificationConfig")) {
                    Some(config) => config,
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing push notification config"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Verify the webhook URL
                let webhook_url = match push_config.get("url").and_then(|u| u.as_str()) {
                    Some(url) => url,
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing webhook URL in push notification config"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // In a real implementation, we would verify the webhook by sending a challenge
                // But for the mock server, we'll just log it
                println!("Setting push notification webhook for task {}: {}", task_id, webhook_url);
                
                // Return success response
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": {
                        "id": task_id
                    }
                });
                
                let json = serde_json::to_string(&response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            "tasks/pushNotification/get" => {
                // Extract task ID from request params
                let task_id_opt = request.get("params").and_then(|p| p.get("id")).and_then(|id| id.as_str());
                let task_id = match task_id_opt {
                    Some(id) => id.to_string(),
                    None => {
                        let error = json!({
                            "jsonrpc": "2.0",
                            "id": request.get("id"),
                            "error": {
                                "code": -32602,
                                "message": "Invalid parameters: missing task id"
                            }
                        });
                        let json = serde_json::to_string(&error).unwrap();
                        return Ok(Response::new(Body::from(json)));
                    }
                };
                
                // Create a mock response using the proper types
                let auth_info = AuthenticationInfo {
                    schemes: vec!["Bearer".to_string()],
                    credentials: None,
                    extra: serde_json::Map::new(),
                };
                
                let push_config = PushNotificationConfig {
                    url: "https://example.com/webhook".to_string(),
                    authentication: Some(auth_info),
                    token: Some("mock-token-123".to_string()),
                };
                
                let task_push_config = TaskPushNotificationConfig {
                    id: task_id,
                    push_notification_config: push_config,
                };
                
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "result": task_push_config
                });
                
                let json = serde_json::to_string(&response).unwrap();
                return Ok(Response::new(Body::from(json)));
            },
            // Add more method handlers here
            _ => {
                // Return method not found error
                let error_response = json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id"),
                    "error": {
                        "code": -32601,
                        "message": "Method not found",
                    }
                });
                
                let json = serde_json::to_string(&error_response).unwrap();
                return Ok(Response::new(Body::from(json)));
            }
        }
    }
    
    // Return invalid request error
    let error_response = json!({
        "jsonrpc": "2.0",
        "id": null,
        "error": {
            "code": -32600,
            "message": "Invalid request",
        }
    });
    
    let json = serde_json::to_string(&error_response).unwrap();
    Ok(Response::new(Body::from(json)))
}

pub fn start_mock_server(port: u16) {
    let rt = Runtime::new().unwrap();
    
    rt.block_on(async {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        
        let make_svc = make_service_fn(|_conn| async {
            Ok::<_, Infallible>(service_fn(handle_a2a_request))
        });
        
        let server = Server::bind(&addr).serve(make_svc);
        
        println!("📡 Mock A2A server running at http://{}", addr);
        println!("Press Ctrl+C to stop the server...");
        
        if let Err(e) = server.await {
            eprintln!("Server error: {}", e);
        }
    });
}