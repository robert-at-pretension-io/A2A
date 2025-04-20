#!/bin/bash

# This script runs a series of client commands to test A2A functionality.
# It can test against a local mock server (default) or a provided server URL.

# Exit on any error
set -e

# --- Configuration ---
TARGET_URL=""
SERVER_PID=""
MANAGE_LOCAL_SERVER=false

# --- Argument Parsing ---
if [ -z "$1" ]; then
  # No URL provided, use local mock server
  echo "======================================================"
  echo "No server URL provided. Starting local mock server..."
  echo "======================================================"
  MANAGE_LOCAL_SERVER=true
  LOCAL_PORT=8080
  TARGET_URL="http://localhost:$LOCAL_PORT"

  # Start server with "None" authentication scheme (reports auth but doesn't require it)
  RUSTFLAGS="-A warnings" cargo run --quiet -- server --port $LOCAL_PORT &
  SERVER_PID=$!

  # Give the server time to start
  echo "Waiting for local server to start..."
  sleep 3 # Increased sleep time slightly for reliability

  # Ensure we kill the server when the script exits ONLY if we started it
  trap "echo 'Stopping local mock server (PID $SERVER_PID)...'; kill $SERVER_PID 2>/dev/null || true" EXIT

  echo "Local mock server started (PID $SERVER_PID). Testing against $TARGET_URL"
else
  # URL provided, use it directly
  TARGET_URL="$1"
  echo "======================================================"
  echo "Testing against provided server URL: $TARGET_URL"
  echo "======================================================"
  # No trap needed as we didn't start the server
fi

# Basic validation
if [ -z "$TARGET_URL" ]; then
    echo "Error: Could not determine target URL." >&2
    exit 1
fi

# --- Authentication (May need adjustment for external servers) ---
# Using "None" scheme for local mock server allows these to pass without actual validation
export AUTH_HEADER="Authorization"
export AUTH_VALUE="Bearer test-token"
echo "(Using default test authentication: Header='$AUTH_HEADER', Value='$AUTH_VALUE')"
echo "Note: Authentication might need adjustment for external servers."

# --- Test Execution ---
echo
echo "==============================="
echo "Testing Client Functions against $TARGET_URL"
echo "==============================="

# Get the agent card
echo "--> Getting Agent Card..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-agent-card --url "$TARGET_URL" || { echo "Failed to get agent card"; exit 1; }

# Send a basic task
echo "--> Sending Task..."
# Capture output carefully, handle potential errors if task creation fails
TASK_OUTPUT=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "$TARGET_URL" --message "This is a test task" --header "$AUTH_HEADER" --value "$AUTH_VALUE") || { echo "Failed to send task"; exit 1; }
TASK_ID=$(echo "$TASK_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$TASK_ID" ]; then
  echo "    Task created with ID: $TASK_ID"

  # Get the task details
  echo "--> Getting Task Details..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "$TARGET_URL" --id "$TASK_ID" --header "$AUTH_HEADER" --value "$AUTH_VALUE" || echo "    Warning: Failed to get task details"

  # Get task state history
  echo "--> Getting Task State History..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-state-history --url "$TARGET_URL" --id "$TASK_ID" || echo "    Warning: Failed to get state history"

  # Get task state metrics
  echo "--> Getting Task State Metrics..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-state-metrics --url "$TARGET_URL" --id "$TASK_ID" || echo "    Warning: Failed to get state metrics"

  # Cancel the task
  echo "--> Cancelling Task..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client cancel-task --url "$TARGET_URL" --id "$TASK_ID" --header "$AUTH_HEADER" --value "$AUTH_VALUE" || echo "    Warning: Failed to cancel task"

  # Get the task details after cancellation
  echo "--> Getting Task Details After Cancellation..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "$TARGET_URL" --id "$TASK_ID" --header "$AUTH_HEADER" --value "$AUTH_VALUE" || echo "    Warning: Failed to get task details after cancel"
else
  echo "    Warning: Could not extract Task ID from send-task response. Skipping dependent tests."
  echo "    Response was: $TASK_OUTPUT"
fi

# Test streaming
echo
echo "==============================="
echo "Testing Streaming"
echo "==============================="
echo "--> Starting streaming task..."
# Note that we can't easily capture the streaming output in this script
RUSTFLAGS="-A warnings" cargo run --quiet -- client stream-task --url "$TARGET_URL" --message "This is a streaming task" &
STREAM_PID=$!

