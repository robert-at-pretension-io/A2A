//! Error types for the Bidirectional Agent module aligned with A2A standard error codes.

use crate::client::errors::{A2aError, ClientError, error_codes};
use crate::server::error::ServerError;
use serde_json::Value;
use thiserror::Error;

/// Standard error type for the bidirectional agent system
#[derive(Error, Debug)]
pub enum AgentError {
    /// Task not found (A2A error code -32001)
    #[error("Task not found: {0}")]
    TaskNotFound(String),

    /// Task not cancelable (A2A error code -32002)
    #[error("Task not cancelable: {0}")]
    TaskNotCancelable(String),

    /// Push notification not supported (A2A error code -32003)
    #[error("Push notification not supported: {0}")]
    PushNotificationNotSupported(String),

    /// Unsupported operation (A2A error code -32004)
    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    /// Incompatible content types (A2A error code -32005)
    #[error("Incompatible content types: {0}")]
    IncompatibleContentTypes(String),

    /// Invalid request (JSON-RPC error code -32600)
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Method not found (JSON-RPC error code -32601)
    #[error("Method not found: {0}")]
    MethodNotFound(String),

    /// Invalid parameters (JSON-RPC error code -32602)
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    /// Internal error (JSON-RPC error code -32603)
    #[error("Internal error: {0}")]
    Internal(String),

    /// Parse error (JSON-RPC error code -32700)
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Configuration error (maps to Internal error)
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Agent registry error (maps to Internal error)
    #[error("Agent registry error: {0}")]
    RegistryError(String),

    /// Client manager error (maps to Internal error)
    #[error("Client manager error: {0}")]
    ClientManagerError(String),

    /// Task routing error (maps to Internal error)
    #[error("Task routing error: {0}")]
    RoutingError(String),

    /// Tool execution error (maps to Internal error)
    #[error("Tool execution error: {0}")]
    ToolError(String),

    /// Task flow error (maps to Internal error)
    #[error("Task flow error: {0}")]
    TaskFlowError(String),

    /// Result synthesis error (maps to Internal error)
    #[error("Result synthesis error: {0}")]
    SynthesisError(String),

    /// Delegation error (maps to Internal error)
    #[error("Delegation error: {0}")]
    DelegationError(String),

    /// Serialization error (maps to Internal error)
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Deserialization error (maps to Parse error)
    #[error("Deserialization error: {0}")]
    DeserializationError(String),

