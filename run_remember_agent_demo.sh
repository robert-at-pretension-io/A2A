#!/bin/bash

# Script to demonstrate the 'remember_agent' tool with descriptive agent names
# Hub (port 4300) will serve as the central hub that connects to and remembers other agents
# Seeker (port 4301) will ask to be remembered by Hub
# Explorer (port 4302) will ask Hub about all agents it knows

# Function to handle cleanup on exit
cleanup() {
    echo "Cleaning up and stopping all agents..."
    
    # Kill any background processes we started
    if [ -n "$HUB_PID" ]; then
        kill $HUB_PID 2>/dev/null || true
    fi
    
    if [ -n "$SEEKER_PID" ]; then
        kill $SEEKER_PID 2>/dev/null || true
    fi
    
    if [ -n "$EXPLORER_PID" ]; then
        kill $EXPLORER_PID 2>/dev/null || true
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

echo "Starting bidirectional agents for 'remember_agent' demonstration..."

# Create config file for Hub Agent (Port 4300)
cat > "${PROJECT_DIR}/agent_remember_config.toml" << EOF
[server]
port = 4300
bind_address = "0.0.0.0"
agent_id = "hub"
agent_name = "Central Hub"

[client]
# No initial target URL

[llm]
# API key set via environment variable
system_prompt = "You are a central registry agent that maintains information about other agents in the network. Your primary function is to respond to agent registration requests and provide information about connected agents when asked. Act as the single source of truth for network connectivity."

[mode]
repl = true
repl_log_file = "remember_agent_demo.log"

[tools]
# Enable all available tools for the hub
enabled = ["echo", "list_agents", "remember_agent", "llm", "execute_command", "summarize", "human_input"]
EOF

# Create config file for Seeker Agent (Port 4301)
cat > "${PROJECT_DIR}/agent_target_config.toml" << EOF
[server]
port = 4301
bind_address = "0.0.0.0"
agent_id = "seeker"
agent_name = "Knowledge Seeker"


[llm]
# API key set via environment variable
system_prompt = "You are an agent that initiates connections with new services. Your primary role is to discover and register with other agents in the network, particularly central registry agents. You request to be remembered by registry hubs and establish new connections."

[mode]
repl = true
repl_log_file = "remember_agent_demo.log"

[tools]
# Enable all tools for the seeker as well
enabled = ["echo", "list_agents", "remember_agent", "llm", "execute_command", "summarize", "human_input"]
EOF

# Create config file for Explorer Agent (Port 4302)
cat > "${PROJECT_DIR}/agent_gamma_config.toml" << EOF
[server]
port = 4302
bind_address = "0.0.0.0"
agent_id = "explorer"
agent_name = "Network Explorer"

[llm]
# API key set via environment variable
system_prompt = "You are an agent that maps the network of connected services. Your primary function is to discover information about the agent network by querying central registries about their known connections and building a comprehensive map of available services."

[mode]
repl = true
repl_log_file = "remember_agent_demo.log"

[tools]
# Enable all tools for the explorer
enabled = ["echo", "list_agents", "remember_agent", "llm", "execute_command", "summarize", "human_input"]
EOF

# Start Knowledge Seeker Agent (listens on 4301)
SEEKER_CMD="cd \"$PROJECT_DIR\" && echo -e \"Starting Knowledge Seeker Agent (listening on port 4301)...\n\" && RUST_LOG=info CLAUDE_API_KEY=$CLAUDE_API_KEY GEMINI_API_KEY=$GEMINI_API_KEY ./target/debug/bidirectional-agent agent_target_config.toml"
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Knowledge Seeker (Port 4301)" -- bash -c "$SEEKER_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Knowledge Seeker (Port 4301)" -e "$SEEKER_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Knowledge Seeker (Port 4301)" -e bash -c "$SEEKER_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    terminal -e "$SEEKER_CMD" &
fi

# Start Network Explorer Agent (listens on 4302)
EXPLORER_CMD="cd \"$PROJECT_DIR\" && echo -e \"Starting Network Explorer Agent (listening on port 4302)...\n\" && RUST_LOG=info CLAUDE_API_KEY=$CLAUDE_API_KEY GEMINI_API_KEY=$GEMINI_API_KEY ./target/debug/bidirectional-agent agent_gamma_config.toml"
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Network Explorer (Port 4302)" -- bash -c "$EXPLORER_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Network Explorer (Port 4302)" -e "$EXPLORER_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Network Explorer (Port 4302)" -e bash -c "$EXPLORER_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    terminal -e "$EXPLORER_CMD" &
fi

# Find and save the PIDs of Seeker and Explorer
sleep 2
SEEKER_PID=$(find_agent_pid "agent_target_config.toml")
if [ -z "$SEEKER_PID" ]; then
    echo "Warning: Failed to get PID for Knowledge Seeker Agent. Cleanup might be incomplete."
else
    echo "Knowledge Seeker Agent PID: $SEEKER_PID"
fi

EXPLORER_PID=$(find_agent_pid "agent_gamma_config.toml")
if [ -z "$EXPLORER_PID" ]; then
    echo "Warning: Failed to get PID for Network Explorer Agent. Cleanup might be incomplete."
else
    echo "Network Explorer Agent PID: $EXPLORER_PID"
fi

# Start Central Hub Agent (listens on 4300)
HUB_CMD="cd \"$PROJECT_DIR\" && echo -e \"Starting Central Hub Agent (listening on port 4300)...\n\" && RUST_LOG=info CLAUDE_API_KEY=$CLAUDE_API_KEY GEMINI_API_KEY=$GEMINI_API_KEY ./target/debug/bidirectional-agent agent_remember_config.toml"
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Central Hub (Port 4300)" -- bash -c "$HUB_CMD" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Central Hub (Port 4300)" -e "$HUB_CMD" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Central Hub (Port 4300)" -e bash -c "$HUB_CMD" &
elif [ "$TERMINAL" = "terminal" ]; then
    terminal -e "$HUB_CMD" &
fi

# Find and save the PID of Central Hub
sleep 2
HUB_PID=$(find_agent_pid "agent_remember_config.toml")
if [ -z "$HUB_PID" ]; then
    echo "Warning: Failed to get PID for Central Hub Agent. Cleanup might be incomplete."
else
    echo "Central Hub Agent PID: $HUB_PID"
fi

echo "All agents started and listening on their respective ports."
echo
echo "=== AGENT NETWORK DEMO INSTRUCTIONS ==="
echo
echo "1. Verify the Central Hub (Port 4300) doesn't know other agents yet:"
echo "   - In Central Hub terminal: :tool list_agents"
echo "   - You should see an empty list or only the Hub itself."
echo
echo "2. In the Knowledge Seeker terminal (Port 4301), request to be remembered by the Hub:"
echo "   - Type: :listen"
echo "   - Then type: :connect http://localhost:4300"
echo "   - Type: please tell the Central Hub to remember me at http://localhost:4301"
echo "   - The Seeker will send this request to the Hub."
echo
echo "3. In the Central Hub terminal, you should see the request. Respond with:"
echo "   - Type: :listen"
echo "   - Type: I'll remember you"
echo "   - The Hub should understand from context and remember the Seeker URL."
echo
echo "4. In the Network Explorer terminal (Port 4302), ask the Hub about known agents:"
echo "   - Type: :listen"
echo "   - Type: :connect http://localhost:4300"
echo "   - Then type: what agents do you know about?"
echo "   - The Hub should respond with information about the Seeker agent."
echo
echo "5. Test the pronoun resolution in the Hub:"
echo "   - In the Hub terminal, if Explorer (4302) asks to be remembered, just type:"
echo "   - Type: remember them"
echo "   - The Hub should correctly interpret 'them' as referring to the Explorer."
echo
echo "6. Verify the Hub now knows both Seeker and Explorer:"
echo "   - In the Hub terminal: :tool list_agents {\"format\":\"detailed\"}"
echo "   - You should see both the Seeker and Explorer listed."
echo
echo "Press Ctrl+C to stop all agents."

# Keep the script running to catch Ctrl+C for cleanup
while true; do
    sleep 1
done