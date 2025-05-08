#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

# --- Configuration ---
AGENT_BINARY="./target/debug/bidirectional-agent"
CONFIG_FILE="agent_registry_config.toml"
REGISTRY_URL="http://localhost:8085" # Must match port in AGENT_CONFIG_FILE
AGENT_DATA_FILE="data/agent_registry.json"
LOG_FILE="logs/agent_registry_test.log"
AGENT_PID=""

# --- Helper Functions ---
cleanup() {
    echo "Cleaning up..."
    if [ ! -z "$AGENT_PID" ]; then
        echo "Stopping agent server (PID: $AGENT_PID)..."
        kill "$AGENT_PID" || echo "Agent server was not running or already stopped."
        wait "$AGENT_PID" 2>/dev/null || true # Wait for process to terminate
    fi
    # Remove agent data file if it exists
    if [ -f "$AGENT_DATA_FILE" ]; then
        echo "Removing agent data file: $AGENT_DATA_FILE"
        rm "$AGENT_DATA_FILE"
    fi
    # Remove log file if it exists
    if [ -f "$LOG_FILE" ]; then
        echo "Removing log file: $LOG_FILE"
        rm "$LOG_FILE"
    fi
    echo "Cleanup complete."
}

# Trap EXIT signal to ensure cleanup runs
trap cleanup EXIT

send_a2a_request() {
    local message_text="$1"
    local request_id="test-req-$(date +%s%N)"
    local task_id="task-$(uuidgen | tr '[:upper:]' '[:lower:]')"

    curl -s -X POST "$REGISTRY_URL" \
        -H "Content-Type: application/json" \
        -H "Accept: application/json" \
        -d '{
            "jsonrpc": "2.0",
            "id": "'"$request_id"'",
            "method": "tasks/send",
            "params": {
                "id": "'"$task_id"'",
                "message": {
                    "role": "user",
                    "parts": [{"type": "text", "text": "'"$message_text"'"}]
                }
            }
        }'
}

check_response_contains() {
    local response="$1"
    local expected_text="$2"
    local description="$3"

    if echo "$response" | grep -q "$expected_text"; then
        echo "✅ PASSED: $description - Found '$expected_text' in response."
    else
        echo "❌ FAILED: $description - Did not find '$expected_text' in response."
        echo "Response was:"
        echo "$response"
        exit 1
    fi
}

check_response_ok() {
    local response="$1"
    local description="$2"

    if echo "$response" | jq -e '.result' > /dev/null; then
        echo "✅ PASSED: $description - Response has a result field."
    else
        echo "❌ FAILED: $description - Response does not have a result field or contains an error."
        echo "Response was:"
        echo "$response" | jq .
        exit 1
    fi
}


# --- Main Test Logic ---

echo "Building agent..."
cargo build --bin bidirectional-agent

# Initial cleanup (in case of previous failed run)
echo "Performing initial cleanup..."
if [ -f "$AGENT_DATA_FILE" ]; then
    rm "$AGENT_DATA_FILE"
fi
if [ -f "$LOG_FILE" ]; then
    rm "$LOG_FILE"
fi
# Ensure log directory exists for the agent
mkdir -p "$(dirname "$LOG_FILE")"
mkdir -p "$(dirname "$AGENT_DATA_FILE")"


echo "Starting agent server in background with config: $CONFIG_FILE..."
# Update config to use the test log file
sed -i.bak "s|repl_log_file = \".*\"|repl_log_file = \"$LOG_FILE\"|" "$CONFIG_FILE"

"$AGENT_BINARY" "$CONFIG_FILE" < /dev/null &
AGENT_PID=$!
echo "Agent server started with PID: $AGENT_PID"

echo "Waiting for server to be ready (max 30 seconds)..."
for i in {1..30}; do
    if curl -s "$REGISTRY_URL/.well-known/agent.json" > /dev/null; then
        echo "Server is up!"
        break
    fi
    if ! kill -0 "$AGENT_PID" 2>/dev/null; then
        echo "❌ FAILED: Agent server process died unexpectedly."
        cat "$LOG_FILE" # Print agent logs
        exit 1
    fi
    sleep 1
    if [ "$i" -eq 30 ]; then
        echo "❌ FAILED: Server did not start within 30 seconds."
        cat "$LOG_FILE" # Print agent logs
        exit 1
    fi
done

# --- Test Cases ---

echo "--- Test Case 1: Register first agent ---"
AGENT1_URL="http://test-agent-1.example.com:8001"
RESPONSE1=$(send_a2a_request "Register my agent at $AGENT1_URL")
echo "Response 1: $RESPONSE1" | jq .
check_response_ok "$RESPONSE1" "Register Agent 1"
check_response_contains "$RESPONSE1" "Registered agent URL: $AGENT1_URL" "Register Agent 1 confirmation"

echo "--- Test Case 2: List agents and verify first agent ---"
RESPONSE2=$(send_a2a_request "list agents")
echo "Response 2: $RESPONSE2" | jq .
check_response_ok "$RESPONSE2" "List Agents after Agent 1"
check_response_contains "$RESPONSE2" "$AGENT1_URL" "List Agents - Agent 1 URL"
# The RegistryRouter might fetch the card and use its name. For now, check URL.

echo "--- Test Case 3: Register second agent ---"
AGENT2_URL="http://another-agent.internal:9090"
RESPONSE3=$(send_a2a_request "Please add this agent: $AGENT2_URL to your directory.")
echo "Response 3: $RESPONSE3" | jq .
check_response_ok "$RESPONSE3" "Register Agent 2"
check_response_contains "$RESPONSE3" "Registered agent URL: $AGENT2_URL" "Register Agent 2 confirmation"

echo "--- Test Case 4: List agents and verify both agents ---"
RESPONSE4=$(send_a2a_request "show all agents") # Using different phrasing
echo "Response 4: $RESPONSE4" | jq .
check_response_ok "$RESPONSE4" "List Agents after Agent 2"
check_response_contains "$RESPONSE4" "$AGENT1_URL" "List Agents - Agent 1 URL still present"
check_response_contains "$RESPONSE4" "$AGENT2_URL" "List Agents - Agent 2 URL present"

# --- Test Case 5: Register an agent that might fail to provide a card (for robustness) ---
AGENT3_URL="http://nonexistent-agent-for-test.com:1234"
echo "Attempting to register an agent that will likely fail card retrieval: $AGENT3_URL"
RESPONSE5=$(send_a2a_request "Add $AGENT3_URL")
echo "Response 5: $RESPONSE5" | jq .
check_response_ok "$RESPONSE5" "Register Agent 3 (non-existent)"
check_response_contains "$RESPONSE5" "Registered agent URL: $AGENT3_URL" "Register Agent 3 confirmation (URL only)"
# We expect the URL to be registered even if card retrieval fails.
# The response might indicate failure to retrieve card, but registration of URL should succeed.

echo "--- Test Case 6: List agents and verify all three (Agent 3 as URL only) ---"
RESPONSE6=$(send_a2a_request "list agents")
echo "Response 6: $RESPONSE6" | jq .
check_response_ok "$RESPONSE6" "List Agents after Agent 3"
check_response_contains "$RESPONSE6" "$AGENT1_URL" "List Agents - Agent 1 URL still present"
check_response_contains "$RESPONSE6" "$AGENT2_URL" "List Agents - Agent 2 URL still present"
check_response_contains "$RESPONSE6" "$AGENT3_URL" "List Agents - Agent 3 URL present"


# Restore original config file
mv "${CONFIG_FILE}.bak" "$CONFIG_FILE"

echo "All tests passed!"
exit 0