# Let it run for a bit
sleep 3

# Kill the streaming client
echo "--> Stopping streaming client..."
kill $STREAM_PID 2>/dev/null || true
wait $STREAM_PID 2>/dev/null || true # Wait briefly to avoid job control messages

# Test push notifications (only proceed if TASK_ID was set)
if [ ! -z "$TASK_ID" ]; then
  echo
  echo "==============================="
  echo "Testing Push Notifications"
  echo "==============================="
  echo "--> Setting Push Notification..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client set-push-notification --url "$TARGET_URL" --id "$TASK_ID" --webhook "https://example.com/webhook" --auth-scheme "Bearer" --token "test-token" || echo "    Warning: Failed to set push notification"

  echo "--> Getting Push Notification..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-push-notification --url "$TARGET_URL" --id "$TASK_ID" || echo "    Warning: Failed to get push notification"
fi

echo
echo "==============================="
echo "Testing Task Batching"
echo "==============================="
# Create a batch
echo "--> Creating Task Batch..."
BATCH_OUTPUT=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client create-batch --url "$TARGET_URL" --tasks "Batch Task 1,Batch Task 2,Batch Task 3" --name "Test Batch") || { echo "Failed to create batch"; exit 1; }
BATCH_ID=$(echo "$BATCH_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$BATCH_ID" ]; then
  echo "    Batch created with ID: $BATCH_ID"
  # Get batch info
  echo "--> Getting Batch Info..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-batch --url "$TARGET_URL" --id "$BATCH_ID" || echo "    Warning: Failed to get batch info"

  # Get batch status
  echo "--> Getting Batch Status..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-batch-status --url "$TARGET_URL" --id "$BATCH_ID" || echo "    Warning: Failed to get batch status"

  # Cancel batch
  echo "--> Cancelling Batch..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client cancel-batch --url "$TARGET_URL" --id "$BATCH_ID" || echo "    Warning: Failed to cancel batch"
else
  echo "    Warning: Could not extract Batch ID from create-batch response. Skipping dependent tests."
  echo "    Response was: $BATCH_OUTPUT"
fi

echo
echo "==============================="
echo "Testing Agent Skills"
echo "==============================="
# List all skills
echo "--> Listing all skills..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client list-skills --url "$TARGET_URL" || echo "    Warning: Failed to list skills"

# List skills filtered by tag
echo "--> Listing skills with 'text' tag..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client list-skills --url "$TARGET_URL" --tags "text" || echo "    Warning: Failed to list skills by tag"

# Get details for a specific skill
echo "--> Getting details for skill 'test-skill-1'..."
SKILL_ID="test-skill-1"
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-skill-details --url "$TARGET_URL" --id "$SKILL_ID" || echo "    Warning: Failed to get skill details"

# Invoke a skill
echo "--> Invoking skill 'test-skill-1'..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client invoke-skill --url "$TARGET_URL" --id "$SKILL_ID" --message "This is a test skill invocation" || echo "    Warning: Failed to invoke skill"

# Invoke a skill with specific input/output modes
echo "--> Invoking skill 'test-skill-1' with specific modes..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client invoke-skill --url "$TARGET_URL" --id "$SKILL_ID" --message "This is a test with specific modes" -n "text/plain" -p "text/plain" || echo "    Warning: Failed to invoke skill with modes"

echo
echo "==============================="
echo "Testing Error Handling"
echo "==============================="

# Test with non-existent task ID to trigger error
echo "--> Testing error handling for non-existent task..."
NON_EXISTENT_TASK="task-does-not-exist-$(uuidgen)"
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "$TARGET_URL" --id "$NON_EXISTENT_TASK" --header "$AUTH_HEADER" --value "$AUTH_VALUE" || echo "    ✅ Got expected error for non-existent task"

# Test with non-existent skill ID to trigger error
echo "--> Testing error handling for non-existent skill..."
NON_EXISTENT_SKILL="skill-does-not-exist-$(uuidgen)"
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-skill-details --url "$TARGET_URL" --id "$NON_EXISTENT_SKILL" || echo "    ✅ Got expected error for non-existent skill"

