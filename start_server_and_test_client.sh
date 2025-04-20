#!/bin/bash

# This script runs a series of client commands to test A2A functionality.
# It can test against a local mock server (default) or a provided server URL.

# Exit on any error
set -e

# --- Colors and Emojis ---
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
RESET='\033[0m'

# --- Configuration ---
TARGET_URL=""
SERVER_PID=""
MANAGE_LOCAL_SERVER=false

# --- Argument Parsing ---
if [ -z "$1" ]; then
  # No URL provided, use local mock server
  echo -e "${BLUE}${BOLD}======================================================${RESET}"
  echo -e "${BLUE}${BOLD}🚀 No server URL provided. Starting local mock server...${RESET}"
  echo -e "${BLUE}${BOLD}======================================================${RESET}"
  MANAGE_LOCAL_SERVER=true
  LOCAL_PORT=8080
  TARGET_URL="http://localhost:$LOCAL_PORT"

  # Start server with "None" authentication scheme (reports auth but doesn't require it)
  RUSTFLAGS="-A warnings" cargo run --quiet -- server --port $LOCAL_PORT &
  SERVER_PID=$!

  # Give the server time to start
  echo -e "${YELLOW}⏳ Waiting for local server to start...${RESET}"
  sleep 3 # Increased sleep time slightly for reliability

  # Ensure we kill the server when the script exits ONLY if we started it
  trap "echo -e \"${YELLOW}🛑 Stopping local mock server (PID $SERVER_PID)...${RESET}\"; kill $SERVER_PID 2>/dev/null || true" EXIT

  echo -e "${GREEN}✅ Local mock server started (PID $SERVER_PID). Testing against $TARGET_URL${RESET}"
else
  # URL provided, use it directly
  TARGET_URL="$1"
  echo -e "${BLUE}${BOLD}======================================================${RESET}"
  echo -e "${BLUE}${BOLD}🔗 Testing against provided server URL: $TARGET_URL${RESET}"
  echo -e "${BLUE}${BOLD}======================================================${RESET}"
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
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}🧪 Testing Client Functions against $TARGET_URL${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"

# Get the agent card
echo -e "${YELLOW}--> Getting Agent Card...${RESET}"
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-agent-card --url "$TARGET_URL" || { echo -e "${RED}❌ Failed to get agent card${RESET}"; exit 1; }

# Send a basic task
echo -e "${YELLOW}--> Sending Task...${RESET}"
# Capture output carefully, handle potential errors if task creation fails
TASK_OUTPUT=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "$TARGET_URL" --message "This is a test task" --header "$AUTH_HEADER" --value "$AUTH_VALUE") || { echo -e "${RED}❌ Failed to send task${RESET}"; exit 1; }
TASK_ID=$(echo "$TASK_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$TASK_ID" ]; then
  echo -e "    ${GREEN}✅ Task created with ID: $TASK_ID${RESET}"

  # Get the task details
  echo -e "${YELLOW}--> Getting Task Details...${RESET}"
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "$TARGET_URL" --id "$TASK_ID" --header "$AUTH_HEADER" --value "$AUTH_VALUE" || echo -e "    ${YELLOW}⚠️ Warning: Failed to get task details${RESET}"

  # Get task state history
  echo -e "${YELLOW}--> Getting Task State History...${RESET}"
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-state-history --url "$TARGET_URL" --id "$TASK_ID" || echo -e "    ${YELLOW}⚠️ Warning: Failed to get state history${RESET}"

  # Get task state metrics
  echo -e "${YELLOW}--> Getting Task State Metrics...${RESET}"
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-state-metrics --url "$TARGET_URL" --id "$TASK_ID" || echo -e "    ${YELLOW}⚠️ Warning: Failed to get state metrics${RESET}"

  # Cancel the task
  echo -e "${YELLOW}--> Cancelling Task...${RESET}"
  RUSTFLAGS="-A warnings" cargo run --quiet -- client cancel-task --url "$TARGET_URL" --id "$TASK_ID" --header "$AUTH_HEADER" --value "$AUTH_VALUE" || echo -e "    ${YELLOW}⚠️ Warning: Failed to cancel task${RESET}"

  # Get the task details after cancellation
  echo -e "${YELLOW}--> Getting Task Details After Cancellation...${RESET}"
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "$TARGET_URL" --id "$TASK_ID" --header "$AUTH_HEADER" --value "$AUTH_VALUE" || echo -e "    ${YELLOW}⚠️ Warning: Failed to get task details after cancel${RESET}"
else
  echo -e "    ${YELLOW}⚠️ Warning: Could not extract Task ID from send-task response. Skipping dependent tests.${RESET}"
  echo "    Response was: $TASK_OUTPUT"