    /// A2A Client error (wrap client errors)
    #[error("A2A client error: {0}")]
    A2aClientError(#[from] ClientError),

    /// I/O Error (maps to Internal error)
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serde JSON Error (maps to Parse error)
    #[error("JSON serialization error: {0}")]
    SerdeError(#[from] serde_json::Error),

    /// TOML Error (maps to Parse error)
    #[error("TOML deserialization error: {0}")]
    TomlError(#[from] toml::de::Error),

    /// Reqwest HTTP Error (maps to Internal error)
    #[error("HTTP error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    /// Anyhow Error (maps to Internal error)
    #[error("Anyhow error: {0}")]
    Anyhow(#[from] anyhow::Error),

    /// Server Error (direct mapping)
    #[error("Server error: {0}")]
    ServerError(#[from] ServerError),

    /// Other Error (maps to Internal error)
    #[error("Other error: {0}")]
    Other(String),
    
    /// Agent not found in registry
    #[error("Agent not found: {0}")]
    AgentNotFound(String),
    
    /// Invalid state for operation
    #[error("Invalid state: {0}")]
    InvalidState(String),
    
    /// Content type error
    #[error("Content type error: {0}")]
    ContentTypeError(String),
    
    /// Internal error with details
    #[error("Internal error: {0}")]
    InternalError(String),
}

impl AgentError {
    /// Convert the error to a JSON-RPC error code
    pub fn code(&self) -> i64 {
        match self {
            AgentError::TaskNotFound(_) => error_codes::ERROR_TASK_NOT_FOUND,
            AgentError::TaskNotCancelable(_) => error_codes::ERROR_TASK_NOT_CANCELABLE,
            AgentError::PushNotificationNotSupported(_) => error_codes::ERROR_PUSH_NOT_SUPPORTED,
            AgentError::UnsupportedOperation(_) => error_codes::ERROR_UNSUPPORTED_OP,
            AgentError::IncompatibleContentTypes(_) => error_codes::ERROR_INCOMPATIBLE_TYPES,
            AgentError::InvalidRequest(_) => error_codes::ERROR_INVALID_REQUEST,
            AgentError::MethodNotFound(_) => error_codes::ERROR_METHOD_NOT_FOUND,
            AgentError::InvalidParameters(_) => error_codes::ERROR_INVALID_PARAMS,
            AgentError::ParseError(_) | AgentError::DeserializationError(_) | 
            AgentError::SerdeError(_) | AgentError::TomlError(_) => error_codes::ERROR_PARSE,
            AgentError::A2aClientError(client_err) => match client_err {
                ClientError::A2aError(a2a_err) => a2a_err.code,
                _ => error_codes::ERROR_INTERNAL,
            },
            AgentError::ServerError(server_err) => server_err.code() as i64,
            // All other errors map to internal error
            _ => error_codes::ERROR_INTERNAL,
        }
    }

    /// Create an A2A error from this error
    pub fn to_a2a_error(&self) -> A2aError {
        let code = self.code();
        let message = self.to_string();
        let data = Some(Value::String(format!("{:?}", self)));
        A2aError::new(code, &message, data)
    }

    /// Convert to a client error
    pub fn to_client_error(&self) -> ClientError {
        match self {
            AgentError::A2aClientError(err) => err.clone(),
            _ => ClientError::A2aError(self.to_a2a_error()),
        }
    }
}

// Implement conversion from String to allow easy error creation
impl From<String> for AgentError {
    fn from(s: String) -> Self {
        AgentError::Other(s)
    }
}

// Implement conversion from &str for convenience
impl From<&str> for AgentError {
    fn from(s: &str) -> Self {
        AgentError::Other(s.to_string())
    }
}

// Implement conversion to ServerError
impl From<AgentError> for ServerError {
    fn from(err: AgentError) -> Self {
        match err {
            AgentError::TaskNotFound(msg) => ServerError::TaskNotFound(msg),
            AgentError::TaskNotCancelable(msg) => ServerError::TaskNotCancelable(msg),
            AgentError::PushNotificationNotSupported(msg) => ServerError::PushNotificationNotSupported(msg),
            AgentError::InvalidRequest(msg) => ServerError::InvalidRequest(msg),
            AgentError::MethodNotFound(msg) => ServerError::MethodNotFound(msg),
            AgentError::InvalidParameters(msg) => ServerError::InvalidParameters(msg),
            AgentError::ServerError(server_err) => server_err,
            // All other errors map to Internal
            _ => ServerError::Internal(err.to_string()),
        }
    }
}

/// Create a common error handler function to map errors to A2A protocol errors
pub fn map_error(error: impl Into<AgentError>) -> ClientError {
    let agent_error: AgentError = error.into();
    agent_error.to_client_error()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        // Test A2A-specific error codes
        assert_eq!(AgentError::TaskNotFound("test".to_string()).code(), error_codes::ERROR_TASK_NOT_FOUND);
        assert_eq!(AgentError::TaskNotCancelable("test".to_string()).code(), error_codes::ERROR_TASK_NOT_CANCELABLE);
        assert_eq!(AgentError::PushNotificationNotSupported("test".to_string()).code(), error_codes::ERROR_PUSH_NOT_SUPPORTED);
        assert_eq!(AgentError::UnsupportedOperation("test".to_string()).code(), error_codes::ERROR_UNSUPPORTED_OP);
        assert_eq!(AgentError::IncompatibleContentTypes("test".to_string()).code(), error_codes::ERROR_INCOMPATIBLE_TYPES);

        // Test JSON-RPC standard error codes
        assert_eq!(AgentError::InvalidRequest("test".to_string()).code(), error_codes::ERROR_INVALID_REQUEST);
        assert_eq!(AgentError::MethodNotFound("test".to_string()).code(), error_codes::ERROR_METHOD_NOT_FOUND);
        assert_eq!(AgentError::InvalidParameters("test".to_string()).code(), error_codes::ERROR_INVALID_PARAMS);
        assert_eq!(AgentError::Internal("test".to_string()).code(), error_codes::ERROR_INTERNAL);
        assert_eq!(AgentError::ParseError("test".to_string()).code(), error_codes::ERROR_PARSE);

        // Test internal error mappings
        assert_eq!(AgentError::ConfigError("test".to_string()).code(), error_codes::ERROR_INTERNAL);
        assert_eq!(AgentError::RegistryError("test".to_string()).code(), error_codes::ERROR_INTERNAL);
        assert_eq!(AgentError::ToolError("test".to_string()).code(), error_codes::ERROR_INTERNAL);
        assert_eq!(AgentError::Other("test".to_string()).code(), error_codes::ERROR_INTERNAL);
    }

    #[test]
    fn test_conversion_to_a2a_error() {
        let agent_error = AgentError::TaskNotFound("task-123".to_string());
        let a2a_error = agent_error.to_a2a_error();

        assert_eq!(a2a_error.code, error_codes::ERROR_TASK_NOT_FOUND);
        assert!(a2a_error.message.contains("task-123"));
    }

    #[test]
    fn test_map_error_function() {
        let string_error = "Test error";
        let client_error = map_error(string_error);

        match client_error {
            ClientError::A2aError(a2a_error) => {
                assert_eq!(a2a_error.code, error_codes::ERROR_INTERNAL);
                assert!(a2a_error.message.contains(string_error));
            },
            _ => panic!("Expected A2aError variant"),
        }
    }

    #[test]
    fn test_conversion_to_server_error() {
        let task_not_found = AgentError::TaskNotFound("task-123".to_string());
        match ServerError::from(task_not_found) {
            ServerError::TaskNotFound(msg) => assert_eq!(msg, "task-123"),
            _ => panic!("Expected TaskNotFound variant"),
        }

        let internal_error = AgentError::ToolError("tool failed".to_string());
        match ServerError::from(internal_error) {
            ServerError::Internal(msg) => assert!(msg.contains("tool failed")),
            _ => panic!("Expected Internal variant"),
        }
    }
}