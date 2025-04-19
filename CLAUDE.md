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

## Documentation
- **README.md**: Overview of A2A protocol and test suite components
- **docs/A2A_dev_docs.md**: Technical documentation for A2A protocol developers
- **testing_plan.md**: Comprehensive plan for testing A2A server implementations

## Build & Test Commands
- Build: `cargo build --quiet`
- Build (no warnings): `RUSTFLAGS="-A warnings" cargo build`
- Run: `cargo run --quiet -- [subcommand]`
- Test all: `cargo test --quiet`
- Test single: `cargo test --quiet [test_name]`
- Property tests: `cargo run --quiet -- test --cases [number]`
- Validate: `cargo run --quiet -- validate --file [path]`
- Mock server: `cargo run --quiet -- server --port [port]`
- Fuzzing: `cargo run --quiet -- fuzz --target [target] --time [seconds]`
- Client commands:
  - Get agent card: `cargo run --quiet -- client get-agent-card --url [url]`
  - Send task: `cargo run --quiet -- client send-task --url [url] --message [text]`
  - Get task: `cargo run --quiet -- client get-task --url [url] --id [task_id]`
  - Cancel task: `cargo run --quiet -- client cancel-task --url [url] --id [task_id]`
  - Stream task: `cargo run --quiet -- client stream-task --url [url] --message [text]` 
  - Resubscribe: `cargo run --quiet -- client resubscribe-task --url [url] --id [task_id]`
  - Set push notification: `cargo run --quiet -- client set-push-notification --url [url] --id [task_id] --webhook [url] --auth-scheme [scheme] --token [token]`
  - Get push notification: `cargo run --quiet -- client get-push-notification --url [url] --id [task_id]`
- Run integration tests: `./start_server_and_test_client.sh`
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
- **src/mock_server.rs**: Reference A2A server implementation 
- **src/fuzzer.rs**: Fuzzing tools for A2A message handlers
- **src/types.rs**: A2A protocol type definitions
- **src/client/**: A2A client implementation
  - **src/client/mod.rs**: Core client structure and common functionality
  - **src/client/cancel_task.rs**: Task cancellation implementation
  - **src/client/streaming.rs**: Streaming task support (SSE)
  - **src/client/push_notifications.rs**: Push notification API support
  - **src/client/tests.rs**: Client unit tests
- **src/client_tests.rs**: Client integration tests
- **start_server_and_test_client.sh**: Script for running integration tests