fi

# Test streaming
echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}🌊 Testing Streaming${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${YELLOW}--> Starting streaming task...${RESET}"
# Note that we can't easily capture the streaming output in this script
RUSTFLAGS="-A warnings" cargo run --quiet -- client stream-task --url "$TARGET_URL" --message "This is a streaming task" &
STREAM_PID=$!

# Let it run for a bit
sleep 3

# Kill the streaming client
echo -e "${YELLOW}--> Stopping streaming client...${RESET}"
kill $STREAM_PID 2>/dev/null || true
wait $STREAM_PID 2>/dev/null || true # Wait briefly to avoid job control messages

# Test push notifications (only proceed if TASK_ID was set)
if [ ! -z "$TASK_ID" ]; then
  echo
  echo -e "${BLUE}${BOLD}===============================${RESET}"
  echo -e "${BLUE}${BOLD}🔔 Testing Push Notifications${RESET}"
  echo -e "${BLUE}${BOLD}===============================${RESET}"
  echo -e "${YELLOW}--> Setting Push Notification...${RESET}"
  RUSTFLAGS="-A warnings" cargo run --quiet -- client set-push-notification --url "$TARGET_URL" --id "$TASK_ID" --webhook "https://example.com/webhook" --auth-scheme "Bearer" --token "test-token" || echo -e "    ${YELLOW}⚠️ Warning: Failed to set push notification${RESET}"

  echo -e "${YELLOW}--> Getting Push Notification...${RESET}"
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-push-notification --url "$TARGET_URL" --id "$TASK_ID" || echo -e "    ${YELLOW}⚠️ Warning: Failed to get push notification${RESET}"
fi

echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}📦 Testing Task Batching${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"
# Create a batch
echo -e "${YELLOW}--> Creating Task Batch...${RESET}"
BATCH_OUTPUT=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client create-batch --url "$TARGET_URL" --tasks "Batch Task 1,Batch Task 2,Batch Task 3" --name "Test Batch") || { echo -e "${RED}❌ Failed to create batch${RESET}"; exit 1; }
BATCH_ID=$(echo "$BATCH_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$BATCH_ID" ]; then
  echo -e "    ${GREEN}✅ Batch created with ID: $BATCH_ID${RESET}"
  # Get batch info
  echo -e "${YELLOW}--> Getting Batch Info...${RESET}"
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-batch --url "$TARGET_URL" --id "$BATCH_ID" || echo -e "    ${YELLOW}⚠️ Warning: Failed to get batch info${RESET}"

  # Get batch status
  echo -e "${YELLOW}--> Getting Batch Status...${RESET}"
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-batch-status --url "$TARGET_URL" --id "$BATCH_ID" || echo -e "    ${YELLOW}⚠️ Warning: Failed to get batch status${RESET}"

  # Cancel batch
  echo -e "${YELLOW}--> Cancelling Batch...${RESET}"
  RUSTFLAGS="-A warnings" cargo run --quiet -- client cancel-batch --url "$TARGET_URL" --id "$BATCH_ID" || echo -e "    ${YELLOW}⚠️ Warning: Failed to cancel batch${RESET}"
else
  echo -e "    ${YELLOW}⚠️ Warning: Could not extract Batch ID from create-batch response. Skipping dependent tests.${RESET}"
  echo "    Response was: $BATCH_OUTPUT"
fi

echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}🛠️ Testing Agent Skills${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"
# List all skills
echo -e "${YELLOW}--> Listing all skills...${RESET}"
RUSTFLAGS="-A warnings" cargo run --quiet -- client list-skills --url "$TARGET_URL" || echo -e "    ${YELLOW}⚠️ Warning: Failed to list skills${RESET}"

# List skills filtered by tag
echo -e "${YELLOW}--> Listing skills with 'text' tag...${RESET}"
RUSTFLAGS="-A warnings" cargo run --quiet -- client list-skills --url "$TARGET_URL" --tags "text" || echo -e "    ${YELLOW}⚠️ Warning: Failed to list skills by tag${RESET}"

# Get details for a specific skill
echo -e "${YELLOW}--> Getting details for skill 'test-skill-1'...${RESET}"
SKILL_ID="test-skill-1"
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-skill-details --url "$TARGET_URL" --id "$SKILL_ID" || echo -e "    ${YELLOW}⚠️ Warning: Failed to get skill details${RESET}"

# Invoke a skill
echo -e "${YELLOW}--> Invoking skill 'test-skill-1'...${RESET}"
RUSTFLAGS="-A warnings" cargo run --quiet -- client invoke-skill --url "$TARGET_URL" --id "$SKILL_ID" --message "This is a test skill invocation" || echo -e "    ${YELLOW}⚠️ Warning: Failed to invoke skill${RESET}"

# Invoke a skill with specific input/output modes
echo -e "${YELLOW}--> Invoking skill 'test-skill-1' with specific modes...${RESET}"
RUSTFLAGS="-A warnings" cargo run --quiet -- client invoke-skill --url "$TARGET_URL" --id "$SKILL_ID" --message "This is a test with specific modes" -n "text/plain" -p "text/plain" || echo -e "    ${YELLOW}⚠️ Warning: Failed to invoke skill with modes${RESET}"

echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}🚨 Testing Error Handling${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"

# Test with non-existent task ID to trigger error
echo -e "${YELLOW}--> Testing error handling for non-existent task...${RESET}"
NON_EXISTENT_TASK="task-does-not-exist-$(uuidgen)"
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "$TARGET_URL" --id "$NON_EXISTENT_TASK" --header "$AUTH_HEADER" --value "$AUTH_VALUE" || echo -e "    ${GREEN}✅ Got expected error for non-existent task${RESET}"

# Test with non-existent skill ID to trigger error
echo -e "${YELLOW}--> Testing error handling for non-existent skill...${RESET}"
NON_EXISTENT_SKILL="skill-does-not-exist-$(uuidgen)"
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-skill-details --url "$TARGET_URL" --id "$NON_EXISTENT_SKILL" || echo -e "    ${GREEN}✅ Got expected error for non-existent skill${RESET}"

# Test with non-existent batch ID to trigger error
echo -e "${YELLOW}--> Testing error handling for non-existent batch...${RESET}"
NON_EXISTENT_BATCH="batch-does-not-exist-$(uuidgen)"
RUSTFLAGS="-A warnings" cargo run --quiet -- client get-batch --url "$TARGET_URL" --id "$NON_EXISTENT_BATCH" || echo -e "    ${GREEN}✅ Got expected error for non-existent batch${RESET}"

echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}📁 Testing File Operations${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"

# Create a test file
TEMP_DIR=$(mktemp -d)
TEST_FILE="$TEMP_DIR/test_file.txt"
echo -e "${YELLOW}--> Creating test file at $TEST_FILE...${RESET}"
echo "This is a test file content for the A2A File Operations API." > "$TEST_FILE"

# Create a task first (to associate uploads with, if needed by server)
echo -e "${YELLOW}--> Creating task for file operations...${RESET}"
FILE_TASK_OUTPUT=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "$TARGET_URL" --message "Task for file operations" --header "$AUTH_HEADER" --value "$AUTH_VALUE") || { echo -e "${RED}❌ Failed to create task for file ops${RESET}"; exit 1; }
FILE_TASK_ID=$(echo "$FILE_TASK_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
echo -e "    ${GREEN}✅ Created task with ID: $FILE_TASK_ID${RESET}"

# Upload the file
echo -e "${YELLOW}--> Uploading file...${RESET}"
UPLOAD_OUTPUT=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client upload-file --url "$TARGET_URL" --file "$TEST_FILE" --task-id "$FILE_TASK_ID") || { echo -e "${RED}❌ Failed to upload file${RESET}"; exit 1; }
UPLOADED_FILE_ID=$(echo "$UPLOAD_OUTPUT" | grep -o '"file_id": "[^"]*"' | head -1 | cut -d'"' -f4)
echo -e "    ${GREEN}✅ File uploaded with ID: $UPLOADED_FILE_ID${RESET}"

