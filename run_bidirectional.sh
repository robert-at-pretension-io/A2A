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
    
    # Exit
    exit 0
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

# Build the agent if it doesn't exist
if [ ! -f "./target/debug/bidirectional-agent" ]; then
    echo "Building bidirectional agent..."
    RUSTFLAGS="-A warnings" cargo build --bin bidirectional-agent
fi

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

[client]
target_url = "http://localhost:4201"

[llm]
# API key set via environment variable
system_prompt = "You are an AI agent that can communicate with other agents."

[mode]
repl = true
EOF

# Create config file for Agent 2 (Port 4201)
cat > "${PROJECT_DIR}/agent2_config.toml" << EOF
[server]
port = 4201
bind_address = "0.0.0.0"
agent_id = "bidirectional-agent-2"

[client]
target_url = "http://localhost:4200"

[llm]
# API key set via environment variable
system_prompt = "You are an AI agent that can communicate with other agents."

[mode]
repl = true
EOF

# Start Agent 1 (listens on 4200, connects to 4201)
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Agent 1 (Port 4200)" -- bash -c "cd \"$PROJECT_DIR\" && echo -e 'Starting Agent 1 listening on port 4200 and connecting to port 4201...\n' && CLAUDE_API_KEY=$CLAUDE_API_KEY ./target/debug/bidirectional-agent agent1_config.toml --listen; read -p 'Press Enter to close...'" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Agent 1 (Port 4200)" -e "cd \"$PROJECT_DIR\" && echo -e 'Starting Agent 1 listening on port 4200 and connecting to port 4201...\n' && CLAUDE_API_KEY=$CLAUDE_API_KEY ./target/debug/bidirectional-agent agent1_config.toml --listen; read -p 'Press Enter to close...'" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Agent 1 (Port 4200)" -e bash -c "cd \"$PROJECT_DIR\" && echo -e 'Starting Agent 1 listening on port 4200 and connecting to port 4201...\n' && CLAUDE_API_KEY=$CLAUDE_API_KEY ./target/debug/bidirectional-agent agent1_config.toml --listen; read -p 'Press Enter to close...'" &
elif [ "$TERMINAL" = "terminal" ]; then
    # For macOS
    terminal -e "cd \"$PROJECT_DIR\" && echo -e 'Starting Agent 1 listening on port 4200 and connecting to port 4201...\n' && CLAUDE_API_KEY=$CLAUDE_API_KEY ./target/debug/bidirectional-agent agent1_config.toml --listen; read -p 'Press Enter to close...'" &
fi

# Save the PID of the first terminal
AGENT1_PID=$!

# Wait a bit for the first agent to start
sleep 2

# Start Agent 2 (listens on 4201, connects to 4200)
if [ "$TERMINAL" = "gnome-terminal" ]; then
    gnome-terminal --title="Agent 2 (Port 4201)" -- bash -c "cd \"$PROJECT_DIR\" && echo -e 'Starting Agent 2 listening on port 4201 and connecting to port 4200...\n' && CLAUDE_API_KEY=$CLAUDE_API_KEY ./target/debug/bidirectional-agent agent2_config.toml --listen; read -p 'Press Enter to close...'" &
elif [ "$TERMINAL" = "xterm" ]; then
    xterm -title "Agent 2 (Port 4201)" -e "cd \"$PROJECT_DIR\" && echo -e 'Starting Agent 2 listening on port 4201 and connecting to port 4200...\n' && CLAUDE_API_KEY=$CLAUDE_API_KEY ./target/debug/bidirectional-agent agent2_config.toml --listen; read -p 'Press Enter to close...'" &
elif [ "$TERMINAL" = "konsole" ]; then
    konsole --new-tab -p tabtitle="Agent 2 (Port 4201)" -e bash -c "cd \"$PROJECT_DIR\" && echo -e 'Starting Agent 2 listening on port 4201 and connecting to port 4200...\n' && CLAUDE_API_KEY=$CLAUDE_API_KEY ./target/debug/bidirectional-agent agent2_config.toml --listen; read -p 'Press Enter to close...'" &
elif [ "$TERMINAL" = "terminal" ]; then
    # For macOS
    terminal -e "cd \"$PROJECT_DIR\" && echo -e 'Starting Agent 2 listening on port 4201 and connecting to port 4200...\n' && CLAUDE_API_KEY=$CLAUDE_API_KEY ./target/debug/bidirectional-agent agent2_config.toml --listen; read -p 'Press Enter to close...'" &
fi

# Save the PID of the second terminal
AGENT2_PID=$!

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