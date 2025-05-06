pub mod agent_helpers;
pub mod bidirectional_agent;
pub mod config;
pub mod llm_client;
pub mod repl;
pub mod task_router;
#[cfg(test)]
pub mod tests;

// Re-export key components
pub use self::bidirectional_agent::BidirectionalAgent;
pub use self::config::BidirectionalAgentConfig;
pub use self::llm_client::{ClaudeLlmClient, GeminiLlmClient, LlmClient};
pub use self::task_router::{BidirectionalTaskRouter, ExecutionMode};
pub use crate::server::task_router::LlmTaskRouterTrait;
