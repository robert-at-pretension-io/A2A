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

## Future Possibilities

* **Agent Marketplaces**: Specialized agent ecosystems where businesses offer their proprietary AI capabilities as services.

* **Agent Orchestration Systems**: Meta-agents that dynamically discover and delegate to the most appropriate specialized agents for each task.

* **Cross-Organization Collaboration**: Secure agent networks that span organizational boundaries for supply chain, customer service, and partnership workflows.

* **Personal Agent Ecosystems**: Individual users with personalized networks of agents that know their preferences and can delegate tasks appropriately.

## Getting Started

This repository provides tools to test A2A protocol implementations, ensuring compatibility across different agent systems. By ensuring your agents conform to the A2A standard, you'll be positioning them to participate in the emerging ecosystem of collaborative AI.

### Test Suite Components

- **Validator**: Validate A2A messages against the JSON schema
- **Property Tests**: Generate and test random A2A messages
- **Mock Server**: A reference implementation for testing clients
- **Client Implementation**: A complete A2A client for interacting with servers
- **Fuzzer**: Test robustness against malformed inputs
- **Integration Tests**: End-to-end testing of client-server interactions

### Client Usage

The A2A client implementation can be used to interact with any A2A-compatible server:

```bash
# Get an agent's card (capabilities, skills, etc.)
cargo run -- client get-agent-card --url "http://localhost:8080"

# Send a task to an agent
cargo run -- client send-task --url "http://localhost:8080" --message "Hello, agent!"

# Retrieve a task's status
cargo run -- client get-task --url "http://localhost:8080" --id "task-123"

# Cancel a task
cargo run -- client cancel-task --url "http://localhost:8080" --id "task-123"
```

#### Advanced Features

Our client supports rich interactions beyond basic text messaging:

* **File Operations**: Send tasks with file attachments either by path or bytes
* **Structured Data**: Transmit JSON data structures alongside text
* **Streaming**: Receive incremental updates via Server-Sent Events
* **Artifacts**: Retrieve, save, and process various artifact types from task results
* **Push Notifications**: Configure webhooks for asynchronous task updates
* **Task Batching**: Create and manage groups of related tasks
* **State History**: Track and analyze task state transitions
* **Agent Skills**: Discover, query, and invoke agent skills

For detailed documentation and examples of these advanced features, see [Client README](src/client/README.md).

Run the full integration test suite:

```bash
./start_server_and_test_client.sh
```

### Comprehensive Testing Strategy

For a detailed approach to testing A2A server implementations, see our [Testing Plan](docs/testing_plan.md), which outlines a comprehensive framework for validating protocol compliance, performance, and security.

---

The A2A protocol represents a crucial step toward mature, enterprise-ready AI that can truly transform businesses and everyday experiences. By standardizing how agents communicate, A2A will unlock collaboration patterns that are currently impossible, moving us from isolated AI capabilities to truly interconnected agent networks.
