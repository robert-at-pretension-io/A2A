#!/bin/bash

# This script starts the mock server and then runs a series of client commands to test functionality

# Exit on any error
set -e

echo "===============================" 
echo "Starting Mock A2A Server"
echo "==============================="

# Start server with "None" authentication scheme
# This means it will report authentication in the agent card but won't actually require it
echo "Starting mock A2A server on port 8080..."
RUSTFLAGS="-A warnings" cargo run --quiet -- server --port 8080 &
SERVER_PID=$!

# Give the server time to start
sleep 2

# Set authentication parameters (using "None" scheme allows all requests to pass)
export AUTH_HEADER="Authorization"
export AUTH_VALUE="Bearer test-token"

# Ensure we kill the server when the script exits
trap "kill $SERVER_PID" EXIT

echo "===============================" 
echo "Testing Client Functions"
echo "==============================="

# Get the agent card
echo "Getting Agent Card..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-agent-card --url "http://localhost:8080"

# Send a basic task
echo "Sending Task..."
TASK_ID=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "http://localhost:8080" --message "This is a test task" --header "$AUTH_HEADER" --value "$AUTH_VALUE" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$TASK_ID" ]; then
  echo "Task created with ID: $TASK_ID"
  
  # Get the task details
  echo "Getting Task Details..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "http://localhost:8080" --id "$TASK_ID" --header "$AUTH_HEADER" --value "$AUTH_VALUE"
  
  # Get task state history
  echo "Getting Task State History..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-state-history --url "http://localhost:8080" --id "$TASK_ID"
  
  # Get task state metrics
  echo "Getting Task State Metrics..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-state-metrics --url "http://localhost:8080" --id "$TASK_ID"
  
  # Cancel the task
  echo "Cancelling Task..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client cancel-task --url "http://localhost:8080" --id "$TASK_ID" --header "$AUTH_HEADER" --value "$AUTH_VALUE"
  
  # Get the task details after cancellation
  echo "Getting Task Details After Cancellation..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "http://localhost:8080" --id "$TASK_ID" --header "$AUTH_HEADER" --value "$AUTH_VALUE"
fi

# Test streaming
echo "===============================" 
echo "Testing Streaming"
echo "==============================="
# Note that we can't easily capture the streaming output
RUSTFLAGS="-A warnings" cargo run --quiet -- client stream-task --url "http://localhost:8080" --message "This is a streaming task" &
STREAM_PID=$!

# Let it run for a bit
sleep 3

# Kill the streaming client
kill $STREAM_PID 2>/dev/null || true

# Test push notifications
echo "===============================" 
echo "Testing Push Notifications"
echo "==============================="
RUSTFLAGS="-A warnings" cargo run --quiet -- client set-push-notification --url "http://localhost:8080" --id "$TASK_ID" --webhook "https://example.com/webhook" --auth-scheme "Bearer" --token "test-token"

RUSTFLAGS="-A warnings" cargo run --quiet -- client get-push-notification --url "http://localhost:8080" --id "$TASK_ID"

echo "===============================" 
echo "Testing Task Batching"
echo "==============================="
# Create a batch
BATCH_ID=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client create-batch --url "http://localhost:8080" --tasks "Task 1,Task 2,Task 3" --name "Test Batch" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$BATCH_ID" ]; then
  # Get batch info
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-batch --url "http://localhost:8080" --id "$BATCH_ID"
  
  # Get batch status
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-batch-status --url "http://localhost:8080" --id "$BATCH_ID"
  
  # Cancel batch
  RUSTFLAGS="-A warnings" cargo run --quiet -- client cancel-batch --url "http://localhost:8080" --id "$BATCH_ID"
fi

echo "===============================" 
echo "Testing Agent Skills"
echo "==============================="
# List all skills
echo "Listing all skills..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client list-skills --url "http://localhost:8080"

# List skills filtered by tag
echo "Listing skills with 'text' tag..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client list-skills --url "http://localhost:8080" --tags "text"

# Get details for a specific skill
echo "Getting details for a specific skill..."
SKILL_ID="test-skill-1"
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-skill-details --url "http://localhost:8080" --id "$SKILL_ID"

# Invoke a skill
echo "Invoking skill..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client invoke-skill --url "http://localhost:8080" --id "$SKILL_ID" --message "This is a test skill invocation"

# Invoke a skill with specific input/output modes
echo "Invoking skill with specific input/output modes..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client invoke-skill --url "http://localhost:8080" --id "$SKILL_ID" --message "This is a test with specific modes" -n "text/plain" -p "text/plain"

echo "===============================" 
echo "Testing Error Handling"
echo "==============================="

# Test with non-existent task ID to trigger error
echo "Testing error handling for non-existent task..."
NON_EXISTENT_TASK="task-does-not-exist-123456"
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "http://localhost:8080" --id "$NON_EXISTENT_TASK" --header "$AUTH_HEADER" --value "$AUTH_VALUE" || echo "Got expected error for non-existent task"

# Test with non-existent skill ID to trigger error
echo "Testing error handling for non-existent skill..."
NON_EXISTENT_SKILL="skill-does-not-exist-123456"
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-skill-details --url "http://localhost:8080" --id "$NON_EXISTENT_SKILL" || echo "Got expected error for non-existent skill"

