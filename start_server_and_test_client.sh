#!/bin/bash

# This script starts the mock server and then runs a series of client commands to test functionality

# Exit on any error
set -e

echo "===============================" 
echo "Starting Mock A2A Server"
echo "==============================="

# Start the server in the background
cargo run --quiet -- server --port 8080 &
SERVER_PID=$!

# Give the server time to start
sleep 1

# Ensure we kill the server when the script exits
trap "kill $SERVER_PID" EXIT

echo "===============================" 
echo "Testing Client Functions"
echo "==============================="

# Get the agent card
echo "Getting Agent Card..."
cargo run --quiet -- client get-agent-card --url "http://localhost:8080"

# Send a basic task
echo "Sending Task..."
TASK_ID=$(cargo run --quiet -- client send-task --url "http://localhost:8080" --message "This is a test task" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$TASK_ID" ]; then
  echo "Task created with ID: $TASK_ID"
  
  # Get the task details
  echo "Getting Task Details..."
  cargo run --quiet -- client get-task --url "http://localhost:8080" --id "$TASK_ID"
  
  # Get task state history
  echo "Getting Task State History..."
  cargo run --quiet -- client get-state-history --url "http://localhost:8080" --id "$TASK_ID"
  
  # Get task state metrics
  echo "Getting Task State Metrics..."
  cargo run --quiet -- client get-state-metrics --url "http://localhost:8080" --id "$TASK_ID"
  
  # Cancel the task
  echo "Cancelling Task..."
  cargo run --quiet -- client cancel-task --url "http://localhost:8080" --id "$TASK_ID"
  
  # Get the task details after cancellation
  echo "Getting Task Details After Cancellation..."
  cargo run --quiet -- client get-task --url "http://localhost:8080" --id "$TASK_ID"
fi

# Test streaming
echo "===============================" 
echo "Testing Streaming"
echo "==============================="
# Note that we can't easily capture the streaming output
cargo run --quiet -- client stream-task --url "http://localhost:8080" --message "This is a streaming task" &
STREAM_PID=$!

# Let it run for a bit
sleep 3

# Kill the streaming client
kill $STREAM_PID 2>/dev/null || true

# Test push notifications
echo "===============================" 
echo "Testing Push Notifications"
echo "==============================="
cargo run --quiet -- client set-push-notification --url "http://localhost:8080" --id "$TASK_ID" --webhook "https://example.com/webhook" --auth-scheme "Bearer" --token "test-token"

cargo run --quiet -- client get-push-notification --url "http://localhost:8080" --id "$TASK_ID"

echo "===============================" 
echo "Testing Task Batching"
echo "==============================="
# Create a batch
BATCH_ID=$(cargo run --quiet -- client create-batch --url "http://localhost:8080" --tasks "Task 1,Task 2,Task 3" --name "Test Batch" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$BATCH_ID" ]; then
  # Get batch info
  cargo run --quiet -- client get-batch --url "http://localhost:8080" --id "$BATCH_ID"
  
  # Get batch status
  cargo run --quiet -- client get-batch-status --url "http://localhost:8080" --id "$BATCH_ID"
  
  # Cancel batch
  cargo run --quiet -- client cancel-batch --url "http://localhost:8080" --id "$BATCH_ID"
fi

echo "===============================" 
echo "Testing Agent Skills"
echo "==============================="
# List all skills
echo "Listing all skills..."
cargo run --quiet -- client list-skills --url "http://localhost:8080"

# List skills filtered by tag
echo "Listing skills with 'text' tag..."
cargo run --quiet -- client list-skills --url "http://localhost:8080" --tags "text"

# Get details for a specific skill
echo "Getting details for a specific skill..."
SKILL_ID="test-skill-1"
cargo run --quiet -- client get-skill-details --url "http://localhost:8080" --id "$SKILL_ID"

# Invoke a skill
echo "Invoking skill..."
cargo run --quiet -- client invoke-skill --url "http://localhost:8080" --id "$SKILL_ID" --message "This is a test skill invocation"

# Invoke a skill with specific input/output modes
echo "Invoking skill with specific input/output modes..."
cargo run --quiet -- client invoke-skill --url "http://localhost:8080" --id "$SKILL_ID" --message "This is a test with specific modes" -n "text/plain" -p "text/plain"

echo "===============================" 
echo "Testing File Operations"
echo "==============================="

# Create a test file
echo "Creating test file..."
echo "This is a test file content for the A2A File Operations API." > test_file.txt

# Upload the file
echo "Uploading a file to the server..."
FILE_UPLOAD_OUTPUT=$(cargo run --quiet -- client upload-file --url "http://localhost:8080" --file "test_file.txt")
echo "$FILE_UPLOAD_OUTPUT"

# Extract file ID from upload response
FILE_ID=$(echo "$FILE_UPLOAD_OUTPUT" | grep "File ID:" | awk '{print $3}')
echo "Uploaded file ID: $FILE_ID"

# List all files
echo "Listing all files on the server..."
cargo run --quiet -- client list-files --url "http://localhost:8080"

# Download the file
echo "Downloading the file from the server..."
cargo run --quiet -- client download-file --url "http://localhost:8080" --id "$FILE_ID" --output "downloaded_file.txt"

# Verify file content
echo "Verifying downloaded file content..."
ORIGINAL_CONTENT=$(cat test_file.txt)
DOWNLOADED_CONTENT=$(cat downloaded_file.txt)

if [ "$ORIGINAL_CONTENT" = "$DOWNLOADED_CONTENT" ]; then
    echo "✅ File content matches!"
else
    echo "❌ File content does not match!"
    echo "Original content: $ORIGINAL_CONTENT"
    echo "Downloaded content: $DOWNLOADED_CONTENT"
    exit 1
fi

# Create a task with a file attachment
echo "Creating a task with a file attachment..."
TASK_WITH_FILE=$(cargo run --quiet -- client send-task-with-file --url "http://localhost:8080" --message "Task with file attachment" --file-path "test_file.txt")
echo "$TASK_WITH_FILE"
TASK_FILE_ID=$(echo "$TASK_WITH_FILE" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
echo "Created task with ID: $TASK_FILE_ID"

# Upload a file and associate it with the task
echo "Uploading a file associated with a task..."
cargo run --quiet -- client upload-file --url "http://localhost:8080" --file "test_file.txt" --task-id "$TASK_FILE_ID"

# List files for the specific task
echo "Listing files for the specific task..."
cargo run --quiet -- client list-files --url "http://localhost:8080" --task-id "$TASK_FILE_ID"

# Clean up test files
echo "Cleaning up test files..."
rm -f test_file.txt downloaded_file.txt

echo "===============================" 
echo "Testing Data Operations"
echo "==============================="

# Create a test JSON data
echo "Creating test JSON data..."
echo '{"name":"test_data","value":42,"properties":{"color":"blue","active":true}}' > test_data.json

# Send a task with structured data
echo "Sending a task with structured data..."
DATA_TASK_ID=$(cargo run --quiet -- client send-task-with-data --url "http://localhost:8080" --message "Task with structured data" --data "$(cat test_data.json)" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$DATA_TASK_ID" ]; then
  echo "Created task with data, ID: $DATA_TASK_ID"
  
  # Get the task details to verify data was attached
  echo "Getting Task Details with Data..."
  cargo run --quiet -- client get-task --url "http://localhost:8080" --id "$DATA_TASK_ID"
fi

# Clean up data test files
echo "Cleaning up data test files..."
rm -f test_data.json

echo "===============================" 
echo "All Tests Completed Successfully"
echo "==============================="

# Server will be killed by trap handler