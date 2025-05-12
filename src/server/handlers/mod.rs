use crate::server::create_agent_card;
use crate::server::services::notification_service::NotificationService;
use crate::server::services::streaming_service::StreamingService;
use crate::server::services::task_service::TaskService;
use crate::server::ServerError;
use crate::types::{TaskIdParams, TaskPushNotificationConfig, TaskQueryParams, TaskSendParams};
use hyper::{header, Body, Request, Response, StatusCode};
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;

/// Placeholder for CORS headers (now handled by Nginx)
///
/// This function previously added CORS headers, but now they are managed by Nginx.
/// We keep it as a no-op to avoid refactoring all call sites.
fn add_cors_headers(builder: hyper::http::response::Builder) -> hyper::http::response::Builder {
    // No CORS headers are added here - they're now handled by Nginx
    builder
}

// JSON-RPC response structure
#[derive(serde::Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(serde::Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl JsonRpcResponse {
    fn success(id: Value, result: impl serde::Serialize) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!(result)),
            error: None,
        }
    }

    fn error(id: Value, error: ServerError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code: error.code(),
                message: error.to_string(),
            }),
        }
    }
}

/// Main handler for JSON-RPC requests
pub async fn jsonrpc_handler(
    req: Request<Body>,
    task_service: Arc<TaskService>,
    streaming_service: Arc<StreamingService>,
    notification_service: Arc<NotificationService>,
) -> Result<Response<Body>, Infallible> {
    // Handle .well-known/agent.json requests
    if req.uri().path() == "/.well-known/agent.json" {
        let agent_card = create_agent_card(None, None, None, None, None);
        return Ok(add_cors_headers(Response::builder())
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(serde_json::to_string(&agent_card).unwrap()))
            .unwrap());
    }

    // Extract headers first (copy what we need)
    let accepts_sse = req
        .headers()
        .get("Accept")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .contains("text/event-stream");

    // Now we can consume the request body
    let body_bytes = hyper::body::to_bytes(req.into_body()).await.unwrap();

    // Parse the request body as JSON-RPC
    match serde_json::from_slice::<serde_json::Value>(&body_bytes) {
        Ok(json_value) => {
            let method = json_value
                .get("method")
                .and_then(|m| m.as_str())
                .unwrap_or("");
            let id = json_value.get("id").cloned().unwrap_or(Value::Null);
            let params = json_value.get("params").cloned().unwrap_or(json!({}));

            // Check for streaming requests based on method or headers
            let _wants_streaming =
                accepts_sse || method == "tasks/sendSubscribe" || method == "tasks/resubscribe"; // Prefix unused variable

            // Dispatch to appropriate handler based on method
            match method {
                "tasks/send" => match serde_json::from_value::<TaskSendParams>(params) {
                    Ok(params) => match task_service.process_task(params).await {
                        Ok(task) => {
                            let response = JsonRpcResponse::success(id, task);
                            Ok(add_cors_headers(Response::builder())
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, "application/json")
                                .body(Body::from(serde_json::to_string(&response).unwrap()))
                                .unwrap())
                        }
                        Err(e) => {
                            let response = JsonRpcResponse::error(id, e);
                            Ok(add_cors_headers(Response::builder())
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, "application/json")
                                .body(Body::from(serde_json::to_string(&response).unwrap()))
                                .unwrap())
                        }
                    },
                    Err(e) => {
                        let response = JsonRpcResponse::error(
                            id,
                            ServerError::InvalidParameters(format!("Invalid parameters: {}", e)),
                        );
                        Ok(add_cors_headers(Response::builder())
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, "application/json")
                            .body(Body::from(serde_json::to_string(&response).unwrap()))
                            .unwrap())
                    }
                },
                "tasks/get" => match serde_json::from_value::<TaskQueryParams>(params) {
                    Ok(params) => match task_service.get_task(params).await {
                        Ok(task) => {
                            let response = JsonRpcResponse::success(id, task);
                            Ok(add_cors_headers(Response::builder())
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, "application/json")
                                .body(Body::from(serde_json::to_string(&response).unwrap()))
                                .unwrap())
                        }
                        Err(e) => {
                            let response = JsonRpcResponse::error(id, e);
                            Ok(add_cors_headers(Response::builder())
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, "application/json")
                                .body(Body::from(serde_json::to_string(&response).unwrap()))
                                .unwrap())
                        }
                    },
                    Err(e) => {
                        let response = JsonRpcResponse::error(
                            id,
                            ServerError::InvalidParameters(format!("Invalid parameters: {}", e)),
                        );
                        Ok(add_cors_headers(Response::builder())
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, "application/json")
                            .body(Body::from(serde_json::to_string(&response).unwrap()))
                            .unwrap())
                    }
                },
                "tasks/cancel" => match serde_json::from_value::<TaskIdParams>(params) {
                    Ok(params) => match task_service.cancel_task(params).await {
                        Ok(task) => {
                            let response = JsonRpcResponse::success(id, task);
                            Ok(add_cors_headers(Response::builder())
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, "application/json")
                                .body(Body::from(serde_json::to_string(&response).unwrap()))
                                .unwrap())
                        }
                        Err(e) => {
                            let response = JsonRpcResponse::error(id, e);
                            Ok(add_cors_headers(Response::builder())
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, "application/json")
                                .body(Body::from(serde_json::to_string(&response).unwrap()))
                                .unwrap())
                        }
                    },
                    Err(e) => {
                        let response = JsonRpcResponse::error(
                            id,
                            ServerError::InvalidParameters(format!("Invalid parameters: {}", e)),
                        );
                        Ok(add_cors_headers(Response::builder())
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, "application/json")
                            .body(Body::from(serde_json::to_string(&response).unwrap()))
                            .unwrap())
                    }
                },
                "tasks/setPushNotification" => {
                    match serde_json::from_value::<TaskPushNotificationConfig>(params)
                    {
                        Ok(config_params) => {
                            // Clone config_params before moving it
                            let config_for_response = config_params.clone();
                            match notification_service
                                .set_push_notification(config_params)
                                .await
                            {
                                Ok(_) => {
                                    // Use the cloned config
                                    let response = JsonRpcResponse::success(id, config_for_response);
                                    Ok(Response::builder()
                                        .status(StatusCode::OK)
                                        .header(header::CONTENT_TYPE, "application/json")
                                        .body(Body::from(serde_json::to_string(&response).unwrap()))
                                        .unwrap())
                                }
                                Err(e) => {
                                    let response = JsonRpcResponse::error(id, e);
                                    Ok(Response::builder()
                                        .status(StatusCode::OK)
                                        .header(header::CONTENT_TYPE, "application/json")
                                        .body(Body::from(serde_json::to_string(&response).unwrap()))
                                        .unwrap())
                                }
                            }
                        }
                        Err(e) => {
                            let response = JsonRpcResponse::error(
                                id,
                                ServerError::InvalidParameters(format!("Invalid parameters: {}", e)),
                            );
                            Ok(add_cors_headers(Response::builder())
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, "application/json")
                                .body(Body::from(serde_json::to_string(&response).unwrap()))
                                .unwrap())
                        }
                    }
                }
                "tasks/getPushNotification" => match serde_json::from_value::<TaskIdParams>(params) {
                    Ok(params) => match notification_service.get_push_notification(params).await {
                        Ok(config) => {
                            let response = JsonRpcResponse::success(id, config);
                            Ok(add_cors_headers(Response::builder())
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, "application/json")
                                .body(Body::from(serde_json::to_string(&response).unwrap()))
                                .unwrap())
                        }
                        Err(e) => {
                            let response = JsonRpcResponse::error(id, e);
                            Ok(add_cors_headers(Response::builder())
                                .status(StatusCode::OK)
                                .header(header::CONTENT_TYPE, "application/json")
                                .body(Body::from(serde_json::to_string(&response).unwrap()))
                                .unwrap())
                        }
                    },
                    Err(e) => {
                        let response = JsonRpcResponse::error(
                            id,
                            ServerError::InvalidParameters(format!("Invalid parameters: {}", e)),
                        );
                        Ok(add_cors_headers(Response::builder())
                            .status(StatusCode::OK)
                            .header(header::CONTENT_TYPE, "application/json")
                            .body(Body::from(serde_json::to_string(&response).unwrap()))
                            .unwrap())
                    }
                },
                // Fallback for unrecognized methods
                _ => {
                    let response = JsonRpcResponse::error(
                        id,
                        ServerError::MethodNotFound(
                            format!("Method '{}' not found or not implemented", method).to_string(),
                        ),
                    );
                    Ok(add_cors_headers(Response::builder())
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(serde_json::to_string(&response).unwrap()))
                        .unwrap())
                }
            }
        }
        Err(_) => {
            // Return a 404 or other error for non-JSON-RPC requests
            Ok(add_cors_headers(Response::builder())
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Not Found"))
                .unwrap())
        }
    }
}