# List files for the specific task
echo -e "${YELLOW}--> Listing files for task $FILE_TASK_ID...${RESET}"
RUSTFLAGS="-A warnings" cargo run --quiet -- client list-files --url "$TARGET_URL" --task-id "$FILE_TASK_ID" || echo -e "    ${YELLOW}⚠️ Warning: File listing failed but continuing...${RESET}"

# Download the file
if [ ! -z "$UPLOADED_FILE_ID" ]; then
  DOWNLOAD_PATH="$TEMP_DIR/downloaded_file.txt"
  echo -e "${YELLOW}--> Downloading file $UPLOADED_FILE_ID to $DOWNLOAD_PATH...${RESET}"
  RUSTFLAGS="-A warnings" cargo run --quiet -- client download-file --url "$TARGET_URL" --id "$UPLOADED_FILE_ID" --output "$DOWNLOAD_PATH" || echo -e "    ${YELLOW}⚠️ Warning: Failed to download file${RESET}"
  # Optional: Verify downloaded content
  # diff "$TEST_FILE" "$DOWNLOAD_PATH" || echo "    Warning: Downloaded file content mismatch"
fi

# Send a task with a file attachment
echo -e "${YELLOW}--> Creating a task with file attachment...${RESET}"
RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task-with-file --url "$TARGET_URL" --message "Task with file attachment" --file-path "$TEST_FILE" || echo -e "    ${YELLOW}⚠️ Warning: Failed to send task with file${RESET}"

# Create test JSON data
TEST_DATA_FILE="$TEMP_DIR/test_data.json"
echo -e "${YELLOW}--> Creating test JSON data at $TEST_DATA_FILE...${RESET}"
echo '{"name":"test_data","value":42,"properties":{"color":"blue","active":true}}' > "$TEST_DATA_FILE"

# Send a task with structured data
echo -e "${YELLOW}--> Sending a task with structured data...${RESET}"
DATA_TASK_OUTPUT=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task-with-data --url "$TARGET_URL" --message "Task with structured data" --data "$(cat $TEST_DATA_FILE)") || { echo -e "${RED}❌ Failed to send task with data${RESET}"; exit 1; }
DATA_TASK_ID=$(echo "$DATA_TASK_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$DATA_TASK_ID" ]; then
  echo -e "    ${GREEN}✅ Created task with data, ID: $DATA_TASK_ID${RESET}"
  # Get the task details to verify data was attached (mock server might not reflect this)
  echo -e "${YELLOW}--> Getting Task Details with Data...${RESET}"
  RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "$TARGET_URL" --id "$DATA_TASK_ID" --header "$AUTH_HEADER" --value "$AUTH_VALUE" || echo -e "    ${YELLOW}⚠️ Warning: Failed to get task details for data task${RESET}"
fi

# Clean up test files
echo -e "${YELLOW}--> Cleaning up temporary directory $TEMP_DIR...${RESET}"
rm -rf "$TEMP_DIR"

echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}🔑 Testing Authentication Validation${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"

# Validate auth (using the default test token)
echo -e "${YELLOW}--> Testing auth validation...${RESET}"
RUSTFLAGS="-A warnings" cargo run --quiet -- client validate-auth --url "$TARGET_URL" --header "$AUTH_HEADER" --value "$AUTH_VALUE" || echo -e "    ${YELLOW}⚠️ Warning: Auth validation failed (might be expected if server requires real auth)${RESET}"

echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}⏱️ Testing Configurable Delays${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"

# Test with a defined delay in metadata
echo -e "${YELLOW}--> Testing task send with 2-second delay...${RESET}"
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
  echo -e "    ${YELLOW}⚠️ Note: Delay might not be supported by this server (Expected >= 2s, got ${DURATION}s).${RESET}"
else
  echo -e "    ${GREEN}✅ Delay seems to have worked correctly!${RESET}"
fi

# Test streaming with configurable chunk delay
echo -e "${YELLOW}--> Testing streaming with configurable chunk delay (1 second)...${RESET}"
echo "    (This will show chunks with 1 second delay between them if supported by server)"
RUSTFLAGS="-A warnings" cargo run --quiet -- client stream-task --url "$TARGET_URL" \
  --message "This is a streaming task with slow chunks" \
  --metadata '{"_mock_chunk_delay_ms": 1000}' &
STREAM_PID=$!

# Let it run for a bit longer
sleep 5

# Kill the streaming client
echo -e "${YELLOW}--> Stopping slow streaming client...${RESET}"
kill $STREAM_PID 2>/dev/null || true
wait $STREAM_PID 2>/dev/null || true

echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}🌊 Testing Dynamic Streaming Content${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"

# Create a task for resubscribe testing
echo -e "${YELLOW}--> Creating a task for resubscribe testing...${RESET}"
RESUBSCRIBE_TASK_OUTPUT=$(RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "$TARGET_URL" \
  --message "Task for resubscribe testing") || { echo -e "${RED}❌ Failed to create task for resubscribe${RESET}"; exit 1; }
RESUBSCRIBE_TASK_ID=$(echo "$RESUBSCRIBE_TASK_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
echo -e "    ${GREEN}✅ Created task with ID: $RESUBSCRIBE_TASK_ID${RESET}"

# Test streaming with custom text chunks
echo -e "${YELLOW}--> Testing streaming with 3 text chunks...${RESET}"
RUSTFLAGS="-A warnings" cargo run --quiet -- client stream-task --url "$TARGET_URL" \
  --message "Streaming task with 3 text chunks" \
  --metadata '{"_mock_stream_text_chunks": 3, "_mock_stream_chunk_delay_ms": 500}' &
STREAM_PID=$!
sleep 4
kill $STREAM_PID 2>/dev/null || true
wait $STREAM_PID 2>/dev/null || true

# Test streaming with only data artifacts
echo -e "${YELLOW}--> Testing streaming with only data artifacts...${RESET}"
RUSTFLAGS="-A warnings" cargo run --quiet -- client stream-task --url "$TARGET_URL" \
  --message "Streaming task with only data artifacts" \
  --metadata '{"_mock_stream_artifact_types": ["data"], "_mock_stream_chunk_delay_ms": 500}' &
STREAM_PID=$!
sleep 3
kill $STREAM_PID 2>/dev/null || true
wait $STREAM_PID 2>/dev/null || true

# Test streaming with custom final state
echo -e "${YELLOW}--> Testing streaming with failed final state...${RESET}"
RUSTFLAGS="-A warnings" cargo run --quiet -- client stream-task --url "$TARGET_URL" \
  --message "Streaming task with failed final state" \
  --metadata '{"_mock_stream_final_state": "failed", "_mock_stream_chunk_delay_ms": 500}' &
STREAM_PID=$!
sleep 3
kill $STREAM_PID 2>/dev/null || true
wait $STREAM_PID 2>/dev/null || true

# Test resubscribe with dynamic configuration
if [ ! -z "$RESUBSCRIBE_TASK_ID" ]; then
  echo -e "${YELLOW}--> Testing resubscribe with dynamic configuration...${RESET}"
  RUSTFLAGS="-A warnings" cargo run --quiet -- client resubscribe-task --url "$TARGET_URL" \
    --id "$RESUBSCRIBE_TASK_ID" \
    --metadata '{"_mock_stream_text_chunks": 2, "_mock_stream_artifact_types": ["text", "data"]}' &
  STREAM_PID=$!
  sleep 4
  kill $STREAM_PID 2>/dev/null || true
  wait $STREAM_PID 2>/dev/null || true
else
  echo -e "    ${YELLOW}⚠️ Skipping resubscribe test as task creation failed earlier.${RESET}"
fi

echo
echo -e "${GREEN}${BOLD}======================================================${RESET}"
echo -e "${GREEN}${BOLD}🎉 All Tests Completed against $TARGET_URL 🎉${RESET}"
echo -e "${GREEN}${BOLD}======================================================${RESET}"

# Server (if started locally) will be killed by trap handler
exit 0
