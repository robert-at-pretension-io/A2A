pub mod config;
pub mod llm_client;
pub mod task_router; // <-- Add this line
pub mod bidirectional_agent;
#[cfg(test)]
pub mod tests;

// Re-export key components
pub use self::config::BidirectionalAgentConfig;
pub use self::llm_client::{LlmClient, ClaudeLlmClient};
pub use self::task_router::BidirectionalTaskRouter; // <-- Add this line
pub use self::bidirectional_agent::BidirectionalAgent;
