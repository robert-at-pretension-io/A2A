# A2A Test Suite: Building the Future of Agent Collaboration

This repository contains testing tools for the Agent-to-Agent (A2A) protocol, an emerging standard that will fundamentally transform how AI systems work together.

## Why A2A Matters

### For Businesses

* **Connect Best-of-Breed AI Solutions**: Deploy specialized AI agents from different vendors that seamlessly work together rather than being locked into a single provider.
* **Enterprise-Ready from Day One**: Built with security, authentication, and monitoring in mind, making it suitable for production enterprise environments.
* **Long-Running Business Processes**: Support for asynchronous, long-running tasks enables AI to participate in complex business processes that span days or weeks.
* **Clear Cost Management**: The task-based structure provides natural units for billing and cost allocation across complex AI agent networks.

### For Developers

* **Universal Agent Interface**: Build once, connect to any A2A-compliant agent without needing custom integration code for each AI provider.
* **Modular Architecture**: Design specialized, focused agents that excel at specific tasks rather than building monolithic systems that try to do everything.
* **Simplified Authentication**: Leverage existing OAuth, JWT, and other enterprise authentication standards rather than implementing proprietary schemes.
* **Rich Media Support**: Exchange not just text but structured data, files, and multimedia between agents through a standardized protocol.

### For Everyone

* **Task Continuity**: Start a complex task on one device and seamlessly continue it on another, with the agent maintaining context and progress.
* **Specialized Expertise**: Access an ecosystem of specialized AI agents that can collaborate to solve your specific problems.
* **Human-in-the-Loop Workflows**: AI systems that can pause for your input when needed and resume automatically once you provide it.
* **Persistent Relationships**: Build ongoing relationships with AI agents that remember your preferences and past interactions.

## The A2A Advantage: Key Technical Features

1. **Agent Cards**: Standardized capability discovery allows agents to advertise their skills and supported formats.

2. **Opaque Execution**: Agents collaborate without sharing their internal mechanisms, protecting intellectual property and maintaining clear boundaries.

3. **Push Notifications**: Support for disconnected operation with agents that can notify clients when long-running tasks complete.

4. **Task-Based Communication**: A well-defined structure for requests, responses, and multi-turn conversations between agents.

5. **Enterprise Security**: Built on established security standards rather than reinventing authentication and authorization.

## Test Suite Components

- **Validator**: Validate A2A messages against the JSON schema
- **Property Tests**: Generate and test random A2A messages
- **Mock Server**: A reference implementation for testing clients
- **Client Implementation**: A complete A2A client for interacting with servers
- **Fuzzer**: Test robustness against malformed inputs with intelligent A2A structure generation
- **Integration Tests**: End-to-end testing of client-server interactions

## Getting Started

This repository provides tools to test A2A protocol implementations, ensuring compatibility across different agent systems. By ensuring your agents conform to the A2A standard, you'll be positioning them to participate in the emerging ecosystem of collaborative AI.

### Client Usage

The A2A client implementation can be used to interact with any A2A-compatible server:

```bash
# Get an agent's card (capabilities, skills, etc.)
cargo run -- client get-agent-card --url "http://localhost:8080"

# Send a task to an agent (with authentication)
cargo run -- client send-task --url "http://localhost:8080" --message "Hello, agent!" --header "Authorization" --value "Bearer your-token"

# Retrieve a task's status
cargo run -- client get-task --url "http://localhost:8080" --id "task-123" --header "Authorization" --value "Bearer your-token"

# Cancel a task
cargo run -- client cancel-task --url "http://localhost:8080" --id "task-123" --header "Authorization" --value "Bearer your-token"

# Validate authentication with the server
cargo run -- client validate-auth --url "http://localhost:8080" --header "Authorization" --value "Bearer your-token"
```

### Advanced Features

Our client implements the complete A2A protocol with support for rich interactions:

* **Authentication**: Support for HTTP-based authentication using Bearer tokens, API keys, and other OpenAPI-compatible schemes
* **File Operations**: Send tasks with file attachments either by path or bytes
* **Structured Data**: Transmit JSON data structures alongside text
* **Streaming**: Receive incremental updates via Server-Sent Events
* **Artifacts**: Retrieve, save, and process various artifact types from task results
* **Push Notifications**: Configure webhooks for asynchronous task updates
* **Task Batching**: Create and manage groups of related tasks
* **State History**: Track and analyze task state transitions
* **Agent Skills**: Discover, query, and invoke agent skills

For detailed documentation and examples of these advanced features, see [Client README](src/client/README.md).

### Authentication

The A2A protocol handles authentication at the HTTP level following the OpenAPI Authentication specification. Our implementation supports:

* **Bearer Token Authentication**: Using the standard `Authorization` header
* **API Key Authentication**: Using custom headers like `X-API-Key`
* **Agent Card Auth Discovery**: Reading authentication requirements from the agent card
* **Auth Validation**: Methods to validate authentication credentials

Authentication can be applied to any client operation by specifying the appropriate header and value.

```rust
// In Rust code
let mut client = A2aClient::new("https://example.com/a2a")
    .with_auth("Authorization", "Bearer your-token-here");
```

The mock server also supports configurable authentication requirements for testing both authenticated and non-authenticated scenarios.

### Running Integration Tests

Run the full integration test suite (with authentication):

```bash
./start_server_and_test_client.sh
```

## Implemented Features

The test suite now includes:

1. **Core Protocol Implementation**:
   - Complete message validation against A2A JSON schema
   - Property-based testing for message correctness
   - Full mock server implementation with all A2A endpoints

2. **Client Library Features**:
   - Basic task creation, retrieval, and cancellation
   - File attachment handling and binary data operations
   - Structured data support (JSON)
   - Streaming task updates via Server-Sent Events
   - Task artifacts management and processing
   - Push notification configuration and management
   - State transition history and metrics
   - Task batch operations
   - Agent skills discovery and invocation
   - Authentication with multiple schemes

3. **Testing Tools**:
   - End-to-end integration tests
   - Structured fuzzing for robustness testing:
     - Schema validation fuzzing
     - JSON-RPC request fuzzing
     - Message parsing fuzzing
   - Comprehensive test script for all features
   - Mock server with configurable authentication

## Future Possibilities

* **Agent Marketplaces**: Specialized agent ecosystems where businesses offer their proprietary AI capabilities as services.

* **Agent Orchestration Systems**: Meta-agents that dynamically discover and delegate to the most appropriate specialized agents for each task.

* **Cross-Organization Collaboration**: Secure agent networks that span organizational boundaries for supply chain, customer service, and partnership workflows.

* **Personal Agent Ecosystems**: Individual users with personalized networks of agents that know their preferences and can delegate tasks appropriately.

## Comprehensive Testing Strategy

For a detailed approach to testing A2A server implementations, see our [Testing Plan](testing_plan.md), which outlines a comprehensive framework for validating protocol compliance, performance, and security.

---

The A2A protocol represents a crucial step toward mature, enterprise-ready AI that can truly transform businesses and everyday experiences. By standardizing how agents communicate, A2A will unlock collaboration patterns that are currently impossible, moving us from isolated AI capabilities to truly interconnected agent networks.