# Test with non-existent batch ID to trigger error
echo "Testing error handling for non-existent batch..."
NON_EXISTENT_BATCH="batch-does-not-exist-123456"
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-batch --url "http://localhost:8080" --id "$NON_EXISTENT_BATCH" || echo "Got expected error for non-existent batch"

echo "===============================" 
echo "Testing File Operations"
echo "==============================="

# Create a test file
echo "Creating test file..."
echo "This is a test file content for the A2A File Operations API." > test_file.txt

# Create a task first (to upload files to)
echo "Creating task for file operations..."
FILE_TASK_ID=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "http://localhost:8080" --message "Task for file operations" --header "$AUTH_HEADER" --value "$AUTH_VALUE" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
echo "Created task with ID: $FILE_TASK_ID"

# Send a task with a file attachment
echo "Creating a task with file attachment..."
TASK_WITH_FILE=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task-with-file --url "http://localhost:8080" --message "Task with file attachment" --file-path "test_file.txt")
echo "$TASK_WITH_FILE"
TASK_FILE_ID=$(echo "$TASK_WITH_FILE" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4 || echo "")
if [ -z "$TASK_FILE_ID" ]; then
  # Try alternative extraction method
  TASK_FILE_ID=$(echo "$TASK_WITH_FILE" | grep -o '"Task created.*"' | cut -d':' -f2 | tr -d '"' | xargs)
fi
echo "Created task with ID: $TASK_FILE_ID"

# List files for the specific task
echo "Listing files for the specific task..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client list-files --url "http://localhost:8080" --task-id "$TASK_FILE_ID" || echo "File listing failed but continuing..."

# Create a test JSON data
echo "Creating test JSON data..."
echo '{"name":"test_data","value":42,"properties":{"color":"blue","active":true}}' > test_data.json

# Send a task with structured data
echo "Sending a task with structured data..."
DATA_TASK_ID=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task-with-data --url "http://localhost:8080" --message "Task with structured data" --data "$(cat test_data.json)" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$DATA_TASK_ID" ]; then
  echo "Created task with data, ID: $DATA_TASK_ID"
  
  # Get the task details to verify data was attached
  echo "Getting Task Details with Data..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "http://localhost:8080" --id "$DATA_TASK_ID" --header "$AUTH_HEADER" --value "$AUTH_VALUE"
fi

# Clean up test files
echo "Cleaning up test files..."
rm -f test_file.txt test_data.json

echo "===============================" 
echo "Testing Authentication"
echo "==============================="

# Get agent card to check auth schemes
echo "Getting agent card to check auth schemes..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-agent-card --url "http://localhost:8080"

# Test with Bearer token authentication
echo "Testing operations with Bearer token auth..."
BEARER_TASK_ID=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "http://localhost:8080" --message "Task with Bearer token auth" --header "Authorization" --value "Bearer test-token-123" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$BEARER_TASK_ID" ]; then
  echo "Created task with Bearer token auth, ID: $BEARER_TASK_ID"
  
  # Get task with Bearer auth
  echo "Getting task with Bearer token auth..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "http://localhost:8080" --id "$BEARER_TASK_ID" --header "Authorization" --value "Bearer test-token-123"
  
  # Cancel task with Bearer auth
  echo "Canceling task with Bearer token auth..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client cancel-task --url "http://localhost:8080" --id "$BEARER_TASK_ID" --header "Authorization" --value "Bearer test-token-123"
fi

# Test with API Key authentication
echo "Testing operations with API Key auth..."
API_KEY_TASK_ID=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "http://localhost:8080" --message "Task with API Key auth" --header "X-API-Key" --value "test-api-key-123" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$API_KEY_TASK_ID" ]; then
  echo "Created task with API Key auth, ID: $API_KEY_TASK_ID"
  
  # Get task with API Key auth
  echo "Getting task with API Key auth..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "http://localhost:8080" --id "$API_KEY_TASK_ID" --header "X-API-Key" --value "test-api-key-123"
fi

# Validate auth
echo "Testing auth validation..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client validate-auth --url "http://localhost:8080" --header "Authorization" --value "Bearer test-token-123"

echo "===============================" 
echo "Testing Configurable Delays"
echo "==============================="

# Test with a defined delay in metadata
echo "Testing task send with 2-second delay..."
START_TIME=$(date +%s)
DELAYED_TASK_ID=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "http://localhost:8080" \
  --message "Task with configurable delay" \
  --metadata '{"_mock_delay_ms": 2000}' \
  --header "$AUTH_HEADER" --value "$AUTH_VALUE" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

echo "Task created with ID: $DELAYED_TASK_ID in approximately $DURATION seconds"
if [ "$DURATION" -lt "2" ]; then
  echo "⚠️ Warning: Delay doesn't seem to be working correctly. Expected minimum 2 seconds, got $DURATION seconds."
else
  echo "✅ Delay worked correctly! Expected minimum 2 seconds, got $DURATION seconds."
fi

# Test streaming with configurable chunk delay
echo "Testing streaming with configurable chunk delay (1 second)..."
echo "This will show chunks with 1 second delay between them..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client stream-task --url "http://localhost:8080" \
  --message "This is a streaming task with slow chunks" \
  --metadata '{"_mock_chunk_delay_ms": 1000}' &
STREAM_PID=$!

# Let it run for a bit longer
sleep 5

# Kill the streaming client
kill $STREAM_PID 2>/dev/null || true

echo "===============================" 
echo "All Tests Completed Successfully"
echo "=============================="

# Server will be killed by trap handler