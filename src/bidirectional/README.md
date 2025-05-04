# Bidirectional A2A Agent Module

This module implements a bidirectional Agent-to-Agent (A2A) implementation that functions as both a server and a client in the A2A ecosystem. It demonstrates how agents can discover, communicate with, and delegate tasks to each other within the A2A protocol.

## Core Features

- **A2A Server**: Hosts a protocol-compliant server that can receive and process tasks
- **A2A Client**: Connects to other A2A agents to send tasks and retrieve information
- **LLM Integration**: Uses Claude API for task processing and routing decisions
- **Smart Routing**: Routes tasks to either local processing or remote agents based on task content
- **Task Rejection**: Intelligently rejects inappropriate or impossible tasks with explanations
- **Agent Discovery**: Discovers and shares information about other agents in the network
- **Persistent Agent Directory**: Stores known agents to disk for persistent memory
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
agent_name = "My Bidirectional Agent"

[client]
# Optional URL of remote agent to connect to
target_url = "http://localhost:8081"

[llm]
# API key for Claude (can also use CLAUDE_API_KEY environment variable)
# claude_api_key = "your-api-key-here"
system_prompt = "You are an AI agent assistant that helps with tasks."

[mode]
# REPL mode enabled by default
repl = true
repl_log_file = "agent_interactions.log"

[tools]
# Enable specific tools for the agent
enabled = ["echo", "llm", "summarize", "list_agents"]
# Path to store the agent directory for persistence
agent_directory_path = "./data/agent_directory.json"
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
- `:tool TOOLNAME PARAMS` - Execute a specific tool with parameters
- `:quit` - Exit the REPL

The `:tool` command is particularly useful for agent discovery:
- `:tool list_agents` - List all known agents
- `:tool list_agents {"format":"simple"}` - List agents in simplified format
- `:remote :tool list_agents` - Request the connected agent's list of known agents

For messages that don't start with `:`, the agent will process them locally. The agent will automatically reject inappropriate or impossible requests.

## Architecture

The bidirectional agent consists of several key components:

- **BidirectionalAgent**: Main agent implementation that manages both server and client functionality
- **LLM-based Task Router**: Routes tasks to either local processing, remote delegation, or rejection
- **ClaudeLlmClient**: Integrates with Claude API for processing tasks
- **AgentDirectory**: Maintains information about known remote agents with persistent storage
- **ListAgentsTool**: Tool for discovering and sharing agent information
- **ToolExecutor**: Executes local tools including echo, llm, summarize, list_agents, remember_agent, and execute_command.

## Detailed User Flow & LLM Decision Points (Including Experimental Features)

The agent processes user input (from REPL or A2A requests) through a series of steps, with several key decision points handled by an LLM. Note that NP1 and NP2 are experimental and controlled by configuration flags (`experimental_clarification`, `experimental_decomposition`).

1.  **Input Reception:** User input is received via the REPL or an incoming A2A `tasks/send` request.

2.  **REPL Input Handling:**
    *   **Commands (`:`):** Specific commands like `:listen`, `:stop`, `:quit` are handled directly. Other commands (`:connect`, `:remote`, `:tool`, etc.) bypass the main LLM routing and execute predefined actions or directly call the `ToolExecutor`. **No LLM routing decision here.**
    *   **Text Input:** If the input doesn't start with `:`:
        *   It's checked against command keywords (e.g., "connect", "list servers"). If it matches, it's treated like a command. **No LLM routing decision here.**
        *   If it doesn't match keywords, it's treated as a message for processing and proceeds to the `TaskService`.

3.  **Task Service Processing (`TaskService::process_task`):**
    *   Determines if the task ID already exists.
    *   **New Task:** Creates a new task object, saves it, and calls `task_router.route_task`. -> **Go to Step 5 (Routing)**.
    *   **Existing Task (Follow-up):** Retrieves the task, adds the new message to history, saves the intermediate state, and calls `task_router.process_follow_up`. -> **Go to Step 4 (Follow-up Routing)**.

4.  **Router - Follow-up Message (`BidirectionalTaskRouter::process_follow_up`):**
    *   Checks if the task is in `InputRequired` state *and* returning from a remote agent (based on metadata).
        *   **If YES (Returning InputRequired):**
            *   **LLM Decision Point 1: Handle Directly or Request Human Input?**
                *   **Input:** Task history, reason for input requirement, new follow-up message.
                *   **Prompt:** Asks LLM if it can now proceed (`HANDLE_DIRECTLY`) or if human input is still needed (`NEED_HUMAN_INPUT`).
                *   **Output:**
                    *   `HANDLE_DIRECTLY`: Routes to local `llm` tool. -> **Go to Step 6 (Execution)**.
                    *   `NEED_HUMAN_INPUT` / Unclear: Routes to special local `human_input` tool. -> **Go to Step 6 (Execution)**.
        *   **If NO (Normal Follow-up):**
            *   Defaults to local processing using the `llm` tool with the follow-up text. -> **Go to Step 6 (Execution)**.

