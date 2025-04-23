use std::fmt;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerError {
    TaskNotFound(String),
    TaskNotCancelable(String),
    InvalidRequest(String),
    MethodNotFound(String),
    InvalidParameters(String),
    Internal(String),
    PushNotificationNotSupported(String),
    AuthenticationError(String),
    FileNotFound(String),
    SkillNotFound(String),
    BatchNotFound(String),
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ServerError::TaskNotFound(id) => write!(f, "Task not found: {}", id),
            ServerError::TaskNotCancelable(msg) => write!(f, "Task not cancelable: {}", msg),
            ServerError::InvalidRequest(msg) => write!(f, "Invalid request: {}", msg),
            ServerError::MethodNotFound(method) => write!(f, "Method not found: {}", method),
            ServerError::InvalidParameters(msg) => write!(f, "Invalid parameters: {}", msg),
            ServerError::Internal(msg) => write!(f, "Internal server error: {}", msg),
            ServerError::PushNotificationNotSupported(msg) => write!(f, "Push notifications not supported: {}", msg),
            ServerError::AuthenticationError(msg) => write!(f, "Authentication error: {}", msg),
            ServerError::FileNotFound(id) => write!(f, "File not found: {}", id),
            ServerError::SkillNotFound(id) => write!(f, "Skill not found: {}", id),
            ServerError::BatchNotFound(id) => write!(f, "Batch not found: {}", id),
        }
    }
}

impl ServerError {
    pub fn code(&self) -> i32 {
        match self {
            // Update error codes to match the client/errors.rs definitions
            ServerError::TaskNotFound(_) => -32001, // Match ERROR_TASK_NOT_FOUND
            ServerError::TaskNotCancelable(_) => -32002, // Match ERROR_TASK_NOT_CANCELABLE
            ServerError::InvalidRequest(_) => -32600, // Match ERROR_INVALID_REQUEST
            ServerError::MethodNotFound(_) => -32601, // Match ERROR_METHOD_NOT_FOUND
            ServerError::InvalidParameters(_) => -32602, // Match ERROR_INVALID_PARAMS
            ServerError::Internal(_) => -32603, // Match ERROR_INTERNAL
            ServerError::PushNotificationNotSupported(_) => -32003, // Match ERROR_PUSH_NOT_SUPPORTED
            ServerError::AuthenticationError(_) => -32004, // Match ERROR_UNSUPPORTED_OP (closest match)
            ServerError::FileNotFound(_) => -32005, // Match ERROR_INCOMPATIBLE_TYPES (closest match)
            ServerError::SkillNotFound(_) => -32001, // Use ERROR_TASK_NOT_FOUND (for compatibility)
            ServerError::BatchNotFound(_) => -32001, // Use ERROR_TASK_NOT_FOUND (for compatibility)
        }
    }
}

impl std::error::Error for ServerError {}