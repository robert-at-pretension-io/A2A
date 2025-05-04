pub mod config;
pub mod llm_client;
pub mod task_router;
pub mod repl;
pub mod agent_helpers; // <-- Add the new helpers module
pub mod bidirectional_agent;
#[cfg(test)]
pub mod tests;

// Re-export key components
pub use self::config::BidirectionalAgentConfig;
pub use self::llm_client::{LlmClient, ClaudeLlmClient};
pub use self::task_router::{BidirectionalTaskRouter, ExecutionMode};
pub use self::bidirectional_agent::BidirectionalAgent;