# Test with non-existent batch ID to trigger error
echo "--> Testing error handling for non-existent batch..."
NON_EXISTENT_BATCH="batch-does-not-exist-$(uuidgen)"
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-batch --url "$TARGET_URL" --id "$NON_EXISTENT_BATCH" || echo "    ✅ Got expected error for non-existent batch"

echo
echo "==============================="
echo "Testing File Operations"
echo "==============================="

# Create a test file
TEMP_DIR=$(mktemp -d)
TEST_FILE="$TEMP_DIR/test_file.txt"
echo "--> Creating test file at $TEST_FILE..."
echo "This is a test file content for the A2A File Operations API." > "$TEST_FILE"

# Create a task first (to associate uploads with, if needed by server)
echo "--> Creating task for file operations..."
FILE_TASK_OUTPUT=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "$TARGET_URL" --message "Task for file operations" --header "$AUTH_HEADER" --value "$AUTH_VALUE") || { echo "Failed to create task for file ops"; exit 1; }
FILE_TASK_ID=$(echo "$FILE_TASK_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
echo "    Created task with ID: $FILE_TASK_ID"

# Upload the file
echo "--> Uploading file..."
UPLOAD_OUTPUT=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client upload-file --url "$TARGET_URL" --file "$TEST_FILE" --task-id "$FILE_TASK_ID") || { echo "Failed to upload file"; exit 1; }
UPLOADED_FILE_ID=$(echo "$UPLOAD_OUTPUT" | grep -o '"file_id": "[^"]*"' | head -1 | cut -d'"' -f4)
echo "    File uploaded with ID: $UPLOADED_FILE_ID"

# List files for the specific task
echo "--> Listing files for task $FILE_TASK_ID..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client list-files --url "$TARGET_URL" --task-id "$FILE_TASK_ID" || echo "    Warning: File listing failed but continuing..."

# Download the file
if [ ! -z "$UPLOADED_FILE_ID" ]; then
  DOWNLOAD_PATH="$TEMP_DIR/downloaded_file.txt"
  echo "--> Downloading file $UPLOADED_FILE_ID to $DOWNLOAD_PATH..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client download-file --url "$TARGET_URL" --id "$UPLOADED_FILE_ID" --output "$DOWNLOAD_PATH" || echo "    Warning: Failed to download file"
  # Optional: Verify downloaded content
  # diff "$TEST_FILE" "$DOWNLOAD_PATH" || echo "    Warning: Downloaded file content mismatch"
fi

# Send a task with a file attachment
echo "--> Creating a task with file attachment..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task-with-file --url "$TARGET_URL" --message "Task with file attachment" --file-path "$TEST_FILE" || echo "    Warning: Failed to send task with file"

# Create test JSON data
TEST_DATA_FILE="$TEMP_DIR/test_data.json"
echo "--> Creating test JSON data at $TEST_DATA_FILE..."
echo '{"name":"test_data","value":42,"properties":{"color":"blue","active":true}}' > "$TEST_DATA_FILE"

# Send a task with structured data
echo "--> Sending a task with structured data..."
DATA_TASK_OUTPUT=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task-with-data --url "$TARGET_URL" --message "Task with structured data" --data "$(cat $TEST_DATA_FILE)") || { echo "Failed to send task with data"; exit 1; }
DATA_TASK_ID=$(echo "$DATA_TASK_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$DATA_TASK_ID" ]; then
  echo "    Created task with data, ID: $DATA_TASK_ID"
  # Get the task details to verify data was attached (mock server might not reflect this)
  echo "--> Getting Task Details with Data..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "$TARGET_URL" --id "$DATA_TASK_ID" --header "$AUTH_HEADER" --value "$AUTH_VALUE" || echo "    Warning: Failed to get task details for data task"
fi

# Clean up test files
echo "--> Cleaning up temporary directory $TEMP_DIR..."
rm -rf "$TEMP_DIR"

echo
echo "==============================="
echo "Testing Authentication Validation"
echo "==============================="

# Validate auth (using the default test token)
echo "--> Testing auth validation..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client validate-auth --url "$TARGET_URL" --header "$AUTH_HEADER" --value "$AUTH_VALUE" || echo "    Warning: Auth validation failed (might be expected if server requires real auth)"

echo
echo "==============================="
echo "Testing Configurable Delays"
echo "==============================="

