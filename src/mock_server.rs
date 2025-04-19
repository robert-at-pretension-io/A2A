use hyper::{Body, Request, Response, Server};
use hyper::service::{make_service_fn, service_fn};
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::runtime::Runtime;
use serde_json::{json, Value};

// Mock handlers for A2A endpoints
async fn handle_a2a_request(req: Request<Body>) -> Result<Response<Body>, Infallible> {
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
                let task_id = match request.get("params").and_then(|p| p.get("id")).and_then(|id| id.as_str()) {
                    Some(id) => id,
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
