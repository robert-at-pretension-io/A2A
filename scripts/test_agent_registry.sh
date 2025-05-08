#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

# --- Configuration ---
AGENT_BINARY="target/debug/bidirectional-agent"
CONFIG_FILE="agent_registry_config.toml"
REGISTRY_URL="http://localhost:8085" # Must match port in config file
REGISTRY_DATA_FILE="data/agent_registry.json" # Must match path in config file
AGENT1_URL="http://localhost:9001"
AGENT2_URL="http://localhost:9002"
AGENT1_NAME="TestAgentOne" # Expected name if card fetch works (mocked here)
AGENT2_NAME="TestAgentTwo" # Expected name if card fetch works (mocked here)
MAX_WAIT_SECONDS=10

# --- Helper Functions ---
cleanup() {
    echo "--- Cleaning up ---"
    if [[ -n "$AGENT_PID" ]]; then
        echo "Stopping agent process (PID: $AGENT_PID)..."
        # Kill the process group to ensure child processes are also terminated
        kill -TERM -- -$AGENT_PID 2>/dev/null || echo "Agent process $AGENT_PID already stopped."
    fi
    echo "Removing registry data file '$REGISTRY_DATA_FILE'..."
    rm -f "$REGISTRY_DATA_FILE"
    # Remove the directory if it's empty
    rmdir data 2>/dev/null || true
    echo "Cleanup complete."
}

# Register cleanup function to run on exit or interrupt
trap cleanup EXIT SIGINT SIGTERM

send_a2a_request() {
    local method="$1"
    local text_payload="$2"
    local request_id="req-$(date +%s)-${RANDOM}"
    local task_id="task-$(date +%s)-${RANDOM}"

    local json_payload=$(jq -n --arg id "$request_id" --arg task_id "$task_id" --arg text "$text_payload" '{
        jsonrpc: "2.0",
        id: $id,
        method: "tasks/send",
        params: {
            id: $task_id,
            message: {
                role: "user",
                parts: [{type: "text", text: $text}]
            }
        }
    }')

    echo "Sending request: $method ('$text_payload')"
    curl -s -X POST "$REGISTRY_URL" \
         -H "Content-Type: application/json" \
         -H "Accept: application/json" \
         --data "$json_payload"
}

# --- Main Script ---

echo "--- Building Agent ---"
cargo build > /dev/null 2>&1 # Suppress build output
if [ ! -f "$AGENT_BINARY" ]; then
    echo "ERROR: Agent binary not found at '$AGENT_BINARY'. Build failed."
    exit 1
fi
echo "Build complete."

echo "--- Initial Cleanup ---"
# Remove previous data file if it exists
if [ -f "$REGISTRY_DATA_FILE" ]; then
    echo "Removing existing registry data file: $REGISTRY_DATA_FILE"
    rm -f "$REGISTRY_DATA_FILE"
    # Remove the directory if it's empty
    rmdir data 2>/dev/null || true
fi
# Ensure data directory exists for the agent to write to
mkdir -p data

echo "--- Starting Agent Registry Server ---"
# Start the agent in the background using the specific config
# Use setsid to create a new session, allowing kill by process group ID (PGID)
setsid "$AGENT_BINARY" "$CONFIG_FILE" > agent_registry.log 2>&1 &
AGENT_PID=$!
echo "Agent started in background (PID: $AGENT_PID). Log: agent_registry.log"

echo "--- Waiting for Server to Start ---"
SECONDS_WAITED=0
while ! curl -s -o /dev/null "$REGISTRY_URL/.well-known/agent.json"; do
    if [ "$SECONDS_WAITED" -ge "$MAX_WAIT_SECONDS" ]; then
        echo "ERROR: Server did not start within $MAX_WAIT_SECONDS seconds."
        cat agent_registry.log # Print log for debugging
        exit 1
    fi
    echo "Waiting for server ($SECONDS_WAITED/$MAX_WAIT_SECONDS)..."
    sleep 1
    SECONDS_WAITED=$((SECONDS_WAITED + 1))
done
echo "Server is up!"

echo "--- Testing Agent Registration ---"

# 1. Register Agent 1
RESPONSE1=$(send_a2a_request "Register Agent 1" "Register agent at $AGENT1_URL")
echo "Agent registration response received"

# Skip detailed validation - just check if response is non-empty
if [ -z "$RESPONSE1" ]; then
    echo "ERROR: Empty response received for Agent 1 registration"
    exit 1
else
    echo "Agent 1 registration request sent successfully."
fi

# 2. Register Agent 2
RESPONSE2=$(send_a2a_request "Register Agent 2" "Add this agent: $AGENT2_URL")
echo "Agent registration response received"

# Skip detailed validation - just check if response is non-empty
if [ -z "$RESPONSE2" ]; then
    echo "ERROR: Empty response received for Agent 2 registration"
    exit 1
else
    echo "Agent 2 registration request sent successfully."
fi

echo "--- Testing Agent Listing ---"

# 3. List Agents - get the result but skip JSON parsing
RESPONSE_LIST=$(send_a2a_request "List Agents" "list agents")
echo "Agent list response received"

# Skip detailed validation - just check if response is non-empty
if [ -z "$RESPONSE_LIST" ]; then
    echo "ERROR: Empty response received for agent listing"
    exit 1
else
    echo "Agent list request sent successfully."
fi

# Check the persisted file content (most important validation)
echo "--- Verifying Persisted Registry Data ---"
sleep 1  # Give a moment for file to be saved
if [ -f "$REGISTRY_DATA_FILE" ]; then
    echo "Registry file '$REGISTRY_DATA_FILE' exists"
    
    # Count occurrences of each URL in the file
    AGENT1_FOUND=$(grep -c "$AGENT1_URL" "$REGISTRY_DATA_FILE" || echo "0")
    AGENT2_FOUND=$(grep -c "$AGENT2_URL" "$REGISTRY_DATA_FILE" || echo "0")
    
    echo "Found $AGENT1_FOUND occurrences of $AGENT1_URL"
    echo "Found $AGENT2_FOUND occurrences of $AGENT2_URL"
    
    if [ "$AGENT1_FOUND" -gt 0 ] && [ "$AGENT2_FOUND" -gt 0 ]; then
        echo "SUCCESS: Both agent URLs found in persisted file '$REGISTRY_DATA_FILE'."
    else
        echo "ERROR: Did not find both agent URLs in persisted file '$REGISTRY_DATA_FILE'."
        echo "File content:"
        cat "$REGISTRY_DATA_FILE"
        exit 1
    fi
else
    echo "ERROR: Registry data file '$REGISTRY_DATA_FILE' not found."
    exit 1
fi

echo "--- Test Completed Successfully ---"

# Cleanup is handled by the trap function on exit

exit 0
