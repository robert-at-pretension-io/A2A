pub mod agent_helpers;
pub mod agent_registry;
pub mod bidirectional_agent;
pub mod config;
pub mod https_support;
pub mod llm_client;
pub mod registry_router;
pub mod repl;
pub mod task_router;
#[cfg(test)]
pub mod tests;

// Re-export key components
pub use self::agent_registry::AgentDirectory;
pub use self::bidirectional_agent::BidirectionalAgent;
pub use self::config::BidirectionalAgentConfig;
pub use self::llm_client::{ClaudeLlmClient, GeminiLlmClient, LlmClient};
pub use self::registry_router::RegistryRouter;
pub use self::task_router::{BidirectionalTaskRouter, ExecutionMode};
pub use crate::server::task_router::LlmTaskRouterTrait;
