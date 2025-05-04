#!/bin/bash

# Script to demonstrate the 'remember_agent' tool
# Agent Remember (port 4300) will be instructed to remember Agent Target (port 4301)

# Function to handle cleanup on exit
cleanup() {
    echo "Cleaning up and stopping all agents..."
    
    # Kill any background processes we started
    if [ -n "$AGENT_REMEMBER_PID" ]; then
        kill $AGENT_REMEMBER_PID 2>/dev/null || true
    fi
    
    if [ -n "$AGENT_TARGET_PID" ]; then
        kill $AGENT_TARGET_PID 2>/dev/null || true
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

# Check if API key is provided (needed for LLM tool if enabled, good practice)
if [ -z "$CLAUDE_API_KEY" ]; then
    echo "CLAUDE_API_KEY environment variable is not set."
    echo "Please set it before running this script, even if only using remember_agent."
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

echo "Starting bidirectional agents for 'remember_agent' demonstration..."

# Create config file for Agent Remember (Port 4300)
cat > "${PROJECT_DIR}/agent_remember_config.toml" << EOF
[server]
port = 4300
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent-remember"
agent_name = "Agent Remember"

[client]
# No initial target URL

[llm]
# API key set via environment variable
system_prompt = "You are an AI agent that can remember other agents using the 'remember_agent' tool."

[mode]
repl = true
repl_log_file = "remember_agent_demo.log"

[tools]
# Enable remember_agent and list_agents
enabled = ["echo", "list_agents", "remember_agent", "llm", "execute_command"] # <-- Add execute_command
# REMOVED agent_directory_path
EOF

# Create config file for Agent Target (Port 4301)
cat > "${PROJECT_DIR}/agent_target_config.toml" << EOF
[server]
port = 4301
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent-target"
agent_name = "Agent Target"

[client]
# No client needed for this agent

[llm]
# API key set via environment variable
system_prompt = "You are a simple target agent."

[mode]
repl = true # Keep REPL for observation if needed
repl_log_file = "remember_agent_demo.log"

[tools]
enabled = ["echo", "llm", "execute_command"] # Only basic tools needed # <-- Add execute_command
# REMOVED agent_directory_path
EOF

# Start Agent Target (listens on 4301)
AGENT_TARGET_CMD="cd \"$PROJECT_DIR\" && echo -e \"Starting Agent Target (listening on port 4301)...\n\" && RUST_LOG=info CLAUDE_API_KEY=$CLAUDE_API_KEY AUTO_LISTEN=true ./target/debug/bidirectional-agent agent_target_config.toml" # Keep RUST_LOG=info
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Agent Target (Port 4301)" -- bash -c "$AGENT_TARGET_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Agent Target (Port 4301)" -e "$AGENT_TARGET_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Agent Target (Port 4301)" -e bash -c "$AGENT_TARGET_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    terminal -e "$AGENT_TARGET_CMD" &
fi

# Find and save the PID of Agent Target
sleep 2
AGENT_TARGET_PID=$(find_agent_pid "agent_target_config.toml")
if [ -z "$AGENT_TARGET_PID" ]; then
    echo "Warning: Failed to get PID for Agent Target. Cleanup might be incomplete."
else
    echo "Agent Target PID: $AGENT_TARGET_PID"
fi

# Start Agent Remember (listens on 4300)
AGENT_REMEMBER_CMD="cd \"$PROJECT_DIR\" && echo -e \"Starting Agent Remember (listening on port 4300)...\n\" && RUST_LOG=info CLAUDE_API_KEY=$CLAUDE_API_KEY AUTO_LISTEN=true ./target/debug/bidirectional-agent agent_remember_config.toml" # Keep RUST_LOG=info
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Agent Remember (Port 4300)" -- bash -c "$AGENT_REMEMBER_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Agent Remember (Port 4300)" -e "$AGENT_REMEMBER_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Agent Remember (Port 4300)" -e bash -c "$AGENT_REMEMBER_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    terminal -e "$AGENT_REMEMBER_CMD" &
fi

# Find and save the PID of Agent Remember
sleep 2
AGENT_REMEMBER_PID=$(find_agent_pid "agent_remember_config.toml")
if [ -z "$AGENT_REMEMBER_PID" ]; then
    echo "Warning: Failed to get PID for Agent Remember. Cleanup might be incomplete."
else
    echo "Agent Remember PID: $AGENT_REMEMBER_PID"
fi

echo "All agents started and listening on their respective ports."
echo
echo "=== REMEMBER AGENT DEMO INSTRUCTIONS ==="
echo
echo "1. Verify Agent Remember (Port 4300) doesn't know Agent Target yet:"
echo "   - In Agent Remember terminal: :tool list_agents"
echo "   - You should see an empty list or only the agent itself."
echo
echo "2. Tell Agent Remember to discover and store Agent Target (Port 4301):"
echo "   - In Agent Remember terminal: :tool remember_agent {\"agent_base_url\":\"http://localhost:4301\"}"
echo "   - Agent Remember will connect to Agent Target, fetch its card, and store it."
echo "   - You should see a success message."
echo
echo "3. Verify Agent Remember now knows Agent Target:"
echo "   - In Agent Remember terminal: :tool list_agents {\"format\":\"detailed\"}"
echo "   - You should now see 'Agent Target' listed."
echo
echo "4. (Optional) Try remembering an invalid URL:"
echo "   - In Agent Remember terminal: :tool remember_agent {\"agent_base_url\":\"http://invalid-url:9999\"}"
echo "   - This should fail and return an error message."
echo
echo "Press Ctrl+C to stop all agents."

# Keep the script running to catch Ctrl+C for cleanup
while true; do
    sleep 1
done
