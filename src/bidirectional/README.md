# Bidirectional A2A Agent Module

This module implements a bidirectional Agent-to-Agent (A2A) implementation that functions as both a server and a client in the A2A ecosystem. It demonstrates how agents can discover, communicate with, and delegate tasks to each other within the A2A protocol.

## Core Features

- **A2A Server**: Hosts a protocol-compliant server that can receive and process tasks
- **A2A Client**: Connects to other A2A agents to send tasks and retrieve information
- **LLM Integration**: Uses Claude API for task processing and routing decisions
- **Smart Routing**: Routes tasks to either local processing or remote agents based on task content
- **Interactive REPL**: Command-line interface for interacting with agents

## Module Structure

- `bidirectional_agent.rs`: Main implementation of the bidirectional agent
- `bin.rs`: Binary entrypoint for standalone operation
- `mod.rs`: Module exports
- `tests/`: Comprehensive test suite

## Configuration

The bidirectional agent is configured using a TOML file. A sample configuration file (`bidirectional_agent.toml`) is included in the project root. Key configuration sections include:

```toml
[server]
port = 8080
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent"

[client]
# Optional URL of remote agent to connect to
target_url = "http://localhost:8081"

[llm]
# API key for Claude (can also use CLAUDE_API_KEY environment variable)
# claude_api_key = "your-api-key-here"

[mode]
# REPL mode enabled by default
repl = true
```

## Runtime Modes

The agent supports multiple operation modes:

1. **Server Mode**: Listens for incoming A2A requests
2. **Client Mode**: Connects to other A2A agents
3. **REPL Mode**: Interactive command-line interface
4. **Direct Message Mode**: Processes a single message and exits
5. **Remote Operations**: Interacts with remote agents (get card, send task)

## REPL Commands

In interactive REPL mode, you can use these commands:

- `:help` - Show help message
- `:card` - Show agent card
- `:servers` - List known remote servers
- `:connect URL` - Connect to a remote agent at URL
- `:connect N` - Connect to Nth server in the servers list
- `:disconnect` - Disconnect from current remote agent
- `:remote MESSAGE` - Send message as task to connected agent
- `:listen PORT` - Start listening server on specified port
- `:stop` - Stop the currently running server
- `:quit` - Exit the REPL

For messages that don't start with `:`, the agent will process them locally.

## Architecture

The bidirectional agent consists of several key components:

- **BidirectionalAgent**: Main agent implementation that manages both server and client functionality
- **LLM-based Task Router**: Routes tasks to either local processing or remote delegation
- **ClaudeLlmClient**: Integrates with Claude API for processing tasks
- **AgentDirectory**: Maintains information about known remote agents

## Usage

For detailed usage instructions, see:
- [README_BIDIRECTIONAL.md](/README_BIDIRECTIONAL.md) - Quick start guide
- [bidirectional_agent_readme.md](/bidirectional_agent_readme.md) - Comprehensive documentation

## Running the Agent

```bash
# Start with default settings in REPL mode
cargo run --bin bidirectional-agent

# Start with a configuration file
cargo run --bin bidirectional-agent -- bidirectional_agent.toml

# Start and connect to a remote agent
cargo run --bin bidirectional-agent -- localhost:8080
```

## Example Session

```
agent> :listen 8080
🚀 Starting server on port 8080...
✅ Server started on http://0.0.0.0:8080

agent> :connect localhost:8080
🔗 Connected to remote agent: localhost:8080
✅ Successfully connected to agent: Bidirectional A2A Agent

agent@localhost:8080> What is the capital of France?
🤖 Agent response:
The capital of France is Paris.
```

## Testing

The module includes a comprehensive test suite in the `tests/` directory covering:

- Agent functionality tests
- Artifact handling
- Configuration parsing
- Input requirement handling
- REPL command processing
- Router decision making
- Session management
- Task service integration