# Test with a defined delay in metadata
echo "--> Testing task send with 2-second delay..."
START_TIME=$(date +%s)
DELAYED_TASK_OUTPUT=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "$TARGET_URL" \
  --message "Task with configurable delay" \
  --metadata '{"_mock_delay_ms": 2000}' \
  --header "$AUTH_HEADER" --value "$AUTH_VALUE") || { echo "Failed to send delayed task"; exit 1; }
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))
DELAYED_TASK_ID=$(echo "$DELAYED_TASK_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

echo "    Task created with ID: $DELAYED_TASK_ID in approximately $DURATION seconds"
if [ "$DURATION" -lt "2" ]; then
  # This might happen if the server doesn't support the _mock_delay_ms metadata
  echo "    Note: Delay might not be supported by this server (Expected >= 2s, got ${DURATION}s)."
else
  echo "    ✅ Delay seems to have worked correctly!"
fi

# Test streaming with configurable chunk delay
echo "--> Testing streaming with configurable chunk delay (1 second)..."
echo "    (This will show chunks with 1 second delay between them if supported by server)"
RUSTFLAGS="-A warnings" cargo run --quiet -- client stream-task --url "$TARGET_URL" \
  --message "This is a streaming task with slow chunks" \
  --metadata '{"_mock_chunk_delay_ms": 1000}' &
STREAM_PID=$!

# Let it run for a bit longer
sleep 5

# Kill the streaming client
echo "--> Stopping slow streaming client..."
kill $STREAM_PID 2>/dev/null || true
wait $STREAM_PID 2>/dev/null || true

echo
echo "==============================="
echo "Testing Dynamic Streaming Content"
echo "==============================="

# Create a task for resubscribe testing
echo "--> Creating a task for resubscribe testing..."
RESUBSCRIBE_TASK_OUTPUT=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "$TARGET_URL" \
  --message "Task for resubscribe testing") || { echo "Failed to create task for resubscribe"; exit 1; }
RESUBSCRIBE_TASK_ID=$(echo "$RESUBSCRIBE_TASK_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
echo "    Created task with ID: $RESUBSCRIBE_TASK_ID"

# Test streaming with custom text chunks
echo "--> Testing streaming with 3 text chunks..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client stream-task --url "$TARGET_URL" \
  --message "Streaming task with 3 text chunks" \
  --metadata '{"_mock_stream_text_chunks": 3, "_mock_stream_chunk_delay_ms": 500}' &
STREAM_PID=$!
sleep 4
kill $STREAM_PID 2>/dev/null || true
wait $STREAM_PID 2>/dev/null || true

# Test streaming with only data artifacts
echo "--> Testing streaming with only data artifacts..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client stream-task --url "$TARGET_URL" \
  --message "Streaming task with only data artifacts" \
  --metadata '{"_mock_stream_artifact_types": ["data"], "_mock_stream_chunk_delay_ms": 500}' &
STREAM_PID=$!
sleep 3
kill $STREAM_PID 2>/dev/null || true
wait $STREAM_PID 2>/dev/null || true

# Test streaming with custom final state
echo "--> Testing streaming with failed final state..."
RUSTFLAGS="-A warnings" cargo run --quiet -- client stream-task --url "$TARGET_URL" \
  --message "Streaming task with failed final state" \
  --metadata '{"_mock_stream_final_state": "failed", "_mock_stream_chunk_delay_ms": 500}' &
STREAM_PID=$!
sleep 3
kill $STREAM_PID 2>/dev/null || true
wait $STREAM_PID 2>/dev/null || true

# Test resubscribe with dynamic configuration
if [ ! -z "$RESUBSCRIBE_TASK_ID" ]; then
  echo "--> Testing resubscribe with dynamic configuration..."
  RUSTFLAGS="-A warnings" cargo run --quiet -- client resubscribe-task --url "$TARGET_URL" \
    --id "$RESUBSCRIBE_TASK_ID" \
    --metadata '{"_mock_stream_text_chunks": 2, "_mock_stream_artifact_types": ["text", "data"]}' &
  STREAM_PID=$!
  sleep 4
  kill $STREAM_PID 2>/dev/null || true
  wait $STREAM_PID 2>/dev/null || true
else
  echo "    Skipping resubscribe test as task creation failed earlier."
fi

echo
echo "======================================================"
echo "All Tests Completed against $TARGET_URL"
echo "======================================================"

# Server (if started locally) will be killed by trap handler
exit 0
