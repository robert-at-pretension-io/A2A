#!/bin/bash

# Kill any existing server process
pkill -f "a2a-test-suite server"

# Start the mock server in the background
cargo run -- server --port 8080 &
SERVER_PID=$!

# Give the server a moment to start
sleep 2

echo "==============================="
echo "Testing Agent Card Retrieval"
echo "==============================="
cargo run -- client get-agent-card --url "http://localhost:8080/.well-known/agent.json"

echo "==============================="
echo "Testing Task Sending"
echo "==============================="
cargo run -- client send-task --url "http://localhost:8080" --message "Hello from test script"

# Get a task ID for further testing
TASK_ID=$(cargo run -- client send-task --url "http://localhost:8080" --message "Get task ID" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)
  
if [ ! -z "$TASK_ID" ]; then
  echo "==============================="
  echo "Testing Task Retrieval"
  echo "==============================="
  cargo run -- client get-task --url "http://localhost:8080" --id "$TASK_ID"
  
  echo "==============================="
  echo "Testing Task Cancellation"
  echo "==============================="
  cargo run -- client cancel-task --url "http://localhost:8080" --id "$TASK_ID"
fi

echo "==============================="
echo "Testing Streaming"
echo "==============================="
# This will timeout after a few seconds, which is fine for testing
timeout 5s cargo run -- client stream-task --url "http://localhost:8080" --message "Streaming test" || true

echo "==============================="
echo "Testing Resubscribe"
echo "==============================="
# Generate a task ID for resubscribe testing
STREAM_TASK_ID="stream-task-example"
# This will timeout after a few seconds, which is fine for testing
timeout 5s cargo run -- client resubscribe-task --url "http://localhost:8080" --id "$STREAM_TASK_ID" || true

echo "==============================="
echo "Testing Push Notification API"
echo "==============================="
# Set push notification for a task
cargo run -- client set-push-notification --url "http://localhost:8080" --id "$TASK_ID" --webhook "https://example.com/webhook" --auth-scheme "Bearer" --token "test-token"
# Get push notification config
cargo run -- client get-push-notification --url "http://localhost:8080" --id "$TASK_ID"

echo "==============================="
echo "Testing Task State History"
echo "==============================="
cargo run -- client get-state-history --url "http://localhost:8080" --id "$TASK_ID"
cargo run -- client get-state-metrics --url "http://localhost:8080" --id "$TASK_ID"

echo "==============================="
echo "Testing Task Batching"
echo "==============================="
# Note: These commands require implementation in main.rs
# Create a batch
BATCH_ID=$(cargo run -- client create-batch --url "http://localhost:8080" --tasks "Task 1,Task 2,Task 3" --name "Test Batch" | grep -o '"id": "[^"]*"' | head -1 | cut -d'"' -f4)

if [ ! -z "$BATCH_ID" ]; then
  # Get batch info
  cargo run -- client get-batch --url "http://localhost:8080" --id "$BATCH_ID"
  
  # Get batch status
  cargo run -- client get-batch-status --url "http://localhost:8080" --id "$BATCH_ID"
  
  # Cancel batch
  cargo run -- client cancel-batch --url "http://localhost:8080" --id "$BATCH_ID"
fi

# Kill the server when done
kill $SERVER_PID
echo "Server stopped."