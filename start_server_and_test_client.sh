#!/bin/bash

# This script runs a series of client commands to test A2A functionality.
# It can test against a local mock server (default) or a provided server URL.

# --- Colors and Emojis ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
BOLD='\033[1m'
RESET='\033[0m'

# --- Configuration ---
TARGET_URL=""
SERVER_PID=""
MANAGE_LOCAL_SERVER=false
DEFAULT_TIMEOUT=15 # Default timeout in seconds for commands
SUCCESS_COUNT=0
FAILURE_COUNT=0
TEST_COUNTER=0
RUN_UNOFFICIAL_TESTS=false

# --- Helper Function ---
# Usage: run_test "Description" "command_string" [timeout_override] [is_unofficial]
run_test() {
  local description="$1"
  local command_str="$2"
  local timeout_val="${3:-$DEFAULT_TIMEOUT}"
  local is_unofficial="${4:-false}" # Default to false if not provided
  local test_num=$((TEST_COUNTER + 1))
  TEST_COUNTER=$test_num

  # Check if this is an unofficial test and if we should run it
  if [ "$is_unofficial" = "true" ] && [ "$RUN_UNOFFICIAL_TESTS" = "false" ]; then
    echo -e "${BLUE}⏭️ [Test ${test_num}] Skipping unofficial test: ${description} (Use --run-unofficial to include)${RESET}"
    # Don't increment success/failure counts for skipped tests
    return 0 # Return success to not fail the script
  fi

  echo -e "${YELLOW}--> [Test ${test_num}] Running: ${description}...${RESET}"
  echo "    Command: timeout ${timeout_val}s ${command_str}"

  # Execute the command with timeout
  timeout "${timeout_val}s" bash -c "${command_str}"
  local exit_status=$?

  if [ $exit_status -eq 0 ]; then
    echo -e "    ${GREEN}✅ [Test ${test_num}] Success: ${description}${RESET}"
    SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
    return 0
  elif [ $exit_status -eq 124 ]; then
    echo -e "    ${RED}❌ [Test ${test_num}] Failed (Timeout): ${description}${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
    return 1
  else
    echo -e "    ${RED}❌ [Test ${test_num}] Failed (Exit Code ${exit_status}): ${description}${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
    return 1
  fi
}

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
  # URL provided, use it directly
  TARGET_URL="$1"
  # Shift arguments to check for --run-unofficial
  shift
  echo -e "${BLUE}${BOLD}======================================================${RESET}"
  echo -e "${BLUE}${BOLD}🔗 Testing against provided server URL: $TARGET_URL${RESET}"
  echo -e "${BLUE}${BOLD}======================================================${RESET}"
  # No trap needed as we didn't start the server
fi

# Check for --run-unofficial flag
if [[ " $@ " =~ " --run-unofficial " ]]; then
  RUN_UNOFFICIAL_TESTS=true
  echo -e "${YELLOW}🧪 Including unofficial tests.${RESET}"
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
run_test "Get Agent Card" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client get-agent-card --url \"$TARGET_URL\""

