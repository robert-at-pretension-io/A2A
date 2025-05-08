# Bidirectional A2A Agent

> Related Documentation:
> - [Project Overview](README.md)
> - [Bidirectional Agent Documentation](src/bidirectional/README.md)
> - [Bidirectional Agent Quickstart](README_BIDIRECTIONAL.md)
> - [HTTPS Setup](README_HTTPS.md)
> - [LLM Configuration](docs/llm_configuration.md)
> - [Rolling Memory](docs/rolling_memory.md)

This is a bidirectional A2A (Agent-to-Agent) implementation that can function as both a server and a client in the A2A ecosystem. It includes the following features:

- **A2A Server**: Hosts an A2A-compliant server that can receive and process tasks
- **A2A Client**: Connects to other A2A agents to send tasks and retrieve information
- **LLM Integration**: Uses Claude API for task processing and routing decisions
- **Smart Routing**: Routes tasks to either local processing or remote agents based on task content
- **Interactive REPL**: Command-line interface for interacting with agents

## Requirements

- Rust toolchain (1.70.0 or later)
- Claude API key (for LLM-based features)
- (Optional) TOML configuration file

## Building the Agent

To build the agent:

```bash
# Build in debug mode
RUSTFLAGS="-A warnings" cargo build --bin bidirectional-agent

# Build in release mode for better performance
RUSTFLAGS="-A warnings" cargo build --release --bin bidirectional-agent
```

The compiled binary will be available at:
- Debug mode: `./target/debug/bidirectional-agent`
- Release mode: `./target/release/bidirectional-agent`

## Configuration

The agent is configured using a TOML configuration file. A sample `bidirectional_agent.toml` file is included in the project root.

```toml
# Bidirectional A2A Agent Configuration

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
# System prompt for the LLM
system_prompt = """
You are an AI agent assistant that helps with tasks. You can:
1. Process tasks directly (for simple questions or tasks you can handle)
2. Delegate tasks to other agents when appropriate
3. Use tools when needed

Always think step-by-step about the best way to handle each request.
"""

[tools]
# List of tools to enable or disable (depending on is_exclusion_list)
# Available tools: "llm", "summarize", "echo", "list_agents", "remember_agent", "execute_command"
# If empty and is_exclusion_list=false, ALL tools will be enabled
enabled = []

# If true, treat 'enabled' as an exclusion list (enable all EXCEPT these)
# If false, treat 'enabled' as an inclusion list (ONLY enable these)
is_exclusion_list = false

# Mode configuration - uncomment the desired mode
[mode]
# Interactive REPL mode
repl = true

# Direct message to process (non-interactive mode)
# message = "What is the capital of France?"

# Remote agent operations
# get_agent_card = false
# remote_task = "Hello from bidirectional agent!"
```

## Usage

### Execution Modes

The agent can operate in different modes, controlled by the `[mode]` section in the TOML configuration:

1. **Server Mode** (default): Starts an A2A server to receive and process tasks
2. **REPL Mode**: Interactive command-line interface for direct interaction
3. **Direct Message Processing**: Processes a single message non-interactively
4. **Remote Agent Operations**: Interacts with remote A2A agents

For example, to run in REPL mode:

```toml
[mode]
repl = true
```

### Starting the Agent

The agent can be started in several ways:

#### 1. Start with no arguments (defaults to REPL mode):

```bash
# Using cargo
cargo run --bin bidirectional-agent

# Using the built binary
./target/debug/bidirectional-agent
```

This starts the agent in REPL mode with sensible defaults:
- Default port is 8080
- Default bind address is 0.0.0.0
- A random agent ID is generated

#### 2. Start with server:port to connect to a remote agent:

```bash
# Using cargo
cargo run --bin bidirectional-agent -- localhost:8080

# Using the built binary
./target/debug/bidirectional-agent localhost:8080
```

This starts the agent in REPL mode and automatically connects to the specified remote agent.

#### 3. Start with a configuration file:

```bash
# Using cargo
cargo run --bin bidirectional-agent -- config_file.toml

# Using the built binary
./target/debug/bidirectional-agent config_file.toml
```

This loads the settings from the specified TOML file.

The simplified command-line interface makes it easy to start the agent and connect to remote servers quickly without having to create a configuration file first.

### Interactive REPL Mode

When configured with `[mode] repl = true`, the agent starts in interactive REPL (Read-Eval-Print Loop) mode.

In REPL mode, you can:
- Type messages to process them directly with the agent
- Connect to and interact with remote agents
- Start your own server and listen on a specified port
- View agent information
- Keep track of known servers

Available REPL commands:
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

### Client Operations

#### Process a Single Message

Process a message directly using the agent without going through the A2A protocol:

```toml
[mode]
message = "What is the capital of France?"
```

#### Get Remote Agent Card

Retrieve and display the agent card from a remote A2A agent:

```toml
[mode]
get_agent_card = true
```

#### Send Task to Remote Agent

Send a task to a remote A2A agent:

```toml
[mode]
remote_task = "Hello from bidirectional agent!"
```

## Extending the Agent

The bidirectional agent is designed for extensibility. Here are some ways to extend it:

1. **Add More Client Operations**: Implement additional A2A client functionality in the `BidirectionalAgent` struct
2. **Enhance Routing Logic**: Modify the `BidirectionalTaskRouter` to make smarter routing decisions
3. **Implement Tools**: Add local tool implementations that can be executed by the agent
4. **Add Authentication**: Implement authentication for accessing the agent's API

## Architecture

The bidirectional agent consists of several key components:

- **BidirectionalAgent**: Main agent implementation that manages both server and client functionality
- **BidirectionalTaskRouter**: Routes tasks to either local processing or remote delegation
- **ClaudeLlmClient**: Integrates with Claude API for processing tasks and making routing decisions
- **AgentDirectory**: Maintains information about known remote agents for delegation purposes

The agent follows the standard A2A protocol for both server and client operations, ensuring compatibility with other A2A-compliant agents.