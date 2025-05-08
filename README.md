# A2A Test Suite

A comprehensive testing framework for the Agent-to-Agent (A2A) protocol, enabling standardized communication between AI agent systems.

## Overview

The A2A protocol establishes a standard way for AI agents to communicate, supporting:

- Task-based interactions with structured state management (Submit, Get, Cancel, Stream)
- Standard data exchange (text, JSON data)
- Streaming updates via Server-Sent Events
- Authentication using standard HTTP mechanisms (e.g., Bearer tokens)
- Push notifications for asynchronous operations (optional)
- Agent capability discovery via `.well-known/agent.json`

This test suite provides tools to validate implementations against the official A2A protocol specification and ensure compliance.

## Components

- **Validator**: Validate A2A messages against the official JSON schema.
- **Property Tests**: Generate and test random valid A2A messages based on the schema.
- **Mock Server**: A reference A2A server implementation for testing clients.
- **Client Implementation**: A compliant A2A client implementation.
- **Bidirectional Agent**: An agent implementation capable of acting as both an A2A client and server, facilitating testing of agent-to-agent interactions.

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

# Start a registry agent (in registry-only mode)
cargo run -- bidirectional --config agent_registry_config.toml
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
```

## Features

- **Official A2A Protocol Methods**: This test suite focuses *exclusively* on validating the official A2A protocol methods as defined in the specification. The supported methods are:
    - Agent Card Discovery: `GET /.well-known/agent.json`
    - `tasks/send`: Create or update a task.
    - `tasks/get`: Retrieve task status and results.
    - `tasks/cancel`: Cancel an ongoing task.
    - `tasks/sendSubscribe`: Create a task and subscribe to streaming updates via Server-Sent Events (SSE).
    - `tasks/resubscribe`: Reconnect to an existing task's SSE stream.
    - `tasks/pushNotification/set`: Configure a webhook URL for push notifications.
    - `tasks/pushNotification/get`: Retrieve the current push notification configuration.
    **Note:** No other methods or non-standard extensions are implemented or tested by this suite.
- **Authentication**: Supports standard HTTP authentication mechanisms like Bearer tokens.
- **Streaming**: Real-time task updates via Server-Sent Events (SSE) for `tasks/sendSubscribe` and `tasks/resubscribe`.
- **Structured Data**: Exchange JSON data within message parts according to the schema.
- **Push Notifications**: Basic support for configuring and retrieving push notification settings (`tasks/pushNotification/set`, `tasks/pushNotification/get`).
- **Bidirectional Operation**: Includes an agent that can act as both client and server, enabling testing of peer-to-peer agent interactions using the official methods.
- **Comprehensive Testing**: Includes schema validation, property-based testing, and integration tests specifically for the official protocol features listed above.

## Bidirectional Agent Architecture

The bidirectional agent implementation demonstrates how an agent can function as both a client and a server within the A2A protocol. Key aspects include:

1.  **Agent Discovery**: Discovering other agents via their `agent.json` card and caching their information.
2.  **Client Role**: Acting as a client to send tasks to other agents.
3.  **Server Role**: Acting as a server to receive and process tasks from other agents.
4.  **Delegated Task Management**: Basic mechanisms for tracking tasks delegated to other agents (associating local task IDs with remote ones).
5.  **Agent Registry**: A specialized bidirectional agent that can operate in registry-only mode to maintain a directory of other agents, storing their URLs and agent cards.

## Development

```bash
# Run all tests
cargo test

# Build with warnings suppressed
RUSTFLAGS="-A warnings" cargo build

# Run specific tests
cargo test client::streaming::tests
```

## HTTPS Support

The A2A Test Suite now includes HTTPS support using TLS certificates from Certbot. 
See [README_HTTPS.md](README_HTTPS.md) for complete setup instructions.

## Documentation

- [Client Documentation](src/client/README.md)
- [Bidirectional Agent Documentation](src/bidirectional/README.md)
- [Schema Overview](docs/schema_overview.md)

## LLM API Key Configuration

To configure the bidirectional agents with either Claude or Gemini API keys, add the following section to your agent TOML configuration files:

```toml
[llm]
claude_api_key = "sk-ant-api03-your-claude-key"  # Your Claude API key
gemini_api_key = "your-gemini-key"               # Your Gemini API key
system_prompt = "Your system prompt here"
gemini_model_id = "gemini-2.5-pro-preview-05-06"  # Optional, has default
gemini_api_endpoint = "https://generativelanguage.googleapis.com/v1beta/models"  # Optional, has default
```

Notes:
- If both API keys are provided, Claude will be used by default
- To use Gemini specifically, omit the Claude API key
- You can also set API keys via environment variables: `CLAUDE_API_KEY` and `GEMINI_API_KEY`

For more detailed configuration options, see [LLM Configuration](docs/llm_configuration.md).

## Project Status

This project provides a test suite focused on the standard A2A protocol. The core client, server, validator, and property testing components implement the official specification. The bidirectional agent demonstrates how to combine client and server roles for agent-to-agent communication, including agent discovery and basic management of delegated tasks. Non-standard extensions (like file handling, batching, skills) have been removed to maintain focus on protocol compliance testing.