# Send a basic task and capture ID
TASK_ID=""
echo -e "${YELLOW}--> [Test $((TEST_COUNTER + 1))] Running: Send Task...${RESET}"
TASK_OUTPUT=$(timeout "${DEFAULT_TIMEOUT}s" RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "$TARGET_URL" --message "This is a test task" --header "$AUTH_HEADER" --value "$AUTH_VALUE")
exit_status=$?
if [ $exit_status -eq 0 ]; then
  TASK_ID=$(echo "$TASK_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
  if [ ! -z "$TASK_ID" ]; then
    echo -e "    ${GREEN}✅ [Test $((TEST_COUNTER + 1))] Success: Send Task (ID: $TASK_ID)${RESET}"
    SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
  else
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed: Send Task (Could not extract ID)${RESET}"
    echo "       Output: $TASK_OUTPUT"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
  fi
elif [ $exit_status -eq 124 ]; then
  echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed (Timeout): Send Task${RESET}"
  FAILURE_COUNT=$((FAILURE_COUNT + 1))
else
  echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed (Exit Code ${exit_status}): Send Task${RESET}"
  FAILURE_COUNT=$((FAILURE_COUNT + 1))
fi
TEST_COUNTER=$((TEST_COUNTER + 1))


# Dependent tests: Only run if TASK_ID was successfully obtained
if [ ! -z "$TASK_ID" ]; then
  run_test "Get Task Details" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client get-task --url \"$TARGET_URL\" --id \"$TASK_ID\" --header \"$AUTH_HEADER\" --value \"$AUTH_VALUE\""
  run_test "Get Task State History" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client get-state-history --url \"$TARGET_URL\" --id \"$TASK_ID\""
  run_test "Get Task State Metrics" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client get-state-metrics --url \"$TARGET_URL\" --id \"$TASK_ID\""
  run_test "Cancel Task" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client cancel-task --url \"$TARGET_URL\" --id \"$TASK_ID\" --header \"$AUTH_HEADER\" --value \"$AUTH_VALUE\""
  run_test "Get Task Details After Cancellation" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client get-task --url \"$TARGET_URL\" --id \"$TASK_ID\" --header \"$AUTH_HEADER\" --value \"$AUTH_VALUE\""
else
  echo -e "${YELLOW}⚠️ Skipping tests dependent on successful task creation.${RESET}"
fi

# Test streaming
echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}🌊 Testing Streaming${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"
# Test if the streaming client *starts* successfully
run_test "Start Streaming Task" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client stream-task --url \"$TARGET_URL\" --message \"This is a streaming task\" & echo \$! > stream_pid.txt" 5 # Short timeout for just starting
STREAM_PID=$(cat stream_pid.txt)
rm stream_pid.txt

if [ $? -eq 0 ] && [ ! -z "$STREAM_PID" ]; then
  echo "    Streaming client started (PID $STREAM_PID). Letting it run for a few seconds..."
  sleep 3
  echo -e "${YELLOW}--> Stopping streaming client (PID $STREAM_PID)...${RESET}"
  kill $STREAM_PID 2>/dev/null || true
  wait $STREAM_PID 2>/dev/null || true # Wait briefly to avoid job control messages
else
  echo -e "${YELLOW}⚠️ Streaming client failed to start or PID not captured. Skipping kill.${RESET}"
fi


# Test push notifications (only proceed if TASK_ID was set)
if [ ! -z "$TASK_ID" ]; then
  echo
  echo -e "${BLUE}${BOLD}===============================${RESET}"
  echo -e "${BLUE}${BOLD}🔔 Testing Push Notifications${RESET}"
  echo -e "${BLUE}${BOLD}===============================${RESET}"
  run_test "Set Push Notification" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client set-push-notification --url \"$TARGET_URL\" --id \"$TASK_ID\" --webhook \"https://example.com/webhook\" --auth-scheme \"Bearer\" --token \"test-token\""
  run_test "Get Push Notification" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client get-push-notification --url \"$TARGET_URL\" --id \"$TASK_ID\""
else
   echo -e "${YELLOW}⚠️ Skipping Push Notification tests as TASK_ID was not set.${RESET}"
fi

echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}📦 Testing Task Batching${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"
# Create a batch and capture ID
BATCH_ID=""
echo -e "${YELLOW}--> [Test $((TEST_COUNTER + 1))] Running: Create Task Batch...${RESET}"
BATCH_OUTPUT=$(timeout "${DEFAULT_TIMEOUT}s" RUSTFLAGS="-A warnings" cargo run --quiet -- client create-batch --url "$TARGET_URL" --tasks "Batch Task 1,Batch Task 2,Batch Task 3" --name "Test Batch")
exit_status=$?
if [ $exit_status -eq 0 ]; then
  BATCH_ID=$(echo "$BATCH_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
  if [ ! -z "$BATCH_ID" ]; then
    echo -e "    ${GREEN}✅ [Test $((TEST_COUNTER + 1))] Success: Create Task Batch (ID: $BATCH_ID)${RESET}"
    SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
  else
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed: Create Task Batch (Could not extract ID)${RESET}"
    echo "       Output: $BATCH_OUTPUT"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
  fi
elif [ $exit_status -eq 124 ]; then
  echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed (Timeout): Create Task Batch${RESET}"
  FAILURE_COUNT=$((FAILURE_COUNT + 1))
else
  echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed (Exit Code ${exit_status}): Create Task Batch${RESET}"
  FAILURE_COUNT=$((FAILURE_COUNT + 1))
fi
TEST_COUNTER=$((TEST_COUNTER + 1))

# Dependent batch tests
if [ ! -z "$BATCH_ID" ]; then
  run_test "Get Batch Info" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client get-batch --url \"$TARGET_URL\" --id \"$BATCH_ID\""
  run_test "Get Batch Status" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client get-batch-status --url \"$TARGET_URL\" --id \"$BATCH_ID\""
  run_test "Cancel Batch" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client cancel-batch --url \"$TARGET_URL\" --id \"$BATCH_ID\""
else
  echo -e "${YELLOW}⚠️ Skipping tests dependent on successful batch creation.${RESET}"
fi

echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}🛠️ Testing Agent Skills${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"
run_test "List all skills" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client list-skills --url \"$TARGET_URL\""
run_test "List skills with 'text' tag" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client list-skills --url \"$TARGET_URL\" --tags \"text\""
SKILL_ID="test-skill-1" # Assuming this skill exists on the mock server
run_test "Get details for skill '$SKILL_ID'" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client get-skill-details --url \"$TARGET_URL\" --id \"$SKILL_ID\""
run_test "Invoke skill '$SKILL_ID'" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client invoke-skill --url \"$TARGET_URL\" --id \"$SKILL_ID\" --message \"This is a test skill invocation\""
run_test "Invoke skill '$SKILL_ID' with specific modes" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client invoke-skill --url \"$TARGET_URL\" --id \"$SKILL_ID\" --message \"This is a test with specific modes\" -n \"text/plain\" -p \"text/plain\""

echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}🚨 Testing Error Handling${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"
# Test with non-existent task ID to trigger error - expect failure (exit code != 0)
echo -e "${YELLOW}--> [Test $((TEST_COUNTER + 1))] Running: Error handling for non-existent task...${RESET}"
NON_EXISTENT_TASK="task-does-not-exist-$(uuidgen)"
timeout "${DEFAULT_TIMEOUT}s" RUSTFLAGS="-A warnings" cargo run --quiet -- client get-task --url "$TARGET_URL" --id "$NON_EXISTENT_TASK" --header "$AUTH_HEADER" --value "$AUTH_VALUE" > /dev/null 2>&1
exit_status=$?
if [ $exit_status -ne 0 ] && [ $exit_status -ne 124 ]; then
    echo -e "    ${GREEN}✅ [Test $((TEST_COUNTER + 1))] Success: Got expected error for non-existent task${RESET}"
    SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
else
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed: Did not get expected error (Exit Code ${exit_status})${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
fi
TEST_COUNTER=$((TEST_COUNTER + 1))

# Test with non-existent skill ID - expect failure
echo -e "${YELLOW}--> [Test $((TEST_COUNTER + 1))] Running: Error handling for non-existent skill...${RESET}"
NON_EXISTENT_SKILL="skill-does-not-exist-$(uuidgen)"
timeout "${DEFAULT_TIMEOUT}s" RUSTFLAGS="-A warnings" cargo run --quiet -- client get-skill-details --url "$TARGET_URL" --id "$NON_EXISTENT_SKILL" > /dev/null 2>&1
exit_status=$?
if [ $exit_status -ne 0 ] && [ $exit_status -ne 124 ]; then
    echo -e "    ${GREEN}✅ [Test $((TEST_COUNTER + 1))] Success: Got expected error for non-existent skill${RESET}"
    SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
else
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed: Did not get expected error (Exit Code ${exit_status})${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
fi
TEST_COUNTER=$((TEST_COUNTER + 1))

# Test with non-existent batch ID - expect failure
echo -e "${YELLOW}--> [Test $((TEST_COUNTER + 1))] Running: Error handling for non-existent batch...${RESET}"
NON_EXISTENT_BATCH="batch-does-not-exist-$(uuidgen)"
timeout "${DEFAULT_TIMEOUT}s" RUSTFLAGS="-A warnings" cargo run --quiet -- client get-batch --url "$TARGET_URL" --id "$NON_EXISTENT_BATCH" > /dev/null 2>&1
exit_status=$?
if [ $exit_status -ne 0 ] && [ $exit_status -ne 124 ]; then
    echo -e "    ${GREEN}✅ [Test $((TEST_COUNTER + 1))] Success: Got expected error for non-existent batch${RESET}"
    SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
else
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed: Did not get expected error (Exit Code ${exit_status})${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
fi
TEST_COUNTER=$((TEST_COUNTER + 1))


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
FILE_TASK_ID=""
echo -e "${YELLOW}--> [Test $((TEST_COUNTER + 1))] Running: Create Task for File Ops...${RESET}"
FILE_TASK_OUTPUT=$(timeout "${DEFAULT_TIMEOUT}s" RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "$TARGET_URL" --message "Task for file operations" --header "$AUTH_HEADER" --value "$AUTH_VALUE")
exit_status=$?
if [ $exit_status -eq 0 ]; then
  FILE_TASK_ID=$(echo "$FILE_TASK_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
  if [ ! -z "$FILE_TASK_ID" ]; then
    echo -e "    ${GREEN}✅ [Test $((TEST_COUNTER + 1))] Success: Create Task for File Ops (ID: $FILE_TASK_ID)${RESET}"
    SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
  else
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed: Create Task for File Ops (Could not extract ID)${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
  fi
elif [ $exit_status -eq 124 ]; then
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed (Timeout): Create Task for File Ops${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
else
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed (Exit Code ${exit_status}): Create Task for File Ops${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
fi
TEST_COUNTER=$((TEST_COUNTER + 1))


# Upload the file
UPLOADED_FILE_ID=""
if [ ! -z "$FILE_TASK_ID" ]; then
    echo -e "${YELLOW}--> [Test $((TEST_COUNTER + 1))] Running: Upload File...${RESET}"
    UPLOAD_OUTPUT=$(timeout "${DEFAULT_TIMEOUT}s" RUSTFLAGS="-A warnings" cargo run --quiet -- client upload-file --url "$TARGET_URL" --file "$TEST_FILE" --task-id "$FILE_TASK_ID")
    exit_status=$?
    if [ $exit_status -eq 0 ]; then
      UPLOADED_FILE_ID=$(echo "$UPLOAD_OUTPUT" | grep -o '"file_id": "[^"]*"' | head -1 | cut -d'"' -f4)
      if [ ! -z "$UPLOADED_FILE_ID" ]; then
        echo -e "    ${GREEN}✅ [Test $((TEST_COUNTER + 1))] Success: Upload File (ID: $UPLOADED_FILE_ID)${RESET}"
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
      else
        echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed: Upload File (Could not extract ID)${RESET}"
        FAILURE_COUNT=$((FAILURE_COUNT + 1))
      fi
    elif [ $exit_status -eq 124 ]; then
        echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed (Timeout): Upload File${RESET}"
        FAILURE_COUNT=$((FAILURE_COUNT + 1))
    else
        echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed (Exit Code ${exit_status}): Upload File${RESET}"
        FAILURE_COUNT=$((FAILURE_COUNT + 1))
    fi
    TEST_COUNTER=$((TEST_COUNTER + 1))

    run_test "List files for task $FILE_TASK_ID" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client list-files --url \"$TARGET_URL\" --task-id \"$FILE_TASK_ID\""

    # Download the file
    if [ ! -z "$UPLOADED_FILE_ID" ]; then
      DOWNLOAD_PATH="$TEMP_DIR/downloaded_file.txt"
      run_test "Download file $UPLOADED_FILE_ID" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client download-file --url \"$TARGET_URL\" --id \"$UPLOADED_FILE_ID\" --output \"$DOWNLOAD_PATH\""
      # Optional: Verify downloaded content
      # diff "$TEST_FILE" "$DOWNLOAD_PATH" || echo "    Warning: Downloaded file content mismatch"
    else
        echo -e "${YELLOW}⚠️ Skipping Download File test as UPLOADED_FILE_ID was not set.${RESET}"
    fi
else
    echo -e "${YELLOW}⚠️ Skipping Upload/List/Download File tests as FILE_TASK_ID was not set.${RESET}"
fi

run_test "Send Task With File Attachment" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client send-task-with-file --url \"$TARGET_URL\" --message \"Task with file attachment\" --file-path \"$TEST_FILE\""

# Create test JSON data
TEST_DATA_FILE="$TEMP_DIR/test_data.json"
echo -e "${YELLOW}--> Creating test JSON data at $TEST_DATA_FILE...${RESET}"
echo '{"name":"test_data","value":42,"properties":{"color":"blue","active":true}}' > "$TEST_DATA_FILE"

# Send a task with structured data
DATA_TASK_ID=""
echo -e "${YELLOW}--> [Test $((TEST_COUNTER + 1))] Running: Send Task With Data...${RESET}"
DATA_TASK_OUTPUT=$(timeout "${DEFAULT_TIMEOUT}s" RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task-with-data --url "$TARGET_URL" --message "Task with structured data" --data "$(cat $TEST_DATA_FILE)")
exit_status=$?
if [ $exit_status -eq 0 ]; then
  DATA_TASK_ID=$(echo "$DATA_TASK_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
  if [ ! -z "$DATA_TASK_ID" ]; then
    echo -e "    ${GREEN}✅ [Test $((TEST_COUNTER + 1))] Success: Send Task With Data (ID: $DATA_TASK_ID)${RESET}"
    SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
  else
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed: Send Task With Data (Could not extract ID)${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
  fi
elif [ $exit_status -eq 124 ]; then
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed (Timeout): Send Task With Data${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
else
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed (Exit Code ${exit_status}): Send Task With Data${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
fi
TEST_COUNTER=$((TEST_COUNTER + 1))

if [ ! -z "$DATA_TASK_ID" ]; then
  run_test "Get Task Details with Data" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client get-task --url \"$TARGET_URL\" --id \"$DATA_TASK_ID\" --header \"$AUTH_HEADER\" --value \"$AUTH_VALUE\""
else
    echo -e "${YELLOW}⚠️ Skipping Get Task Details with Data test as DATA_TASK_ID was not set.${RESET}"
fi

# Clean up test files
echo -e "${YELLOW}--> Cleaning up temporary directory $TEMP_DIR...${RESET}"
rm -rf "$TEMP_DIR"

echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}🔑 Testing Authentication Validation${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"
run_test "Validate Authentication" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client validate-auth --url \"$TARGET_URL\" --header \"$AUTH_HEADER\" --value \"$AUTH_VALUE\""

# --- Configurable Delays Tests (Unofficial) ---
echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}⏱️ Testing Configurable Delays (Unofficial)${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"

# Mark this section as unofficial
if [ "$RUN_UNOFFICIAL_TESTS" = "false" ]; then
  echo -e "${BLUE}⏭️ Skipping Configurable Delays tests (Use --run-unofficial to include)${RESET}"
else
# Test with a defined delay in metadata
DELAYED_TASK_ID=""
echo -e "${YELLOW}--> [Test $((TEST_COUNTER + 1))] Running: Send Task with 2-second delay...${RESET}"
START_TIME=$(date +%s)
DELAYED_TASK_OUTPUT=$(timeout "${DEFAULT_TIMEOUT}s" RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "$TARGET_URL" \
  --message "Task with configurable delay" \
  --metadata '{"_mock_delay_ms": 2000}' \
  --header "$AUTH_HEADER" --value "$AUTH_VALUE")
exit_status=$?
END_TIME=$(date +%s)
DURATION=$((END_TIME - START_TIME))

if [ $exit_status -eq 0 ]; then
  DELAYED_TASK_ID=$(echo "$DELAYED_TASK_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
  if [ ! -z "$DELAYED_TASK_ID" ]; then
    echo "    Task created with ID: $DELAYED_TASK_ID in approximately $DURATION seconds"
    if [ "$DURATION" -lt "2" ]; then
      echo -e "    ${YELLOW}⚠️ [Test $((TEST_COUNTER + 1))] Success (Warning): Delay might not be supported by this server (Expected >= 2s, got ${DURATION}s).${RESET}"
      SUCCESS_COUNT=$((SUCCESS_COUNT + 1)) # Count as success but warn
    else
      echo -e "    ${GREEN}✅ [Test $((TEST_COUNTER + 1))] Success: Send Task with 2-second delay${RESET}"
      SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
    fi
  else
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed: Send Task with 2-second delay (Could not extract ID)${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
  fi
elif [ $exit_status -eq 124 ]; then
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed (Timeout): Send Task with 2-second delay${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
else
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed (Exit Code ${exit_status}): Send Task with 2-second delay${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
fi
TEST_COUNTER=$((TEST_COUNTER + 1))


# Test streaming with configurable chunk delay
echo -e "${YELLOW}--> [Test $((TEST_COUNTER + 1))] Running: Start Streaming Task with slow chunks...${RESET}"
# Test if the streaming client *starts* successfully
timeout 5s RUSTFLAGS="-A warnings" cargo run --quiet -- client stream-task --url "$TARGET_URL" \
  --message "This is a streaming task with slow chunks" \
  --metadata '{"_mock_chunk_delay_ms": 1000}' &
STREAM_PID=$!
sleep 1 # Give it a moment to potentially fail starting

if ps -p $STREAM_PID > /dev/null; then
   echo -e "    ${GREEN}✅ [Test $((TEST_COUNTER + 1))] Success: Start Streaming Task with slow chunks${RESET}"
   SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
   echo "    Streaming client started (PID $STREAM_PID). Letting it run for a few seconds..."
   sleep 5 # Let it run longer to observe delay
   echo -e "${YELLOW}--> Stopping slow streaming client (PID $STREAM_PID)...${RESET}"
   kill $STREAM_PID 2>/dev/null || true
   wait $STREAM_PID 2>/dev/null || true
else
   echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed: Start Streaming Task with slow chunks${RESET}"
   FAILURE_COUNT=$((FAILURE_COUNT + 1))
fi
TEST_COUNTER=$((TEST_COUNTER + 1))

fi # End of unofficial block for Configurable Delays


# --- Dynamic Streaming Content Tests (Unofficial) ---
echo
echo -e "${BLUE}${BOLD}===============================${RESET}"
echo -e "${BLUE}${BOLD}🌊 Testing Dynamic Streaming Content (Unofficial)${RESET}"
echo -e "${BLUE}${BOLD}===============================${RESET}"

# Mark this section as unofficial
if [ "$RUN_UNOFFICIAL_TESTS" = "false" ]; then
  echo -e "${BLUE}⏭️ Skipping Dynamic Streaming Content tests (Use --run-unofficial to include)${RESET}"
else

# Create a task for resubscribe testing
RESUBSCRIBE_TASK_ID=""
echo -e "${YELLOW}--> [Test $((TEST_COUNTER + 1))] Running: Create Task for Resubscribe...${RESET}"
RESUBSCRIBE_TASK_OUTPUT=$(timeout "${DEFAULT_TIMEOUT}s" RUSTFLAGS="-A warnings" cargo run --quiet -- client send-task --url "$TARGET_URL" \
  --message "Task for resubscribe testing")
exit_status=$?
if [ $exit_status -eq 0 ]; then
  RESUBSCRIBE_TASK_ID=$(echo "$RESUBSCRIBE_TASK_OUTPUT" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
  if [ ! -z "$RESUBSCRIBE_TASK_ID" ]; then
    echo -e "    ${GREEN}✅ [Test $((TEST_COUNTER + 1))] Success: Create Task for Resubscribe (ID: $RESUBSCRIBE_TASK_ID)${RESET}"
    SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
  else
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed: Create Task for Resubscribe (Could not extract ID)${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
  fi
elif [ $exit_status -eq 124 ]; then
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed (Timeout): Create Task for Resubscribe${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
else
    echo -e "    ${RED}❌ [Test $((TEST_COUNTER + 1))] Failed (Exit Code ${exit_status}): Create Task for Resubscribe${RESET}"
    FAILURE_COUNT=$((FAILURE_COUNT + 1))
fi
TEST_COUNTER=$((TEST_COUNTER + 1))


# Test streaming with custom text chunks
run_test "Start Streaming Task with 3 text chunks" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client stream-task --url \"$TARGET_URL\" \
  --message \"Streaming task with 3 text chunks\" \
  --metadata '{\"_mock_stream_text_chunks\": 3, \"_mock_stream_chunk_delay_ms\": 500}' & echo \$! > stream_pid.txt" 5
STREAM_PID=$(cat stream_pid.txt)
rm stream_pid.txt
if [ $? -eq 0 ] && [ ! -z "$STREAM_PID" ]; then sleep 4; kill $STREAM_PID 2>/dev/null || true; wait $STREAM_PID 2>/dev/null || true; else echo -e "${YELLOW}⚠️ Stream client failed to start.${RESET}"; fi

# Test streaming with only data artifacts
run_test "Start Streaming Task with only data artifacts" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client stream-task --url \"$TARGET_URL\" \
  --message \"Streaming task with only data artifacts\" \
  --metadata '{\"_mock_stream_artifact_types\": [\"data\"], \"_mock_stream_chunk_delay_ms\": 500}' & echo \$! > stream_pid.txt" 5
STREAM_PID=$(cat stream_pid.txt)
rm stream_pid.txt
if [ $? -eq 0 ] && [ ! -z "$STREAM_PID" ]; then sleep 3; kill $STREAM_PID 2>/dev/null || true; wait $STREAM_PID 2>/dev/null || true; else echo -e "${YELLOW}⚠️ Stream client failed to start.${RESET}"; fi

# Test streaming with custom final state
run_test "Start Streaming Task with failed final state" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client stream-task --url \"$TARGET_URL\" \
  --message \"Streaming task with failed final state\" \
  --metadata '{\"_mock_stream_final_state\": \"failed\", \"_mock_stream_chunk_delay_ms\": 500}' & echo \$! > stream_pid.txt" 5
STREAM_PID=$(cat stream_pid.txt)
rm stream_pid.txt
if [ $? -eq 0 ] && [ ! -z "$STREAM_PID" ]; then sleep 3; kill $STREAM_PID 2>/dev/null || true; wait $STREAM_PID 2>/dev/null || true; else echo -e "${YELLOW}⚠️ Stream client failed to start.${RESET}"; fi

# Test resubscribe with dynamic configuration
if [ ! -z "$RESUBSCRIBE_TASK_ID" ]; then
  run_test "Start Resubscribe Task with dynamic configuration" "RUSTFLAGS=\"-A warnings\" cargo run --quiet -- client resubscribe-task --url \"$TARGET_URL\" \
    --id \"$RESUBSCRIBE_TASK_ID\" \
    --metadata '{\"_mock_stream_text_chunks\": 2, \"_mock_stream_artifact_types\": [\"text\", \"data\"]}' & echo \$! > stream_pid.txt" 5
  STREAM_PID=$(cat stream_pid.txt)
  rm stream_pid.txt
  if [ $? -eq 0 ] && [ ! -z "$STREAM_PID" ]; then sleep 4; kill $STREAM_PID 2>/dev/null || true; wait $STREAM_PID 2>/dev/null || true; else echo -e "${YELLOW}⚠️ Stream client failed to start.${RESET}"; fi
else
  echo -e "${YELLOW}⚠️ Skipping resubscribe test as task creation failed earlier.${RESET}"
fi

fi # End of unofficial block for Dynamic Streaming

# --- Test Summary ---
echo
echo -e "${BLUE}${BOLD}======================================================${RESET}"
echo -e "${BLUE}${BOLD}📊 Test Summary${RESET}"
echo -e "${BLUE}${BOLD}======================================================${RESET}"
echo -e "Total Tests Attempted: ${BOLD}${TEST_COUNTER}${RESET}"
echo -e "${GREEN}Successful Tests: ${BOLD}${SUCCESS_COUNT}${RESET}"
echo -e "${RED}Failed Tests: ${BOLD}${FAILURE_COUNT}${RESET}"
if [ "$RUN_UNOFFICIAL_TESTS" = "true" ]; then
  echo -e "${YELLOW}(Including unofficial tests)${RESET}"
else
  echo -e "${BLUE}(Unofficial tests were skipped. Use --run-unofficial to include them)${RESET}"
fi
echo -e "${BLUE}${BOLD}======================================================${RESET}"

# Server (if started locally) will be killed by trap handler

# Exit with non-zero status if any tests failed
if [ $FAILURE_COUNT -gt 0 ]; then
    echo -e "${RED}${BOLD}Some tests failed!${RESET}"
    exit 1
else
    echo -e "${GREEN}${BOLD}All tests passed!${RESET}"
    exit 0
fi
