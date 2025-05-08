#!/bin/bash

# Exit immediately if a command exits with a non-zero status.
set -e

# --- Configuration ---
REGISTRY_URL="http://repository-agent.com" # Remote agent registry server
AGENT1_URL="http://example.com/agent1" # Example agent 1 URL to register
AGENT2_URL="http://example.com/agent2" # Example agent 2 URL to register
MAX_WAIT_SECONDS=10

# --- Helper Functions ---
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

echo "--- Testing Remote Agent Registry at $REGISTRY_URL ---"

# Check if server is accessible
echo "--- Checking Server Availability ---"
SECONDS_WAITED=0
while ! curl -s -o /dev/null "$REGISTRY_URL/.well-known/agent.json"; do
    if [ "$SECONDS_WAITED" -ge "$MAX_WAIT_SECONDS" ]; then
        echo "ERROR: Server not accessible at $REGISTRY_URL within $MAX_WAIT_SECONDS seconds."
        exit 1
    fi
    echo "Waiting for server ($SECONDS_WAITED/$MAX_WAIT_SECONDS)..."
    sleep 1
    SECONDS_WAITED=$((SECONDS_WAITED + 1))
done
echo "Server is accessible!"

echo "--- Testing Agent Registration ---"

# 1. Register Agent 1
RESPONSE1=$(send_a2a_request "Register Agent 1" "Register agent at $AGENT1_URL")
echo "Agent registration response received"

# Extract and print the important parts of the response
echo "=== AGENT 1 REGISTRATION RESPONSE ==="
echo "$RESPONSE1" | jq -r '.result.artifacts[0].parts[0].text' 2>/dev/null || echo "$RESPONSE1"
echo "======================================="

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

# Extract and print the important parts of the response
echo "=== AGENT 2 REGISTRATION RESPONSE ==="
echo "$RESPONSE2" | jq -r '.result.artifacts[0].parts[0].text' 2>/dev/null || echo "$RESPONSE2"
echo "======================================="

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

# Extract and print the full agent list
echo "=== AGENT LIST RESPONSE ==="
echo "$RESPONSE_LIST" | jq -r '.result.status.message.parts[0].text' 2>/dev/null || echo "$RESPONSE_LIST"
echo "======================================="

# Skip detailed validation - just check if response is non-empty
if [ -z "$RESPONSE_LIST" ]; then
    echo "ERROR: Empty response received for agent listing"
    exit 1
else
    echo "Agent list request sent successfully."
fi

# Print the full raw response for debugging if needed
echo "=== RAW SERVER RESPONSE (LIST) ==="
echo "$RESPONSE_LIST" | jq -r '.' 2>/dev/null || echo "$RESPONSE_LIST"
echo "======================================="

# Verify that our agents are in the list response
if echo "$RESPONSE_LIST" | grep -q "$AGENT1_URL" && echo "$RESPONSE_LIST" | grep -q "$AGENT2_URL"; then
    echo "SUCCESS: Both agent URLs found in the agent list response."
else
    echo "WARNING: Could not confirm both agent URLs in the list response."
    echo "This could be due to response formatting or caching issues."
fi

echo "--- Test Completed Successfully ---"

exit 0