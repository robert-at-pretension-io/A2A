# Running the Bidirectional A2A Agent

The bidirectional agent is a standalone binary that can be used to interact with A2A-compatible agents, serve as an A2A agent itself, or both simultaneously.

## Claude API Key Setup

For local message processing, you need to provide a Claude API key in one of two ways:

1. **Environment Variable (recommended)**:
   ```bash
   export CLAUDE_API_KEY=your-api-key-here
   ./target/debug/bidirectional-agent
   ```
   
   Or as a one-liner:
   ```bash
   CLAUDE_API_KEY=your-api-key-here ./target/debug/bidirectional-agent
   ```

2. **In the TOML configuration file**:
   ```toml
   [llm]
   claude_api_key = "your-api-key-here"
   ```

The environment variable takes precedence over the TOML configuration.

## Building the Agent

To build the agent:

```bash
# Build in debug mode
RUSTFLAGS="-A warnings" cargo build --bin bidirectional-agent

# Build in release mode for better performance
RUSTFLAGS="-A warnings" cargo build --release --bin bidirectional-agent
```

## Running the Agent

The agent has several ways to start:

### 1. Simple REPL Mode (No Arguments)

```bash
# Using cargo
cargo run --bin bidirectional-agent

# Using the built binary
./target/debug/bidirectional-agent
```

This starts the agent in interactive REPL mode with default settings.

### 2. Connect to a Remote Server

```bash
# Connect to a server on localhost port 8080
cargo run --bin bidirectional-agent -- localhost:8080

# Or with the binary
./target/debug/bidirectional-agent localhost:8080
```

This automatically connects to the specified server and starts the REPL.

### 3. Using a Configuration File

```bash
# Load settings from a TOML config file
cargo run --bin bidirectional-agent -- bidirectional_agent.toml

# Or with the binary
./target/debug/bidirectional-agent bidirectional_agent.toml
```

## REPL Commands

Once in the REPL, you can use these commands:

- `:help` - Show help message
- `:card` - Show agent card
- `:servers` - List known remote servers
- `:connect URL` - Connect to a remote agent at URL
- `:connect HOST:PORT` - Connect to a remote agent by host and port
- `:connect N` - Connect to Nth server in the server list
- `:disconnect` - Disconnect from current remote agent
- `:remote MSG` - Send message as task to connected agent
- `:listen PORT` - Start listening server on specified port
- `:stop` - Stop the currently running server
- `:quit` - Exit the REPL

For any message that isn't a command (doesn't start with `:`), the agent will process it locally.

## Example Session

```
$ cargo run --bin bidirectional-agent

========================================
⚡ Bidirectional A2A Agent REPL Mode ⚡
========================================
Type a message to process it directly with the agent.
Special commands:
  :help            - Show this help message
  :card            - Show agent card
  :servers         - List known remote servers
  :connect URL     - Connect to a remote agent at URL
  :connect HOST:PORT - Connect to a remote agent by host and port
  :connect N       - Connect to Nth server in the server list
  :disconnect      - Disconnect from current remote agent
  :remote MSG      - Send message as task to connected agent
  :listen PORT     - Start listening server on specified port
  :stop            - Stop the currently running server
  :quit            - Exit the REPL
========================================

agent> :listen 8080
🚀 Starting server on port 8080...
✅ Server started on http://0.0.0.0:8080
The server will run until you exit the REPL or send :stop

agent> :connect localhost:8080
🔗 Connected to remote agent: localhost:8080
✅ Successfully connected to agent: Bidirectional A2A Agent

agent@localhost:8080> What is the capital of France?
🤖 Agent response:
The capital of France is Paris.

agent@localhost:8080> :quit
Exiting REPL. Goodbye!
Shutting down server...
```