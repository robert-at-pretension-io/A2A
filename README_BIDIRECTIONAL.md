# Bidirectional A2A Agent Quick Start Guide

> For a more detailed guide, see the [Complete Bidirectional Agent Documentation](src/bidirectional/README.md) or the [Bidirectional Agent README](bidirectional_agent_readme.md).

This guide provides quick-start instructions for running the Bidirectional A2A Agent, a component of the A2A Test Suite that can function as both a client and server in the A2A protocol ecosystem.

## Prerequisites

- Rust toolchain (1.70.0 or later recommended)
- Claude API key (for LLM-based routing features) or Gemini API key
- (Optional) TOML configuration file

## Running the Agent

### Simplest method (defaults)

```bash
cargo run -- bidirectional
```

This starts the agent in interactive REPL mode with sensible defaults.

### With a configuration file

```bash
cargo run -- bidirectional --config bidirectional_agent.toml
```

A sample configuration file is provided in the repository root.

### Connecting to a specific server

```bash
cargo run -- bidirectional localhost:8080
```

This starts the agent and automatically connects to the specified server.

## REPL Commands

Once in the REPL, you can:

- Type messages directly to process them with the local agent
- Use the following commands:

```
:help                   Show this help message
:card                   Show agent card (capabilities)
:servers                List known remote servers
:connect URL            Connect to a remote agent at URL
:connect N              Connect to Nth server in the servers list
:disconnect             Disconnect from current remote agent
:remote MESSAGE         Send message as task to connected agent
:listen PORT            Start listening server on specified port
:stop                   Stop the currently running server
:ns, :new-session       Create a new session
:tasks                  List tasks in current session
:task ID                Show detailed info for a specific task
:file PATH              Create and send a file as an artifact
:data JSON              Create and send JSON data as an artifact
:history ID             Show full conversation history for a task
:artifacts ID           Download all artifacts for a task
:tool TOOL_NAME ARGS    Execute a specific tool command
:mem, :memory           Show contents of agent's memory
:clear-mem              Clear agent's rolling memory
:quit                   Exit the REPL
```

## Example Session

```
🤖 A2A Agent started. Type a message or command (:help for info)
> :listen 8080
✅ Server started on port 8080

> :connect localhost:8081
✅ Connected to Testing A2A Agent

> :remote Hello, can you help me analyze this data?
📤 Sending message to remote agent...
📥 Response: I'd be happy to help you analyze your data! To get started, could you please:

1. Tell me more about what kind of data you have
2. Share what specific insights you're looking for
3. Provide the data as a file or in JSON format

You can use `:file PATH` to send me a file, or `:data JSON` to send structured data.

> :data {"values": [12, 18, 24, 32, 45, 52]}
📤 Sending data to remote agent...
📥 Response: Here's a quick analysis of your data:

Sample: [12, 18, 24, 32, 45, 52]
Count: 6 values
Sum: 183
Mean: 30.5
Median: 28
Range: 40 (12 to 52)
Standard Deviation: ~15.38

The data shows a steadily increasing pattern with each value larger than the previous one.
Would you like me to perform any specific statistical tests or provide a visualization?

> :disconnect
🔌 Disconnected from localhost:8081

> :quit
👋 Goodbye!
```

## Related Documentation

For more detailed information, see:
- [Complete Bidirectional Agent Documentation](src/bidirectional/README.md)
- [LLM Configuration Guide](docs/llm_configuration.md) - For configuring Claude or Gemini
- [Rolling Memory Documentation](docs/rolling_memory.md) - For understanding how context is preserved
- [Agent Registry Documentation](docs/agent_registry.md) - For managing known agents 
- [HTTPS Setup Guide](README_HTTPS.md) - For enabling secure communication

## Convenience Scripts

Several scripts are provided to help you get started:
- `run_agent.bat` - Windows script for running the agent
- `run_remember_agent_demo.sh` - Demonstrates the rolling memory feature
- `run_three_agents.sh` - Runs three interconnected agents for testing
- `run_two_agents_debug.sh` - Runs two agents in debug mode