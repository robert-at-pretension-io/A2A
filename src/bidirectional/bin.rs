// Binary entrypoint for the bidirectional agent

// Import from the parent crate
extern crate a2a_test_suite;

// Use the bidirectional agent module
use a2a_test_suite::bidirectional::bidirectional_agent;

fn main() -> anyhow::Result<()> {
    // Call into the agent's main function
    bidirectional_agent::main()
}
