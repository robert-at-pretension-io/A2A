#!/bin/bash

# Script to run two bidirectional agents connected to each other
# One listens on port 4200 and connects to 4201
# The other listens on port 4201 and connects to 4200

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

    if [ -n "$AGENT2_PID" ]; then
        echo "Stopping Agent 2 (PID: $AGENT2_PID)..."
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
        # Use absolute path for config file to be more specific if needed, but relative should work here
        # Use head -n 1 in case multiple matches are found (unlikely here)
        pid=$(pgrep -f "bidirectional-agent ${config_file}" | head -n 1)
        if [ -z "$pid" ]; then
            sleep 0.5
            attempts=$((attempts + 1))
        else
            # Optional: Verify the process command line more strictly if needed
            # local cmdline=$(ps -p $pid -o cmd=)
            # echo "Found potential PID $pid with cmdline: $cmdline" >&2 # Debug
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
    echo "Please either:"
    echo "1. Export your API key: export CLAUDE_API_KEY=your-api-key"
    echo "2. Set it inline: CLAUDE_API_KEY=your-api-key ./run_bidirectional.sh"
    echo "3. Or uncomment and set claude_api_key in bidirectional_agent.toml"
    exit 1
fi

# Check if gnome-terminal, xterm or konsole is available
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

echo "Starting bidirectional agents..."

# Create config file for Agent 1 (Port 4200)
cat > "${PROJECT_DIR}/agent1_config.toml" << EOF
[server]
port = 4200
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent-1"
agent_name = "Agent One" # Set the desired name

[client]
target_url = "http://localhost:4201"

[llm]
# API key set via environment variable
system_prompt = "You are an AI agent that can communicate with other agents."

[mode]
repl = true
get_agent_card = false # Explicitly add the field
repl_log_file = "repl_interactions.log" # Log REPL to this file
EOF

# Create config file for Agent 2 (Port 4201)
cat > "${PROJECT_DIR}/agent2_config.toml" << EOF
[server]
port = 4201
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent-2"
agent_name = "Agent Two" # Set the desired name

[client]
target_url = "http://localhost:4200"

[llm]
# API key set via environment variable
system_prompt = "You are an AI agent that can communicate with other agents."

[mode]
repl = true
get_agent_card = false # Explicitly add the field
repl_log_file = "repl_interactions.log" # Log REPL to the same file
EOF

# Start Agent 1 (listens on 4200, connects to 4201) - Run agent in foreground of terminal's shell
AGENT1_CMD="cd \"$PROJECT_DIR\" && echo -e 'Starting Agent 1 (PID \$$) listening on port 4200, connecting to 4201...\n' && RUST_LOG=info CLAUDE_API_KEY=$CLAUDE_API_KEY ./target/debug/bidirectional-agent agent1_config.toml --listen" # Added RUST_LOG=info
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Agent 1 (Port 4200)" -- bash -c "$AGENT1_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Agent 1 (Port 4200)" -e "$AGENT1_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Agent 1 (Port 4200)" -e bash -c "$AGENT1_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    # For macOS - Adjust if 'terminal -e' doesn't work as expected
    terminal -e "$AGENT1_CMD" &
fi

# Find and save the PID of the actual agent process
AGENT1_PID=$(find_agent_pid "agent1_config.toml")
if [ -z "$AGENT1_PID" ]; then
    echo "Warning: Failed to get PID for Agent 1. Cleanup might be incomplete."
else
    echo "Agent 1 PID: $AGENT1_PID"
fi

# Wait a bit for the first agent's server to potentially bind
sleep 1

# Start Agent 2 (listens on 4201, connects to 4200) - Run agent in foreground of terminal's shell
AGENT2_CMD="cd \"$PROJECT_DIR\" && echo -e 'Starting Agent 2 (PID \$$) listening on port 4201, connecting to 4200...\n' && RUST_LOG=info CLAUDE_API_KEY=$CLAUDE_API_KEY ./target/debug/bidirectional-agent agent2_config.toml --listen" # Added RUST_LOG=info
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Agent 2 (Port 4201)" -- bash -c "$AGENT2_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Agent 2 (Port 4201)" -e "$AGENT2_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Agent 2 (Port 4201)" -e bash -c "$AGENT2_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    # For macOS - Adjust if 'terminal -e' doesn't work as expected
    terminal -e "$AGENT2_CMD" &
fi

# Find and save the PID of the actual agent process
AGENT2_PID=$(find_agent_pid "agent2_config.toml")
if [ -z "$AGENT2_PID" ]; then
    echo "Warning: Failed to get PID for Agent 2. Cleanup might be incomplete."
else
    echo "Agent 2 PID: $AGENT2_PID"
fi

echo "Both agents started and listening on their respective ports:"
echo "- Agent 1: Port 4200 (and connecting to port 4201)"
echo "- Agent 2: Port 4201 (and connecting to port 4200)"
echo
echo "To complete the connections:"
echo "1. In Agent 1 terminal, run ':connect http://localhost:4201' to connect to Agent 2"
echo "2. In Agent 2 terminal, run ':connect http://localhost:4200' to connect to Agent 1"
echo "3. You can then send messages between agents using the ':remote <message>' command"
echo
echo "Press Ctrl+C to stop all agents."

# Keep the script running so we can catch Ctrl+C to clean up
while true; do
    sleep 1
done
