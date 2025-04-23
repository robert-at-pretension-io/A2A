# CLAUDE.md

This file provides guidance to Claude Code (or any other AI tool -- aider, cursor, continue, etc) when working with code in this repository.

## Project Purpose
The A2A Test Suite is a comprehensive testing framework for the Agent-to-Agent (A2A) protocol, which enables standardized communication between AI agent systems. The project provides validation, property testing, mock server, client implementation, and fuzzing tools to ensure protocol compliance.

## Development Methodology

**IMPORTANT: THIS PROJECT MUST ALWAYS FOLLOW TEST-DRIVEN DEVELOPMENT**

- Write tests before implementing features
- Run `cargo build` often to verify the feature is building
- Implement features "slowly" and meticulously
- Always verify tests pass before considering work complete
- Ensure builds work with `cargo test && cargo build` before submitting changes
- Never skip testing or make untested changes
- Implement features iteratively: small, testable units
- Keep the `start_server_and_test_client.sh` script updated with new features for end-to-end testing

## Documentation
- **README.md**: Overview of A2A protocol and test suite components
- **docs/schema_overview.md**: Detailed A2A protocol schema documentation
- **src/client/README.md**: Comprehensive client feature documentation and examples
- **src/client/tests/integration_test.rs**: Example code demonstrating client features

## Build & Test Commands
- Generate schema types: `cargo run --quiet -- config generate-types`
- Set schema version: `cargo run --quiet -- config set-schema-version [version]`
- Build: `cargo build --quiet`
- Build (no warnings): `RUSTFLAGS="-A warnings" cargo build`
- Run: `cargo run --quiet -- [subcommand]`
- Test all: `cargo test --quiet`
- Test single: `cargo test --quiet [test_name]`
- Property tests: `cargo run --quiet -- test --cases [number]`
- Validate: `cargo run --quiet -- validate --file [path]`
- Mock server: `cargo run --quiet -- server --port [port]`
- Reference server: `cargo run --quiet -- reference-server --port [port]`
- Fuzzing: `cargo run --quiet -- fuzz --target [target] --time [seconds]`
- Run integration tests: `cargo run --quiet -- run-tests`
- Client commands:
  - Get agent card: `cargo run --quiet -- client get-agent-card --url [url]`
  - Send task: `cargo run --quiet -- client send-task --url [url] --message [text] [--metadata '{"_mock_delay_ms": 2000}'] [--header "header_name"] [--value "auth_value"]`
  - Send task with simulated state machine: `cargo run --quiet -- client send-task --url [url] --message [text] --metadata '{"_mock_duration_ms": 5000, "_mock_require_input": true}'`
  - Send task with file: `cargo run --quiet -- client send-task-with-file --url [url] --message [text] --file-path [path]`
  - Send task with data: `cargo run --quiet -- client send-task-with-data --url [url] --message [text] --data [json]`
  - Get task: `cargo run --quiet -- client get-task --url [url] --id [task_id] [--header "header_name"] [--value "auth_value"]`
  - Get artifacts: `cargo run --quiet -- client get-artifacts --url [url] --id [task_id] --output-dir [dir]`
  - Cancel task: `cargo run --quiet -- client cancel-task --url [url] --id [task_id] [--header "header_name"] [--value "auth_value"]`
  - Validate auth: `cargo run --quiet -- client validate-auth --url [url] --header "header_name" --value "auth_value"`
  - Stream task: `cargo run --quiet -- client stream-task --url [url] --message [text] [--metadata '{"_mock_chunk_delay_ms": 1000}']` 
  - Stream with dynamic content: `cargo run --quiet -- client stream-task --url [url] --message [text] --metadata '{"_mock_stream_text_chunks": 5, "_mock_stream_artifact_types": ["text", "data"]}'`
  - Resubscribe: `cargo run --quiet -- client resubscribe-task --url [url] --id [task_id] [--metadata '{"_mock_stream_final_state": "failed"}']`
  - Set push notification: `cargo run --quiet -- client set-push-notification --url [url] --id [task_id] --webhook [url] --auth-scheme [scheme] --token [token]`
  - Get push notification: `cargo run --quiet -- client get-push-notification --url [url] --id [task_id]`
  - Get state history: `cargo run --quiet -- client get-state-history --url [url] --id [task_id]`
  - Get state metrics: `cargo run --quiet -- client get-state-metrics --url [url] --id [task_id]`
  - Create task batch: `cargo run --quiet -- client create-batch --url [url] --tasks "task 1,task 2,task 3" --name [batch_name]`
  - Get batch: `cargo run --quiet -- client get-batch --url [url] --id [batch_id]`
  - Get batch status: `cargo run --quiet -- client get-batch-status --url [url] --id [batch_id]`
  - Cancel batch: `cargo run --quiet -- client cancel-batch --url [url] --id [batch_id]`
  - List skills: `cargo run --quiet -- client list-skills --url [url] --tags [optional_tags]`
  - Get skill details: `cargo run --quiet -- client get-skill-details --url [url] --id [skill_id]`
  - Invoke skill: `cargo run --quiet -- client invoke-skill --url [url] --id [skill_id] --message [text] --input-mode [optional_mode] --output-mode [optional_mode] [--metadata '{"_mock_duration_ms": 3000}']`
