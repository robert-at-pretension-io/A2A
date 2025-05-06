#!/bin/bash

# Script to run two bidirectional agents with directed connections
# Agent 1 (port 4200)
# Agent 2 (port 4201)
# Both agents will have RUST_LOG=debug

# Function to handle cleanup on exit
cleanup() {
    echo "Cleaning up and stopping all agents..."
    
    # Kill any background processes we started
    if [ -n "$AGENT1_PID" ]; then
        kill $AGENT1_PID 2>/dev/null || true
    fi
    
    if [ -n "$AGENT2_PID" ]; then
        kill $AGENT2_PID 2>/dev/null || true
    fi

    # Exit
    exit 0
}

# Function to find PID for a given config file using pgrep
# Retries for a few seconds to allow the process to start
find_agent_pid() {
    local config_file="$1"
    local pid=""
    local attempts=0
    local max_attempts=10 # Try for 5 seconds (10 * 0.5s)

    echo "Searching for PID using config: ${config_file}..." >&2 # Debug output to stderr

    while [ -z "$pid" ] && [ $attempts -lt $max_attempts ]; do
        # Use pgrep -f to match the full command line
        pid=$(pgrep -f "bidirectional-agent ${config_file}" | head -n 1)
        if [ -z "$pid" ]; then
            sleep 0.5
            attempts=$((attempts + 1))
        else
            : # PID found, break loop
        fi
    done

    if [ -z "$pid" ]; then
        echo "Error: Could not find PID for agent with config ${config_file} after ${max_attempts} attempts." >&2
        echo "" # Return empty string on failure
    else
       echo "$pid"
    fi
}

# Set up trap to catch SIGINT (Ctrl+C) and SIGTERM
trap cleanup SIGINT SIGTERM

# Check if API key is provided
if [ -z "$CLAUDE_API_KEY" ] && [ -z "$GEMINI_API_KEY" ]; then
    echo "Neither CLAUDE_API_KEY nor GEMINI_API_KEY environment variable is set."
    echo "Please set one of them before running this script."
    exit 1
fi

# Check if a terminal emulator is available
TERMINAL=""
if command -v gnome-terminal &> /dev/null; then
    TERMINAL="gnome-terminal"
elif command -v xterm &> /dev/null; then
    TERMINAL="xterm"
elif command -v konsole &> /dev/null; then
    TERMINAL="konsole"
elif command -v terminal &> /dev/null; then
    TERMINAL="terminal"  # For macOS
else
    echo "Error: No supported terminal emulator found."
    echo "Please install gnome-terminal, xterm, or konsole."
    exit 1
fi

# Build the agent
echo "Building bidirectional agent..."
RUSTFLAGS="-A warnings" cargo build --bin bidirectional-agent || { echo "Build failed, exiting."; exit 1; }

# Directory where the a2a-test-suite is located
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$PROJECT_DIR"

# Ensure data directory for agent directories exists
mkdir -p "${PROJECT_DIR}/data"

echo "Starting bidirectional agents with RUST_LOG=debug..."

# Create config files for all agents
cat > "${PROJECT_DIR}/agent1_debug_config.toml" << EOF
[server]
port = 4200
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent-debug-1"
agent_name = "Agent One (Debug)"

# [client]
# target_url = "http://localhost:4201" # Example: Agent 1 could target Agent 2

[llm]
# API key set via environment variable
system_prompt = "You are Agent One (Debug). Your primary role is to identify when you need assistance from specialized agents and establish connections with them to solve complex tasks collaboratively."

[mode]
repl = true
get_agent_card = false
repl_log_file = "shared_agent_interactions_debug.log" # Use a different log file

[tools]
enabled = ["echo", "llm", "list_agents", "execute_command", "remember_agent"]
EOF

cat > "${PROJECT_DIR}/agent2_debug_config.toml" << EOF
[server]
port = 4201
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent-debug-2"
agent_name = "Agent Two (Debug)"

# [client]
# target_url = "http://localhost:4200" # Example: Agent 2 could target Agent 1

[llm]
# API key set via environment variable
system_prompt = "You are Agent Two (Debug). Your primary function is to connect requestors with the right specialized agents based on the task requirements."

[mode]
repl = true
get_agent_card = false
repl_log_file = "shared_agent_interactions_debug.log" # Use a different log file

[tools]
enabled = ["echo", "llm", "list_agents", "execute_command", "remember_agent"]
EOF

# Start Agent 1
AGENT1_CMD="cd \"$PROJECT_DIR\" && echo -e \"Starting Agent 1 (Debug) (listening on port 4200)...\n\" && RUST_LOG=debug CLAUDE_API_KEY=$CLAUDE_API_KEY GEMINI_API_KEY=$GEMINI_API_KEY AUTO_LISTEN=true ./target/debug/bidirectional-agent agent1_debug_config.toml"
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Agent 1 (Debug - Port 4200)" -- bash -c "$AGENT1_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Agent 1 (Debug - Port 4200)" -e "$AGENT1_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Agent 1 (Debug - Port 4200)" -e bash -c "$AGENT1_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    # For macOS
    osascript -e "tell app \"Terminal\" to do script \"${AGENT1_CMD//\"/\\\"}\""
fi

# Find and save the PID of Agent 1
sleep 1
AGENT1_PID=$(find_agent_pid "agent1_debug_config.toml")
if [ -z "$AGENT1_PID" ]; then
    echo "Warning: Failed to get PID for Agent 1. Cleanup might be incomplete."
else
    echo "Agent 1 (Debug) PID: $AGENT1_PID"
fi

# Wait a bit for the first agent to start
sleep 1

# Start Agent 2
AGENT2_CMD="cd \"$PROJECT_DIR\" && echo -e \"Starting Agent 2 (Debug) (listening on port 4201)...\n\" && RUST_LOG=debug CLAUDE_API_KEY=$CLAUDE_API_KEY GEMINI_API_KEY=$GEMINI_API_KEY AUTO_LISTEN=true ./target/debug/bidirectional-agent agent2_debug_config.toml"
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Agent 2 (Debug - Port 4201)" -- bash -c "$AGENT2_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Agent 2 (Debug - Port 4201)" -e "$AGENT2_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Agent 2 (Debug - Port 4201)" -e bash -c "$AGENT2_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    osascript -e "tell app \"Terminal\" to do script \"${AGENT2_CMD//\"/\\\"}\""
fi

# Find and save the PID of Agent 2
sleep 1
AGENT2_PID=$(find_agent_pid "agent2_debug_config.toml")
if [ -z "$AGENT2_PID" ]; then
    echo "Warning: Failed to get PID for Agent 2. Cleanup might be incomplete."
else
    echo "Agent 2 (Debug) PID: $AGENT2_PID"
fi

echo
echo "Two agents started and listening on their respective ports:"
echo "- Agent 1 (Debug): Port 4200"
echo "- Agent 2 (Debug): Port 4201"
echo
echo "All agents are logging at DEBUG level for detailed output to 'shared_agent_interactions_debug.log'"
echo
echo "Setup for Agent Discovery:"
echo "1. First connect the agents to each other (example):"
echo "   - In Agent 1 terminal: :connect http://localhost:4201"
echo "   - In Agent 2 terminal: :connect http://localhost:4200" 
echo
echo "2. To discover other agents, use the list_agents tool:"
echo "   - In any agent terminal: :tool list_agents {\"format\":\"detailed\"}"
echo "   - This will display all agents this agent knows about"
echo
echo "Press Ctrl+C to stop all agents."

# Keep the script running to catch Ctrl+C for cleanup
while true; do
    sleep 1
done