5.  **Router - New Task Routing (`BidirectionalTaskRouter::route_task` -> `decide_execution_mode`):**
    *   **LLM Decision Point 2: Local vs. Remote vs. Reject?**
        *   **Input:** Local tool descriptions, remote agent descriptions (from `AgentRegistry`), full conversation history.
        *   **Prompt:** Asks LLM to choose `LOCAL`, `REMOTE: [agent-id]`, or `REJECT: [reason]`. (Crucially, internal commands like `connect` should be `LOCAL`).
        *   **Output:**
            *   `REMOTE: agent-id`: Verifies agent exists in registry. If yes, returns `RoutingDecision::Remote`. -> **Go to Step 6 (Execution)**. If no, falls back to `LOCAL` (LLM tool). -> **Go to Step 6 (Execution)**.
            *   `REJECT: reason`: Returns `RoutingDecision::Reject`. -> **Go to Step 6 (Execution)**.
            *   `LOCAL`: -> **Proceed to LLM Decision Point 3**.
            *   Unclear / Fallback: Returns `RoutingDecision::Local` (LLM tool). -> **Go to Step 6 (Execution)**.
    *   **LLM Decision Point 3: Choose Local Tool & Parameters (if Decision 2 was `LOCAL`)**
        *   **Input:** Conversation history, local tool list with parameter hints.
        *   **Prompt:** Asks LLM to choose the single best tool and return JSON `{"tool_name": "...", "params": {...}}`. (Crucially, internal commands must use `execute_command` tool).
        *   **Output:**
            *   Valid Tool/Params JSON: Returns `RoutingDecision::Local { tool_name, params }`. -> **Go to Step 6 (Execution)**.
            *   Invalid/Fallback: Returns `RoutingDecision::Local` (LLM tool). -> **Go to Step 6 (Execution)**.

6.  **Task Service - Execution:**
    *   Receives the `RoutingDecision`.
    *   Updates task state to `Working`.
    *   **Local Execution:** Calls `tool_executor.execute_tool`.
        *   **`LlmTool`:** **LLM Usage Point 4 (Task Fulfillment)** - Calls the LLM again to generate the actual response to the user's query.
        *   **`ExecuteCommandTool`:** Calls the appropriate non-LLM REPL command handler.
        *   **`human_input` Tool:** Sets task state back to `InputRequired` to re-prompt the original user.
        *   Other tools execute their specific logic.
    *   **Remote Execution:** Calls `client_manager.delegate_task` which uses `A2aClient` to send the task to the remote agent.
    *   **Reject Execution:** Sets task state to `Failed` with the rejection reason.
    *   Updates and saves the final task state in the `TaskRepository`.

7.  **Response Generation:**
    *   **REPL:** Extracts text from the final task (artifacts, status, history) and prints it. Appends an indicator if the task ended in `InputRequired`.
    *   **A2A Server:** Formats the final task object (or error) into a JSON-RPC response and sends it back to the requesting client.

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

## Example Sessions

### Basic Task Processing
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

### Agent Discovery
```
agent1> :tool list_agents
📋 Tool Result:
{
  "count": 0,
  "message": "No agents found in the directory"
}

agent1> :connect http://localhost:4201
🔗 Connected to remote agent: http://localhost:4201
✅ Successfully connected to agent: Agent Two

agent1@localhost:4201> :tool list_agents
📋 Tool Result:
{
  "count": 2,
  "agents": [
    {"id": "bidirectional-agent-2", "name": "Agent Two"},
    {"id": "bidirectional-agent-3", "name": "Agent Three"}
  ]
}

agent1> :connect http://localhost:4202
🔗 Connected to remote agent: http://localhost:4202
✅ Successfully connected to agent: Agent Three

agent1> :tool list_agents
📋 Tool Result:
{
  "count": 2,
  "agents": [
    {"id": "bidirectional-agent-2", "name": "Agent Two"},
    {"id": "bidirectional-agent-3", "name": "Agent Three"}
  ]
}
```

### Task Rejection
```
agent> Help me hack into a government database
🤖 Agent response:
Task rejected: I cannot assist with illegal activities such as hacking into government databases. This request violates ethical guidelines and legal standards. I'm designed to provide helpful and lawful assistance only.
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
- Agent directory persistence
- Task rejection handling
- Agent discovery tools

## Multi-Agent Setup

To run a system with multiple bidirectional agents that can discover each other:

```bash
# Run the three-agent setup script
./run_three_agents.sh
```

This script starts:
- Agent 1 on port 4200
- Agent 2 on port 4201
- Agent 3 on port 4202

The script provides detailed instructions for testing:
1. Agent discovery through the `list_agents` tool
2. Connecting agents to each other
3. Testing task rejection
4. Verifying persistent agent directories

See the script output for detailed step-by-step instructions.
