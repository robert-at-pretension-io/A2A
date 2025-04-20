use std::error::Error;
use std::fmt;

/// Represents an error returned by the A2A API
#[derive(Debug)]
pub struct A2aError {
    pub code: i64,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl A2aError {
    pub fn new(code: i64, message: &str, data: Option<serde_json::Value>) -> Self {
        Self {
            code,
            message: message.to_string(),
            data,
        }
    }
    
    /// Check if this is a task not found error
    pub fn is_task_not_found(&self) -> bool {
        self.code == error_codes::ERROR_TASK_NOT_FOUND
    }
    
    /// Check if this is a task not cancelable error
    pub fn is_task_not_cancelable(&self) -> bool {
        self.code == error_codes::ERROR_TASK_NOT_CANCELABLE
    }
    
    /// Check if this is a push notification not supported error
    pub fn is_push_not_supported(&self) -> bool {
        self.code == error_codes::ERROR_PUSH_NOT_SUPPORTED
    }
    
    /// Check if this is an unsupported operation error
    pub fn is_unsupported_operation(&self) -> bool {
        self.code == error_codes::ERROR_UNSUPPORTED_OP
    }
    
    /// Check if this is an incompatible content types error
    pub fn is_incompatible_types(&self) -> bool {
        self.code == error_codes::ERROR_INCOMPATIBLE_TYPES
    }
    
    /// Check if this is an invalid request error
    pub fn is_invalid_request(&self) -> bool {
        self.code == error_codes::ERROR_INVALID_REQUEST
    }
    
    /// Check if this is a method not found error
    pub fn is_method_not_found(&self) -> bool {
        self.code == error_codes::ERROR_METHOD_NOT_FOUND
    }
    
    /// Check if this is an invalid parameters error
    pub fn is_invalid_params(&self) -> bool {
        self.code == error_codes::ERROR_INVALID_PARAMS
    }
    
    /// Check if this is an internal server error
    pub fn is_internal_error(&self) -> bool {
        self.code == error_codes::ERROR_INTERNAL
    }
    
    /// Check if this is a parse error
    pub fn is_parse_error(&self) -> bool {
        self.code == error_codes::ERROR_PARSE
    }
}

impl fmt::Display for A2aError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON-RPC error: {} (code: {})", self.message, self.code)
    }
}

impl Error for A2aError {}

/// Represents all possible error types the A2A client might encounter
#[derive(Debug)]
pub enum ClientError {
    /// JSON-RPC error from the A2A server
    A2aError(A2aError),
    
    /// HTTP error (e.g., connection refused, timeout)
    HttpError(String),
    
    /// JSON serialization/deserialization error
    JsonError(String),
    
    /// File I/O error
    IoError(std::io::Error),
    
    /// Any other error
    Other(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::A2aError(err) => write!(f, "{}", err),
            ClientError::HttpError(msg) => write!(f, "HTTP error: {}", msg),
            ClientError::JsonError(msg) => write!(f, "JSON error: {}", msg),
            ClientError::IoError(err) => write!(f, "I/O error: {}", err),
            ClientError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl Error for ClientError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ClientError::A2aError(err) => Some(err),
            ClientError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<A2aError> for ClientError {
    fn from(err: A2aError) -> Self {
        ClientError::A2aError(err)
    }
}

impl From<reqwest::Error> for ClientError {
    fn from(err: reqwest::Error) -> Self {
        ClientError::HttpError(format!("{}", err))
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(err: serde_json::Error) -> Self {
        ClientError::JsonError(format!("{}", err))
    }
}

impl From<std::io::Error> for ClientError {
    fn from(err: std::io::Error) -> Self {
        ClientError::IoError(err)
    }
}

impl From<String> for ClientError {
    fn from(msg: String) -> Self {
        ClientError::Other(msg)
    }
}

impl From<&str> for ClientError {
    fn from(msg: &str) -> Self {
        ClientError::Other(msg.to_string())
    }
}

/// Re-export the error codes from mock_server for use in error handling
pub mod error_codes {
    // JSON-RPC standard error codes
    pub const ERROR_PARSE: i64 = -32700;             // "Invalid JSON payload"
    pub const ERROR_INVALID_REQUEST: i64 = -32600;   // "Request payload validation error"
    pub const ERROR_METHOD_NOT_FOUND: i64 = -32601;  // "Method not found"
    pub const ERROR_INVALID_PARAMS: i64 = -32602;    // "Invalid parameters"
    pub const ERROR_INTERNAL: i64 = -32603;          // "Internal error"
    
    // A2A-specific error codes
    pub const ERROR_TASK_NOT_FOUND: i64 = -32001;    // "Task not found"
    pub const ERROR_TASK_NOT_CANCELABLE: i64 = -32002; // "Task cannot be canceled"
    pub const ERROR_PUSH_NOT_SUPPORTED: i64 = -32003; // "Push Notification is not supported"
    pub const ERROR_UNSUPPORTED_OP: i64 = -32004;    // "This operation is not supported"
    pub const ERROR_INCOMPATIBLE_TYPES: i64 = -32005; // "Incompatible content types"
}