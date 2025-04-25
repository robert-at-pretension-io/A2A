//! Error types for the Bidirectional Agent module.

#![cfg(feature = "bidir-core")]

use thiserror::Error;
use crate::client::errors::ClientError; // Reuse client errors where applicable

#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Configuration Error: {0}")]
    ConfigError(String),

    #[error("Agent Registry Error: {0}")]
    RegistryError(String),

    #[error("Client Management Error: {0}")]
    ClientManagerError(String),

    #[error("Task Routing Error: {0}")]
    RoutingError(String),

    #[error("Tool Execution Error: {0}")]
    ToolError(String),

    #[error("Task Flow Error: {0}")]
    TaskFlowError(String),

    #[error("Result Synthesis Error: {0}")]
    SynthesisError(String),

    #[error("Delegation Error: {0}")]
    DelegationError(String),

    #[error("A2A Client Error: {0}")]
    A2aClientError(#[from] ClientError),

    #[error("I/O Error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization/Deserialization Error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("TOML Deserialization Error: {0}")]
    TomlError(#[from] toml::de::Error),

    #[error("Reqwest HTTP Error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Anyhow Error: {0}")]
    Anyhow(#[from] anyhow::Error),

    #[error("Other Error: {0}")]
    Other(String),
}

// Implement conversion from String to allow easy error creation
impl From<String> for AgentError {
    fn from(s: String) -> Self {
        AgentError::Other(s)
    }
}
