#!/bin/bash

# Script to run the bidirectional agent with an API key

# Check if API key is provided
if [ -z "$CLAUDE_API_KEY" ]; then
    echo "CLAUDE_API_KEY environment variable is not set."
    echo "Please either:"
    echo "1. Export your API key: export CLAUDE_API_KEY=your-api-key"
    echo "2. Set it inline: CLAUDE_API_KEY=your-api-key ./run_bidirectional.sh"
    echo "3. Or uncomment and set claude_api_key in bidirectional_agent.toml"
    exit 1
fi

# Build the agent if it doesn't exist
if [ ! -f "./target/debug/bidirectional-agent" ]; then
    echo "Building bidirectional agent..."
    RUSTFLAGS="-A warnings" cargo build --bin bidirectional-agent
fi

# Run the agent with any passed arguments
echo "Starting bidirectional agent with Claude API key..."
./target/debug/bidirectional-agent "$@"