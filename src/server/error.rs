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
    // Additional errors for the server components
    A2aClientError(String),
    AgentNotFound(String),
    UnsupportedOperation(String),
    ConfigError(String),
    ServerTaskExecutionFailed(String),
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
            ServerError::A2aClientError(_) => todo!(),
            ServerError::AgentNotFound(_) => todo!(),
            ServerError::UnsupportedOperation(_) => todo!(),
            ServerError::ConfigError(_) => todo!(),
            ServerError::ServerTaskExecutionFailed(_) => todo!(),
        }
    }
}

impl ServerError {
    pub fn code(&self) -> i32 {
        match self {
            ServerError::TaskNotFound(_) => -32001,
            ServerError::TaskNotCancelable(_) => -32002,
            ServerError::InvalidRequest(_) => -32600,
            ServerError::MethodNotFound(_) => -32601,
            ServerError::InvalidParameters(_) => -32602,
            ServerError::Internal(_) => -32603,
            ServerError::PushNotificationNotSupported(_) => -32003,
            ServerError::AuthenticationError(_) => -32004,
            ServerError::FileNotFound(_) => -32005,
            ServerError::SkillNotFound(_) => -32001,
            ServerError::BatchNotFound(_) => -32001,
            ServerError::A2aClientError(_) => todo!(),
ServerError::AgentNotFound(_) => todo!(),
            ServerError::UnsupportedOperation(_) => todo!(),
ServerError::ConfigError(_) => todo!(),
            ServerError::ServerTaskExecutionFailed(_) => todo!(),
                    }
    }
}

impl std::error::Error for ServerError {}