# A2A Test Suite

A comprehensive testing framework for the Agent-to-Agent (A2A) protocol, enabling standardized communication between AI agent systems.

## Overview

The A2A protocol establishes a standard way for AI agents to communicate, supporting:

- Task-based interactions with structured state management
- Rich media exchange (text, data, files)
- Streaming updates via Server-Sent Events
- Authentication using standard HTTP mechanisms
- Push notifications for asynchronous operations
- Agent capability discovery and skill invocation

This test suite provides tools to validate implementations and ensure protocol compliance.

## Components

- **Validator**: Validate A2A messages against the JSON schema
- **Property Tests**: Generate and test random A2A messages
- **Mock Server**: A reference implementation for testing clients
- **Client Implementation**: A complete A2A client
- **Fuzzer**: Test robustness against malformed inputs
- **Bidirectional Agent**: An agent that can act as both client and server

## Usage

### Basic Commands

```bash
# Generate schema types
cargo run -- config generate-types

# Validate an A2A message
cargo run -- validate --file example.json

# Start the mock server
cargo run -- server --port 8080

# Run comprehensive tests
cargo run -- run-tests
```

### Client Operations

```bash
# Get agent capabilities
cargo run -- client get-agent-card --url "http://localhost:8080"

# Send a task
cargo run -- client send-task --url "http://localhost:8080" --message "Hello, agent!"

# Get task status
cargo run -- client get-task --url "http://localhost:8080" --id "task-123"

# Stream task updates
cargo run -- client stream-task --url "http://localhost:8080" --message "Stream updates"

# Send a task with a file
cargo run -- client send-task-with-file --url "http://localhost:8080" --message "Process this" --file-path "data.csv"
```

## Features

- **Complete Protocol Support**: Implements all required and optional A2A endpoints
- **Authentication**: Bearer tokens, API keys, and other standard mechanisms
- **Streaming**: Real-time updates via Server-Sent Events
- **File Handling**: Upload, download, and process files
- **Structured Data**: Exchange JSON data alongside text
- **Push Notifications**: Configure webhooks for asynchronous updates
- **Batching**: Group related tasks together
- **Agent Skills**: Discover and invoke agent-specific capabilities
- **Bidirectional Operation**: Support for agents that both consume and provide A2A services
- **Comprehensive Testing**: Validation, property tests, fuzzing, and integration tests

## Bidirectional Agent Architecture

The bidirectional agent implementation supports:

1. **Agent Discovery**: Automatic discovery and caching of agent capabilities
2. **Local & Remote Execution**: Intelligent routing of tasks to local tools or remote agents
3. **Task Delegation**: Offloading tasks to specialized remote agents
4. **Result Synthesis**: Combining results from multiple sub-tasks
5. **Polling & Monitoring**: Tracking delegated task status

## Development

```bash
# Run all tests
cargo test

# Build with warnings suppressed
RUSTFLAGS="-A warnings" cargo build

# Run specific tests
cargo test client::streaming::tests
```

## Documentation

- [Client Documentation](src/client/README.md)
- [Schema Overview](docs/schema_overview.md)
- [Docker Guide](docker-guide.md)
- [Cross-Compilation](cross-compile.md)

## Project Status

This project is under active development. The core A2A protocol implementation is complete, with full client features and mock server functionality. The bidirectional agent capabilities are being progressively implemented, with current support for agent discovery, client management, and delegated task monitoring.