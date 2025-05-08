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
cargo build
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
echo "Response 1: $RESPONSE1"
# Basic check: Ensure it's a valid JSON-RPC response with a result and contains the URL
echo "$RESPONSE1" | jq -e '.result.id' > /dev/null
echo "$RESPONSE1" | jq -e '.result.status.message.parts[0].text | contains("'$AGENT1_URL'")' > /dev/null
echo "Agent 1 registration request sent successfully (URL found in response)."

# 2. Register Agent 2
RESPONSE2=$(send_a2a_request "Register Agent 2" "Add this agent: $AGENT2_URL")
echo "Response 2: $RESPONSE2"
echo "$RESPONSE2" | jq -e '.result.id' > /dev/null
echo "$RESPONSE2" | jq -e '.result.status.message.parts[0].text | contains("'$AGENT2_URL'")' > /dev/null
echo "Agent 2 registration request sent successfully (URL found in response)."

echo "--- Testing Agent Listing ---"

# 3. List Agents
RESPONSE_LIST=$(send_a2a_request "List Agents" "list agents")
echo "Response List: $RESPONSE_LIST"
echo "$RESPONSE_LIST" | jq -e '.result.id' > /dev/null

# Check if both agent URLs are in the response text
LIST_TEXT=$(echo "$RESPONSE_LIST" | jq -r '.result.status.message.parts[0].text')
echo "List Text: $LIST_TEXT"

if echo "$LIST_TEXT" | grep -q "$AGENT1_URL" && echo "$LIST_TEXT" | grep -q "$AGENT2_URL"; then
    echo "SUCCESS: Both agent URLs found in the list."
else
    echo "ERROR: Did not find both agent URLs ($AGENT1_URL, $AGENT2_URL) in the list response."
    exit 1
fi

# Check the persisted file content (optional but good verification)
echo "--- Verifying Persisted Registry Data ---"
if [ -f "$REGISTRY_DATA_FILE" ]; then
    echo "Registry file '$REGISTRY_DATA_FILE' content:"
    cat "$REGISTRY_DATA_FILE"
    if jq -e '.[] | select(.url=="'$AGENT1_URL'")' "$REGISTRY_DATA_FILE" > /dev/null && \
       jq -e '.[] | select(.url=="'$AGENT2_URL'")' "$REGISTRY_DATA_FILE" > /dev/null; then
        echo "SUCCESS: Both agent URLs found in persisted file '$REGISTRY_DATA_FILE'."
    else
        echo "ERROR: Did not find both agent URLs in persisted file '$REGISTRY_DATA_FILE'."
        exit 1
    fi
else
    echo "ERROR: Registry data file '$REGISTRY_DATA_FILE' not found."
    exit 1
fi


echo "--- Test Completed Successfully ---"

# Cleanup is handled by the trap function on exit

exit 0
