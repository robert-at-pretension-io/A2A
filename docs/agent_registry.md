# Agent Registry for A2A Protocol

## Introduction

The Agent Registry is a critical component of the Agent-to-Agent (A2A) ecosystem that enables agent discovery, cataloging, and interoperability. It functions as a centralized or distributed directory of available A2A agents, their capabilities, and connection information. This document provides an in-depth explanation of the Agent Registry implementation, its architecture, features, and the crucial role it plays in facilitating agent communication in the A2A protocol.

## Why an Agent Registry is Essential

### The Problem: Agent Discovery and Interoperability

In a distributed system of AI agents, several challenges emerge:

1. **Discovery Challenge**: How do agents find other agents that can provide specific services?
2. **Capability Awareness**: How does an agent know what another agent can do?
3. **Protocol Compatibility**: How can agents ensure they're communicating using compatible protocols?
4. **Network Topology**: How can we maintain a dynamic map of the growing agent ecosystem?
5. **Trust and Verification**: How can agents establish trust with newly discovered agents?

### The Solution: A Robust Agent Registry

The Agent Registry addresses these challenges by providing:

1. **Centralized Directory**: A single source of truth for agent discovery
2. **Capability Cataloging**: Detailed records of what each agent can do
3. **Dynamic Updates**: Real-time discovery and updates as new agents come online
4. **Metadata Storage**: Storing relevant information about each agent
5. **URL Normalization**: Consistent handling of agent endpoints
6. **Historical Tracking**: Recording when agents were last contacted

## Core Features

### 1. Agent Discovery

The registry can automatically extract and validate agent URLs from messages. It uses sophisticated pattern matching to identify:

- Standard URLs with protocols (http://, https://)
- Domain names with ports
- Localhost references
- IP addresses with optional ports

Example:
```
"Connect to the agent at http://example.com:8080 and also check out the analytics agent at data.ai-agents.com:9000"
```

From this message, the registry will automatically extract both URLs and attempt to register them.

### 2. Agent Information Storage

For each discovered agent, the registry stores:

- **URL**: Normalized agent endpoint
- **Name**: Agent's self-identified name
- **Last Contacted**: Timestamp of most recent interaction
- **Agent Card**: Complete agent capabilities card (if available)
- **Metadata**: Additional custom information about the agent

### 3. AgentCard Integration

The registry fetches and stores comprehensive AgentCard information from each discovered agent. This includes:

- **Basic Information**: Name, version, description
- **Capabilities**: Streaming support, push notifications, state history tracking
- **Skills**: Available agent skills and their descriptions
- **API Parameters**: Expected input/output formats
- **Authentication Requirements**: Required authentication methods

### 4. URL Normalization

To ensure consistency, the registry normalizes all URLs:

- Adding protocol if missing (defaulting to http://)
- Validating URL format
- Removing trailing slashes
- Ensuring consistent port representation

### 5. Persistent Storage

The registry saves all agent information to a JSON file, allowing:

- Persistence across restarts
- Sharing agent information between systems
- Backup and recovery
- Offline analysis of agent networks

### 6. Registry-Only Mode

The system supports a "registry-only" mode that operates without requiring an LLM. This allows for:

- Lower operational costs (no API calls to language models)
- Faster response times
- Simpler deployment in resource-constrained environments
- Focus on the core agent discovery functionality

## Architecture

### AgentDirectory

The core storage component that manages agent information:

```rust
pub struct AgentDirectory {
    agents: Arc<DashMap<String, AgentDirectoryEntry>>,
    directory_path: Option<String>,
}
```

### AgentDirectoryEntry

Each registered agent is stored as an AgentDirectoryEntry:

```rust
pub struct AgentDirectoryEntry {
    pub url: String,
    pub name: Option<String>,
    pub last_contacted: Option<String>,
    pub agent_card: Option<AgentCard>,
    pub metadata: Option<serde_json::Map<String, serde_json::Value>>,
}
```

### RegistryRouter

A specialized router that handles URL extraction and agent registration:

```rust
pub struct RegistryRouter {
    registry: Arc<AgentDirectory>,
}
```

This router implements the `LlmTaskRouterTrait` interface but operates without requiring an LLM for its core functionality.

## Workflow

### 1. Agent Registration Process

When a message containing potential agent URLs is received:

1. The `RegistryRouter` extracts all potential URLs using regex patterns
2. Each URL is normalized for consistency
3. The URL is added to the registry with a timestamp
4. The router attempts to fetch the agent's AgentCard
5. If successful, the card information is stored with the agent entry
6. The registry is saved to persistent storage
7. A response is generated detailing the registration results

### 2. Agent Lookup Process

When a non-URL message is received:

1. The router recognizes it's not a registration request
2. The registry compiles a list of all known agents
3. Agents with full card information are grouped separately
4. Detailed capabilities information is formatted for display
5. A comprehensive response is generated showing all registered agents

### 3. Agent Card Fetching

For each discovered agent URL:

1. A temporary A2A client is created for the URL
2. The client attempts to fetch the agent's card via the A2A protocol
3. If successful, the card is stored with the agent's entry
4. The agent's name is updated based on the card information
5. Last contacted timestamp is updated

## Integration with A2A Ecosystem

### Bidirectional Agent Support

The registry integrates seamlessly with the Bidirectional Agent architecture:

1. **Configuration**: Simple TOML configuration for registry setup
2. **Startup Modes**: Supports both registry-only and full LLM modes
3. **REPL Integration**: Works with the interactive REPL interface
4. **Agent Helpers**: Utilizes the same helper functions as other agent types

### A2A Protocol Compatibility

The registry is fully compatible with the A2A protocol:

1. Uses standard A2A client for agent card fetching
2. Respects agent capabilities as defined in the protocol
3. Maintains connection information in protocol-compatible format
4. Properly handles task processing as defined in the protocol

## Use Cases

### 1. Agent Network Mapping

The registry can be used to map out the topology of an A2A agent network:

```
Connected to agent registry at http://localhost:8085...

> http://finance-agent.example.com:8080 http://weather-agent.example.com:8081

✅ Registered 2 agent URLs: http://finance-agent.example.com:8080, http://weather-agent.example.com:8081

🔍 Retrieved agent information:
- Finance Agent: http://finance-agent.example.com:8080
- Weather API Agent: http://weather-agent.example.com:8081

> list agents

📋 Known Agents (2)

🔍 Agents with full information:
1. Finance Agent (http://finance-agent.example.com:8080)
   Description: Provides financial data and analysis
   Capabilities: streaming, push_notifications
   Skills: 5
   Last contacted: 2025-05-07T10:15:30Z

2. Weather API Agent (http://weather-agent.example.com:8081)
   Description: Provides weather forecasts and historical data
   Capabilities: streaming
   Skills: 3
   Last contacted: 2025-05-07T10:15:31Z
```

### 2. Service Discovery

Applications can use the registry to discover appropriate agents for specific tasks:

```
> find agent for weather forecast

Based on registered agents, the Weather API Agent (http://weather-agent.example.com:8081) 
can provide weather forecasts. It supports streaming responses and has 3 registered skills.
```

### 3. Agent Network Health Monitoring

The registry can be used to monitor the health and availability of agents:

```
> check agent status

Checking all registered agents...

✅ Finance Agent (http://finance-agent.example.com:8080): Online
❌ Weather API Agent (http://weather-agent.example.com:8081): Offline (Last seen: 3 days ago)
```

### 4. Agent Capability Discovery

Developers can use the registry to explore available agent capabilities:

```
> show agent capabilities http://finance-agent.example.com:8080

Finance Agent Capabilities:
- Streaming: Yes
- Push Notifications: Yes
- State Transition History: No

Available Skills:
1. stock_quote - Get current stock price information
2. company_financials - Retrieve company financial statements
3. market_analysis - Analyze market trends and provide insights
4. portfolio_optimization - Optimize investment portfolio allocation
5. risk_assessment - Assess investment risk profiles
```

## Configuration

The Agent Registry can be configured through a TOML file:

```toml
# Agent Registry Configuration

# Server configuration
[server]
port = 8085
bind_address = "0.0.0.0"
agent_id = "agent-registry"
agent_name = "Agent Registry"

# Registry configuration
[registry]
# Enable registry-only mode (no LLM needed)
registry_only_mode = true
# Path to store registry data
registry_path = "data/agent_registry.json"

# Mode configuration
[mode]
# Enable REPL by default
repl = true
# Auto-start server
auto_listen = true
# Log file for REPL interactions
repl_log_file = "logs/agent_registry.log"
```

## Implementation Details

### URL Extraction Logic

The registry uses multiple regex patterns to extract potential agent URLs:

```rust
let url_patterns = [
    // Standard URL with protocol
    r"https?://(?:[-\w.]|(?:%[\da-fA-F]{2}))+(?::\d+)?(?:/[-\w%!$&'()*+,;=:@/~]*)?",
    // Host:port format without protocol
    r"\b(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}(?::\d+)?\b",
    // Localhost with port
    r"localhost:\d+",
    // IP address with optional port
    r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}(?::\d+)?\b",
];
```

### Agent Card Fetching

When a URL is discovered, the registry attempts to fetch its agent card:

```rust
async fn fetch_agent_card(&self, url: &str) -> Result<Option<AgentCard>, ServerError> {
    let client = A2aClient::new(url);
    
    match client.get_agent_card().await {
        Ok(card) => {
            self.registry.update_agent_card(url, card.clone())?;
            Ok(Some(card))
        }
        Err(e) => {
            debug!(url = %url, error = %e, "Failed to fetch agent card");
            Ok(None)
        }
    }
}
```

### URL Normalization

To ensure consistency, URLs are normalized:

```rust
pub fn normalize_url(url: &str) -> Result<String> {
    // If URL doesn't start with http(s), add http://
    let url_with_scheme = if !url.starts_with("http://") && !url.starts_with("https://") {
        format!("http://{}", url)
    } else {
        url.to_string()
    };

    // Parse URL to validate and normalize
    match Url::parse(&url_with_scheme) {
        Ok(parsed_url) => {
            // Return normalized URL string
            let normalized = parsed_url.to_string();
            // Remove trailing slash for consistency
            let normalized = normalized.trim_end_matches('/').to_string();
            Ok(normalized)
        },
        Err(e) => {
            error!(url = %url, error = %e, "Failed to parse URL");
            Err(anyhow!("Invalid URL format: {}", e))
        }
    }
}
```

## Importance in the A2A Ecosystem

### 1. Enabling Agent Networks

The Agent Registry is a fundamental building block for creating effective agent networks. Without it, agents would exist in isolation, unable to discover and collaborate with each other. The registry serves as the connective tissue that transforms individual agents into a powerful, collaborative network.

### 2. Facilitating Specialization

In an effective agent ecosystem, agents should specialize rather than attempting to do everything. The registry enables this specialization by making it easy to:

- Discover specialized agents
- Understand their capabilities
- Connect to them for specific tasks

This leads to a more efficient ecosystem where each agent can focus on what it does best.

### 3. Supporting Runtime Adaptation

As new agents come online or existing agents update their capabilities, the registry allows the network to adapt dynamically. This runtime adaptation is crucial for a living, evolving agent ecosystem.

### 4. Creating Network Effects

As more agents register themselves, the value of the registry increases for all participants. This positive network effect accelerates adoption and enhances the utility of the entire A2A ecosystem.

### 5. Providing Trust Infrastructure

By validating and storing agent information, the registry provides a foundation for trust between agents. This is crucial for building confident, reliable agent interactions.

## Future Enhancements

### 1. Agent Verification and Trust Scores

Implementing a verification system that assigns trust scores to agents based on their interaction history, provider reputation, and verification status.

### 2. Capability-Based Routing

Enhancing the registry to support routing requests to agents based on specific capabilities rather than just agent identity.

### 3. Federation and Distribution

Implementing a federation protocol allowing multiple registries to synchronize information, creating a distributed agent discovery network.

### 4. Agent Health Monitoring

Adding proactive health checks to monitor agent availability and performance metrics.

### 5. Semantic Search

Implementing natural language search capabilities to find agents based on capability descriptions rather than exact matching.

### 6. Authentication and Access Control

Adding support for authenticated registry access with different permission levels for agent registration and discovery.

### 7. Versioning Support

Enhancing the registry to track and manage multiple versions of the same agent.

## Conclusion

The Agent Registry is a cornerstone component of the A2A ecosystem, enabling agent discovery, collaboration, and network formation. By providing a centralized directory with robust URL handling, agent card integration, and persistent storage, it solves the critical challenges of agent discovery and interoperability.

In the rapidly evolving landscape of AI agents, the registry serves as the map that helps agents navigate the terrain. It transforms a collection of isolated agents into a powerful, interconnected network that can collaboratively solve complex problems.

As the A2A protocol continues to mature, the Agent Registry will play an increasingly important role in fostering a rich ecosystem of specialized, interoperable agents working together to provide enhanced capabilities to users and systems.