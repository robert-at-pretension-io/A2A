# Using A2A Test Suite with Docker

This guide explains how to use the A2A Test Suite Docker image to test your A2A server implementation.

## Quick Start

You can run the A2A Test Suite in a Docker container without having to install Rust or build the binaries yourself.

### Building the Docker Image

Clone the repository and build the Docker image:

```bash
git clone https://github.com/your-org/a2a-test-suite.git
cd a2a-test-suite
docker build -t a2a-test-suite .
```

### Running Tests Against Your Server

```bash
docker run --network=host a2a-test-suite run-tests --url http://your-server-url
```

> Note: If your server is running on localhost, using `--network=host` allows the container to access it.
> If your server is on a different host, you can simply use the full URL.

### Adding Options

You can pass any options supported by the test suite:

```bash
# Run unofficial tests
docker run a2a-test-suite run-tests --url http://your-server-url --run-unofficial

# Set custom timeout
docker run a2a-test-suite run-tests --url http://your-server-url --timeout 30
```

## Using Docker Compose

For a more complete testing environment, you can use Docker Compose to run both the A2A Test Suite and your server.

Create a `docker-compose.yml` file:

```yaml
version: '3'

services:
  a2a-server:
    image: your-a2a-server-image
    ports:
      - "8080:8080"
    # Add any environment variables or configuration your server needs
    
  a2a-test-suite:
    build: .
    command: run-tests --url http://a2a-server:8080
    depends_on:
      - a2a-server
```

Then run:

```bash
docker-compose up
```

## Running the Mock Server

To run the A2A Test Suite's mock server in Docker:

```bash
docker run -p 8080:8080 a2a-test-suite server --port 8080
```

This starts the mock server on port 8080, which you can then use for development or testing.

## Testing Against the Mock Server

You can run both the mock server and tests in separate containers:

```bash
# First terminal: Start the mock server
docker run --name a2a-mock-server -p 8080:8080 a2a-test-suite server --port 8080

# Second terminal: Run tests against it
docker run --network=host a2a-test-suite run-tests --url http://localhost:8080
```

## Troubleshooting

### Network Issues

If your tests can't connect to your server:

1. Check that your server is accessible from the Docker container
2. If your server is on localhost, use `--network=host` 
3. If your server is in another container, ensure they're on the same Docker network

### Test Failures

If tests are failing:

1. Check the error messages for specific protocol compliance issues
2. Verify your server's URL is correct
3. Ensure your server implementation follows the A2A protocol specification

## Advanced Usage

For more advanced usage, including manual testing of specific endpoints, use the client commands:

```bash
# Get agent card
docker run a2a-test-suite client get-agent-card --url http://your-server-url

# Send a task
docker run a2a-test-suite client send-task --url http://your-server-url --message "Test task"
```