- **REQUIRED VERIFICATION**: Always run `RUSTFLAGS="-A warnings" cargo test && cargo build` before finalizing changes

## Code Style Guidelines
- Follow standard Rust formatting with 4-space indentation
- Group imports: external crates first, then internal modules
- Use snake_case for functions/variables, CamelCase for types
- Error handling: Use Result types with descriptive messages and `?` operator
- Document public functions with /// comments
- Follow existing validator/property test patterns for new test implementations
- Use strong typing and avoid `unwrap()` without error handling
- Implement client features as modular extensions in separate files

## Project Structure
- **src/validator.rs**: A2A message JSON schema validation
- **src/property_tests.rs**: Property-based testing using proptest
- **src/mock_server.rs**: Mock A2A server implementation with configurable network delay simulation, state machine fidelity, and dynamic streaming content
- **src/server/**: Clean reference A2A server implementation following best practices
  - **src/server/mod.rs**: Core server setup and entrypoint
  - **src/server/error.rs**: Server error types and handling
  - **src/server/repositories/**: Data storage layer
  - **src/server/services/**: Business logic layer
  - **src/server/handlers/**: Request handling and routing
- **src/fuzzer.rs**: Fuzzing tools for A2A message handlers
- **src/types.rs**: A2A protocol type definitions
- **src/client/**: A2A client implementation
  - **src/client/mod.rs**: Core client structure and common functionality
  - **src/client/cancel_task.rs**: Task cancellation implementation
  - **src/client/streaming.rs**: Streaming task support (SSE)
  - **src/client/push_notifications.rs**: Push notification API support
  - **src/client/file_operations.rs**: File attachment and binary data handling
  - **src/client/data_operations.rs**: Structured data operations
  - **src/client/artifacts.rs**: Artifact management and processing
  - **src/client/state_history.rs**: State transition history tracking and analysis
  - **src/client/task_batch.rs**: Batch operations for managing multiple tasks
  - **src/client/agent_skills.rs**: Agent skills discovery and invocation
  - **src/client/auth.rs**: Authentication and authorization support
  - **src/client/tests/**: Client unit tests
- **src/client_tests.rs**: Client integration tests
- **start_server_and_test_client.sh**: Script for running integration tests

## Optimal Feature Development Workflow

1. **Study Schema First**: Review `docs/schema_overview.md` to understand the protocol's data model for your feature.

2. **Planning Phase**:
   - Define the feature's scope and API surface (function names, parameters)
   - Identify required data structures and client/server interactions
   - Plan for both happy path and error cases

3. **Test-Driven Development**:
   - Start with a unit test in the relevant module's `tests` mod
   - Add an integration test in `src/client/tests/integration_test.rs`
   - Tests should be failing at this point (RED)

4. **Implementation Steps**:
   1. Create a new module file for feature-specific code
   2. Add the module to `client/mod.rs`
   3. Implement client methods and data structures
   4. Update the mock server in `mock_server.rs` to support the feature
   5. Run tests (`cargo test --quiet`) and iterate until passing (GREEN)

5. **Validation and Refinement**:
   - Verify with `RUSTFLAGS="-A warnings" cargo test && cargo build`
   - Add CLI commands in `main.rs` if needed
   - Update CLAUDE.md with new commands and module descriptions
   - Document the feature in `src/client/README.md` with examples
   - Update `start_server_and_test_client.sh` if your feature requires special setup or teardown

6. **Parallel Testing Tip**: Use `--test-threads=1` for tests that use the mock server to avoid port conflicts:
   ```
   cargo test -- --test-threads=1
   ```

7. **Debugging Tips**:
   - Use `println!("Debug: {:?}", variable)` in tests for visibility
   - Set up test mock server with unique ports for each test
   - For complex features, implement and test small parts incrementally