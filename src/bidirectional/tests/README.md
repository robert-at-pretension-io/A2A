# Bidirectional Agent Tests

This directory contains tests for the bidirectional A2A agent implementation. The tests are organized into several modules, each focusing on a specific aspect of the agent's functionality.

## Test Modules

### 1. Configuration Tests (`config_tests.rs`)

Tests for the configuration handling in the bidirectional agent, including:
- Default configuration
- Loading from TOML files
- Handling partial configuration
- Error handling for invalid configurations

### 2. Router Tests (`router_tests.rs`)

Tests for the task routing logic, including:
- Local vs. remote routing decisions
- Handling unknown agents
- Fallback behavior for unclear decisions
- Prompt formatting

### 3. Agent Tests (`agent_tests.rs`)

Tests for the agent functionality, including:
- Agent card creation
- Configuration validation
- Client initialization

### 4. REPL Tests (`repl_tests.rs`)

Tests for the interactive REPL functionality, focusing on the command parsing logic:
- Parsing various REPL commands
- Server management commands
- Connection commands

## Mock Implementations (`mocks.rs`)

Contains mock implementations used across the test modules:
- `MockLlmClient`: A mock implementation of the LLM client interface
- `MockA2aClient`: A mock implementation of the A2A client

## Running the Tests

To run all the bidirectional agent tests:

```bash
cargo test bidirectional::tests
```

To run tests in a specific module:

```bash
cargo test bidirectional::tests::config_tests
```

## Future Test Improvements

The current tests focus on the components that are easy to test in isolation. Future improvements could include:

1. More robust testing of the REPL functionality by mocking stdin/stdout
2. Integration tests for the bidirectional agent with mock servers
3. Tests for the process_message_directly method with various message types
4. Tests for remote agent interaction with various error scenarios
5. Tests for server functionality with mock clients

The tests would benefit from further refactoring of the agent implementation to improve testability through dependency injection and better separation of concerns.