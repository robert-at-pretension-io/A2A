// Test modules for bidirectional agent
mod agent_tests;
mod agent_card_tests; // New test module for comprehensive agent card testing
mod toml_agent_card_test; // New test module for TOML configuration of agent cards
mod artifact_tests;
mod config_tests;
mod input_required_remote_tests; // New test module for InputRequired remote task handling
mod input_required_tests;
mod memory_isolation_tests; // New test module for rolling memory security and isolation tests
mod memory_tests; // New test module for rolling memory feature
mod mocks;
mod repl_commands_tests;
mod repl_tests;
mod router_tests;
mod session_tests;
mod task_rejection_tests;
mod task_service_tests;
pub mod server_static_files_test;
