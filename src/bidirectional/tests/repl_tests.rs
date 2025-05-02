// Note: Testing interactive REPL functionality is challenging
// These tests focus on the command parsing logic that could be extracted
// from the REPL implementation for better testability

use crate::bidirectional::bidirectional_agent::{
    BidirectionalAgentConfig, ServerConfig, ClientConfig, LlmConfig, ModeConfig
};
use std::sync::Arc;
use std::io::Cursor;

// Helper functions that would be useful for testing the REPL

fn parse_repl_command(cmd: &str) -> (&str, Option<&str>) {
    if !cmd.starts_with(':') {
        return ("message", Some(cmd));
    }
    
    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
    let command = parts[0].trim_start_matches(':');
    let args = parts.get(1).map(|&s| s);
    
    (command, args)
}

#[test]
fn test_parse_repl_command() {
    // Test help command
    let (cmd, args) = parse_repl_command(":help");
    assert_eq!(cmd, "help");
    assert_eq!(args, None);
    
    // Test card command
    let (cmd, args) = parse_repl_command(":card");
    assert_eq!(cmd, "card");
    assert_eq!(args, None);
    
    // Test connect command with URL
    let (cmd, args) = parse_repl_command(":connect http://example.com");
    assert_eq!(cmd, "connect");
    assert_eq!(args, Some("http://example.com"));
    
    // Test remote command with message
    let (cmd, args) = parse_repl_command(":remote Hello world");
    assert_eq!(cmd, "remote");
    assert_eq!(args, Some("Hello world"));
    
    // Test direct message (not a command)
    let (cmd, args) = parse_repl_command("What is the capital of France?");
    assert_eq!(cmd, "message");
    assert_eq!(args, Some("What is the capital of France?"));
}

// In a real implementation, we would want to test the actual REPL functionality
// by mocking stdin/stdout and providing controlled input/output. This would require
// refactoring the REPL implementation to be more testable.

// Here's an outline of tests that would be valuable:
// - test_repl_help_command - Verify help text is displayed
// - test_repl_card_command - Verify agent card is displayed
// - test_repl_connect_command - Verify connecting to a remote agent
// - test_repl_remote_command - Verify sending a task to a remote agent
// - test_repl_direct_message - Verify processing a direct message
// - test_repl_quit_command - Verify exiting the REPL

// For now, we'll focus on the command parsing logic that can be extracted
// and tested independently of the REPL implementation.

#[test]
fn test_server_commands() {
    // Test listen command parsing
    let (cmd, args) = parse_repl_command(":listen 8080");
    assert_eq!(cmd, "listen");
    assert_eq!(args, Some("8080"));
    
    // Test stop command parsing
    let (cmd, args) = parse_repl_command(":stop");
    assert_eq!(cmd, "stop");
    assert_eq!(args, None);
}

#[test]
fn test_server_management_commands() {
    // Test servers command parsing
    let (cmd, args) = parse_repl_command(":servers");
    assert_eq!(cmd, "servers");
    assert_eq!(args, None);
    
    // Test disconnect command parsing
    let (cmd, args) = parse_repl_command(":disconnect");
    assert_eq!(cmd, "disconnect");
    assert_eq!(args, None);
}

#[test]
fn test_connect_variations() {
    // Test connect with URL
    let (cmd, args) = parse_repl_command(":connect http://example.com");
    assert_eq!(cmd, "connect");
    assert_eq!(args, Some("http://example.com"));
    
    // Test connect with number
    let (cmd, args) = parse_repl_command(":connect 1");
    assert_eq!(cmd, "connect");
    assert_eq!(args, Some("1"));
}

// Additional tests that would be valuable if the REPL was refactored for better testability:
// - test_repl_command_history
// - test_repl_known_servers_tracking
// - test_repl_server_autostart
// - test_repl_error_handling