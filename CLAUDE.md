# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands
- Build: `cargo build`
- Run: `cargo run -- [subcommand]`
- Test all: `cargo test`
- Test single: `cargo test [test_name]`
- Property tests: `cargo run -- test --cases [number]`
- Validate: `cargo run -- validate --file [path]`
- Mock server: `cargo run -- server --port [port]`
- Fuzzing: `cargo run -- fuzz --target [target] --time [seconds]`

## Code Style Guidelines
- Follow standard Rust formatting with 4-space indentation
- Group imports: external crates first, then internal modules
- Use snake_case for functions/variables, CamelCase for types
- Error handling: Use Result types with descriptive messages and `?` operator
- Document public functions with /// comments
- Follow existing validator/property test patterns for new test implementations
- Use strong typing and avoid `unwrap()` without error handling