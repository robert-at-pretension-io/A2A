#!/bin/bash

# Script to demonstrate agent delegation capabilities
# Agent 1 (port 4200) -> Agent 2 (port 4201) -> Agent 3 (port 4202)
# Agent 1 doesn't initially know about Agent 3
# Requests flow from Agent 1 -> Agent 2 -> Agent 3 and responses flow back

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
find_agent_pid() {
    local config_file="$1"
    local pid=""
    local attempts=0
    local max_attempts=10 # Try for 5 seconds (10 * 0.5s)

    echo "Searching for PID using config: ${config_file}..." >&2

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

echo "Starting bidirectional agents for delegation demonstration..."

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
system_prompt = "You are a helpful assistant agent that relies on other agents to accomplish tasks. When you need data analysis, ask Agent Two for help."

[mode]
repl = true
get_agent_card = false
repl_log_file = "delegation_interactions.log"

[tools]
enabled = ["echo", "llm", "list_agents"]
agent_directory_path = "./data/agent1_directory.json"
EOF

cat > "${PROJECT_DIR}/agent2_config.toml" << EOF
[server]
port = 4201
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent-2"
agent_name = "Agent Two"

[client]
target_url = "http://localhost:4202"

[llm]
# API key set via environment variable
system_prompt = "You are a router agent that doesn't have specialized skills yourself. When Agent One asks for data analysis, you should delegate the task to Agent Three who is an expert in data analysis. Always relay Agent Three's responses back to Agent One."

[mode]
repl = true
get_agent_card = false
repl_log_file = "delegation_interactions.log"

[tools]
enabled = ["echo", "llm", "list_agents"]
agent_directory_path = "./data/agent2_directory.json"
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
system_prompt = "You are an expert data analysis agent. When you receive tasks from Agent Two, perform the analysis and return detailed results. Always mention that you are Agent Three in your responses."

[mode]
repl = true
get_agent_card = false
repl_log_file = "delegation_interactions.log"

[tools]
enabled = ["echo", "summarize", "list_agents"]
agent_directory_path = "./data/agent3_directory.json"
EOF

# Start Agent 3 (expert)
AGENT3_CMD="cd \"$PROJECT_DIR\" && echo -e \"Starting Agent 3 (Expert Data Analyst, port 4202)...\n\" && RUST_LOG=info CLAUDE_API_KEY=$CLAUDE_API_KEY AUTO_LISTEN=true ./target/debug/bidirectional-agent agent3_config.toml"
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Agent 3 (Expert - Port 4202)" -- bash -c "$AGENT3_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Agent 3 (Expert - Port 4202)" -e "$AGENT3_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Agent 3 (Expert - Port 4202)" -e bash -c "$AGENT3_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    terminal -e "$AGENT3_CMD" &
fi

# Find and save the PID of Agent 3
sleep 2
AGENT3_PID=$(find_agent_pid "agent3_config.toml")
if [ -z "$AGENT3_PID" ]; then
    echo "Warning: Failed to get PID for Agent 3. Cleanup might be incomplete."
else
    echo "Agent 3 PID: $AGENT3_PID"
fi

# Start Agent 2 (router)
AGENT2_CMD="cd \"$PROJECT_DIR\" && echo -e \"Starting Agent 2 (Router, port 4201)...\n\" && RUST_LOG=info CLAUDE_API_KEY=$CLAUDE_API_KEY AUTO_LISTEN=true ./target/debug/bidirectional-agent agent2_config.toml"
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Agent 2 (Router - Port 4201)" -- bash -c "$AGENT2_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Agent 2 (Router - Port 4201)" -e "$AGENT2_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Agent 2 (Router - Port 4201)" -e bash -c "$AGENT2_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    terminal -e "$AGENT2_CMD" &
fi

# Find and save the PID of Agent 2
sleep 2
AGENT2_PID=$(find_agent_pid "agent2_config.toml")
if [ -z "$AGENT2_PID" ]; then
    echo "Warning: Failed to get PID for Agent 2. Cleanup might be incomplete."
else
    echo "Agent 2 PID: $AGENT2_PID"
fi

# Start Agent 1 (client)
AGENT1_CMD="cd \"$PROJECT_DIR\" && echo -e \"Starting Agent 1 (Client, port 4200)...\n\" && RUST_LOG=info CLAUDE_API_KEY=$CLAUDE_API_KEY AUTO_LISTEN=true ./target/debug/bidirectional-agent agent1_config.toml"
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Agent 1 (Client - Port 4200)" -- bash -c "$AGENT1_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Agent 1 (Client - Port 4200)" -e "$AGENT1_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Agent 1 (Client - Port 4200)" -e bash -c "$AGENT1_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    terminal -e "$AGENT1_CMD" &
fi

# Find and save the PID of Agent 1
sleep 2
AGENT1_PID=$(find_agent_pid "agent1_config.toml")
if [ -z "$AGENT1_PID" ]; then
    echo "Warning: Failed to get PID for Agent 1. Cleanup might be incomplete."
else
    echo "Agent 1 PID: $AGENT1_PID"
fi

echo "All agents started and listening on their respective ports."
echo
echo "=== DELEGATION SETUP INSTRUCTIONS ==="
echo
echo "1. First connect the agents to each other:"
echo "   - In Agent 1 terminal: :connect http://localhost:4201"
echo "   - In Agent 2 terminal: :connect http://localhost:4202"
echo "   - In Agent 3 terminal: :connect http://localhost:4201"
echo
echo "2. Verify Agent 1 only knows about Agent 2:"
echo "   - In Agent 1 terminal: :tool list_agents {\"format\":\"detailed\"}"
echo
echo "3. Verify Agent 2 knows about both Agent 1 and Agent 3:"
echo "   - In Agent 2 terminal: :tool list_agents {\"format\":\"detailed\"}"
echo
echo "4. To demonstrate delegation, have Agent 1 ask for data analysis:"
echo "   - In Agent 1 terminal: :remote Can you help me analyze this sample dataset of customer purchases?"
echo "   - Watch as Agent 2 delegates this to Agent 3"
echo "   - The response will flow back through Agent 2 to Agent 1"
echo
echo "5. For more complex delegation examples:"
echo "   - Try: :remote Can you analyze the trend in these numbers: 10, 15, 22, 31, 42?"
echo "   - Try: :remote What insights can you provide about this quarterly sales data: Q1:120K, Q2:135K, Q3:98K, Q4:142K?"
echo
echo "Press Ctrl+C to stop all agents."

# Keep the script running to catch Ctrl+C for cleanup
while true; do
    sleep 1
done