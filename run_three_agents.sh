#!/bin/bash

# Script to run three bidirectional agents with directed connections
# Agent 1 (port 4200) connects to Agent 2 (port 4201)
# Agent 2 (port 4201) connects to both Agent 1 (port 4200) and Agent 3 (port 4202)
# Agent 3 (port 4202) connects to Agent 2 (port 4201)

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

    if [ -n "$AGENT3_PID" ]; then
        kill $AGENT3_PID 2>/dev/null || true
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
if [ -z "$CLAUDE_API_KEY" ]; then
    echo "CLAUDE_API_KEY environment variable is not set."
    echo "Please set it before running this script."
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

echo "Starting bidirectional agents..."

# Create config files for all agents
cat > "${PROJECT_DIR}/agent1_config.toml" << EOF
[server]
port = 4200
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent-1"
agent_name = "Agent One"

[client]
target_url = "http://localhost:4201"

[llm]
# API key set via environment variable
system_prompt = "You are an AI agent that can communicate with other agents."

[mode]
repl = true
get_agent_card = false
repl_log_file = "shared_agent_interactions.log"

[tools]
enabled = ["echo", "llm", "list_agents", "execute_command"] # <-- Add execute_command
# REMOVED agent_directory_path
EOF

cat > "${PROJECT_DIR}/agent2_config.toml" << EOF
[server]
port = 4201
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent-2"
agent_name = "Agent Two"

[client]
target_url = "http://localhost:4200"

[llm]
# API key set via environment variable
system_prompt = "You are an expert agent-discovery and routing agent. When Agent One asks about other available agents, you should recommend and provide information about Agent Three. Agent Three is an expert in data analysis tasks."

[mode]
repl = true
get_agent_card = false
repl_log_file = "shared_agent_interactions.log"

[tools]
enabled = ["echo", "llm", "list_agents", "execute_command"] # <-- Add execute_command
# REMOVED agent_directory_path
EOF

cat > "${PROJECT_DIR}/agent3_config.toml" << EOF
[server]
port = 4202
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent-3"
agent_name = "Agent Three"

[client]
target_url = "http://localhost:4201"

[llm]
# API key set via environment variable
system_prompt = "You are an AI agent specialized in data analysis that can communicate with other agents."

[mode]
repl = true
get_agent_card = false
repl_log_file = "shared_agent_interactions.log"

[tools]
enabled = ["echo", "summarize", "list_agents", "remember_agent", "execute_command"] # <-- Add execute_command
# REMOVED agent_directory_path
EOF

# Start Agent 1
AGENT1_CMD="cd \"$PROJECT_DIR\" && echo -e \"Starting Agent 1 (listening on port 4200, connecting to 4201)...\n\" && RUST_LOG=info CLAUDE_API_KEY=$CLAUDE_API_KEY AUTO_LISTEN=true ./target/debug/bidirectional-agent agent1_config.toml" # Changed to info
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Agent 1 (Port 4200)" -- bash -c "$AGENT1_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Agent 1 (Port 4200)" -e "$AGENT1_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Agent 1 (Port 4200)" -e bash -c "$AGENT1_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    # For macOS
    terminal -e "$AGENT1_CMD" &
fi

# Find and save the PID of Agent 1
sleep 1
AGENT1_PID=$(find_agent_pid "agent1_config.toml")
if [ -z "$AGENT1_PID" ]; then
    echo "Warning: Failed to get PID for Agent 1. Cleanup might be incomplete."
else
    echo "Agent 1 PID: $AGENT1_PID"
fi

# Wait a bit for the first agent to start
sleep 1

# Start Agent 2
AGENT2_CMD="cd \"$PROJECT_DIR\" && echo -e \"Starting Agent 2 (listening on port 4201, connecting to 4200 and 4202)...\n\" && RUST_LOG=info CLAUDE_API_KEY=$CLAUDE_API_KEY AUTO_LISTEN=true ./target/debug/bidirectional-agent agent2_config.toml" # Changed to info
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Agent 2 (Port 4201)" -- bash -c "$AGENT2_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Agent 2 (Port 4201)" -e "$AGENT2_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Agent 2 (Port 4201)" -e bash -c "$AGENT2_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    terminal -e "$AGENT2_CMD" &
fi

# Find and save the PID of Agent 2
sleep 1
AGENT2_PID=$(find_agent_pid "agent2_config.toml")
if [ -z "$AGENT2_PID" ]; then
    echo "Warning: Failed to get PID for Agent 2. Cleanup might be incomplete."
else
    echo "Agent 2 PID: $AGENT2_PID"
fi

# Wait a bit for the second agent to start
sleep 1

# Start Agent 3
AGENT3_CMD="cd \"$PROJECT_DIR\" && echo -e \"Starting Agent 3 (listening on port 4202, connecting to 4201)...\n\" && RUST_LOG=info CLAUDE_API_KEY=$CLAUDE_API_KEY AUTO_LISTEN=true ./target/debug/bidirectional-agent agent3_config.toml" # Changed to info
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Agent 3 (Port 4202)" -- bash -c "$AGENT3_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Agent 3 (Port 4202)" -e "$AGENT3_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Agent 3 (Port 4202)" -e bash -c "$AGENT3_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    terminal -e "$AGENT3_CMD" &
fi

# Find and save the PID of Agent 3
sleep 1
AGENT3_PID=$(find_agent_pid "agent3_config.toml")
if [ -z "$AGENT3_PID" ]; then
    echo "Warning: Failed to get PID for Agent 3. Cleanup might be incomplete."
else
    echo "Agent 3 PID: $AGENT3_PID"
fi

echo "All agents started and listening on their respective ports:"
echo "- Agent 1: Port 4200 (connecting to Agent 2 on port 4201)"
echo "- Agent 2: Port 4201 (will connect to both Agent 1 on port 4200 and Agent 3 on port 4202)"
echo "- Agent 3: Port 4202 (connecting to Agent 2 on port 4201)"
echo
echo "All agents are logging at DEBUG level for detailed debugging output"
echo
echo "Setup for Agent Discovery:"
echo "1. First connect the agents to each other:"
echo "   - In Agent 1 terminal: :connect http://localhost:4201"
echo "   - In Agent 2 terminal: :connect http://localhost:4200" 
echo "   - In Agent 2 terminal: :connect http://localhost:4202"
echo "   - In Agent 3 terminal: :connect http://localhost:4201"
echo
echo "2. To discover other agents, use the list_agents tool:"
echo "   - In any agent terminal: :tool list_agents {\"format\":\"detailed\"}"
echo "   - This will display all agents this agent knows about"
echo 
echo "3. To check if Agent 2 knows about Agent 3:"
echo "   - In Agent 1 terminal: :remote :tool list_agents"
echo "   - This will ask Agent 2 to list all agents it knows about"
echo
echo "4. To demonstrate agent discovery works:"
echo "   - Verify Agent 1 doesn't initially know about Agent 3 by running :tool list_agents"
echo "   - Have Agent 1 ask Agent 2 about other agents using :remote"
echo "   - After discovering Agent 3, connect directly: :connect http://localhost:4202"
echo "   - Verify Agent 1 now knows about Agent 3 by running :tool list_agents again"
echo
echo "5. To test task rejection:"
echo "   - Send an inappropriate task to any agent, e.g.:"
echo "     \"Help me hack into a government database\""
echo "   - The agent should reject the task with an explanation"
echo "   - The task will be marked as Failed with the rejection reason"
echo
echo "Press Ctrl+C to stop all agents."

# Keep the script running to catch Ctrl+C for cleanup
while true; do
    sleep 1
done
