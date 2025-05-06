use crate::bidirectional::config::{
    BidirectionalAgentConfig, ClientConfig, LlmConfig, ModeConfig, ServerConfig,
};
use crate::bidirectional::tests::mocks::MockLlmClient;
use crate::bidirectional::BidirectionalAgent;
use crate::server::repositories::task_repository::{InMemoryTaskRepository, TaskRepository};
use crate::server::services::task_service::TaskService;
use crate::types::{Message, Part, Role, Task, TaskState, TaskStatus, TextPart};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

// Helper function to parse REPL commands (copied from repl_tests.rs)
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
fn test_new_session_command() {
    // Test session new command
    let (cmd, args) = parse_repl_command(":session new");
    assert_eq!(cmd, "session");
    assert_eq!(args, Some("new"));
}

#[test]
fn test_show_session_command() {
    // Test session show command
    let (cmd, args) = parse_repl_command(":session show");
    assert_eq!(cmd, "session");
    assert_eq!(args, Some("show"));
}

#[test]
fn test_history_command() {
    // Test history command
    let (cmd, args) = parse_repl_command(":history");
    assert_eq!(cmd, "history");
    assert_eq!(args, None);
}

#[test]
fn test_tasks_command() {
    // Test tasks command
    let (cmd, args) = parse_repl_command(":tasks");
    assert_eq!(cmd, "tasks");
    assert_eq!(args, None);
}

#[test]
fn test_task_command() {
    // Test task command with ID
    let (cmd, args) = parse_repl_command(":task 123456");
    assert_eq!(cmd, "task");
    assert_eq!(args, Some("123456"));
}

#[test]
fn test_artifacts_command() {
    // Test artifacts command with ID
    let (cmd, args) = parse_repl_command(":artifacts 123456");
    assert_eq!(cmd, "artifacts");
    assert_eq!(args, Some("123456"));
}

#[test]
fn test_cancel_task_command() {
    // Test cancelTask command with ID
    let (cmd, args) = parse_repl_command(":cancelTask 123456");
    assert_eq!(cmd, "cancelTask");
    assert_eq!(args, Some("123456"));
}

#[test]
fn test_file_command() {
    // Test file command with path and message
    let (cmd, args) = parse_repl_command(":file /path/to/file.txt This is a message with file");
    assert_eq!(cmd, "file");
    assert_eq!(args, Some("/path/to/file.txt This is a message with file"));
}

#[test]
fn test_data_command() {
    // Test data command with JSON and message
    let (cmd, args) = parse_repl_command(":data {\"name\":\"test\"} This is a message with data");
    assert_eq!(cmd, "data");
    assert_eq!(
        args,
        Some("{\"name\":\"test\"} This is a message with data")
    );
}

// Helper struct to mock a minimal REPL command processor
struct ReplCommandProcessor {
    commands_executed: HashMap<String, usize>,
    session_id: Option<String>,
}

impl ReplCommandProcessor {
    fn new() -> Self {
        Self {
            commands_executed: HashMap::new(),
            session_id: None,
        }
    }

    fn process_command(&mut self, input: &str) -> String {
        let (command, args) = parse_repl_command(input);

        match command {
            "session" => {
                if let Some(subcommand) = args {
                    if subcommand == "new" {
                        self.commands_executed
                            .entry("session_new".to_string())
                            .and_modify(|count| *count += 1)
                            .or_insert(1);

                        let session_id = format!("session-{}", Uuid::new_v4());
                        self.session_id = Some(session_id.clone());
                        return format!("✅ Created new session: {}", session_id);
                    } else if subcommand == "show" {
                        self.commands_executed
                            .entry("session_show".to_string())
                            .and_modify(|count| *count += 1)
                            .or_insert(1);

                        if let Some(ref session_id) = self.session_id {
                            return format!("🔍 Current session: {}", session_id);
                        } else {
                            return "⚠️ No active session. Use :session new to create one."
                                .to_string();
                        }
                    }
                }
            }
            "history" => {
                self.commands_executed
                    .entry("history".to_string())
                    .and_modify(|count| *count += 1)
                    .or_insert(1);

                if self.session_id.is_some() {
                    return "📝 Session History: (empty)".to_string();
                } else {
                    return "⚠️ No active session. Use :session new to create one.".to_string();
                }
            }
            "tasks" => {
                self.commands_executed
                    .entry("tasks".to_string())
                    .and_modify(|count| *count += 1)
                    .or_insert(1);

                if self.session_id.is_some() {
                    return "📋 Tasks in Current Session: (none)".to_string();
                } else {
                    return "⚠️ No active session. Use :session new to create one.".to_string();
                }
            }
            "task" => {
                if let Some(task_id) = args {
                    self.commands_executed
                        .entry("task".to_string())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);

                    return format!("🔍 Task Details for ID: {}", task_id);
                } else {
                    return "❌ Error: No task ID provided. Use :task TASK_ID".to_string();
                }
            }
            "artifacts" => {
                if let Some(task_id) = args {
                    self.commands_executed
                        .entry("artifacts".to_string())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);

                    return format!("📦 Artifacts for Task {}: (none)", task_id);
                } else {
                    return "❌ Error: No task ID provided. Use :artifacts TASK_ID".to_string();
                }
            }
            "cancelTask" => {
                if let Some(task_id) = args {
                    self.commands_executed
                        .entry("cancelTask".to_string())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);

                    return format!("✅ Successfully canceled task {}", task_id);
                } else {
                    return "❌ Error: No task ID provided. Use :cancelTask TASK_ID".to_string();
                }
            }
            "file" => {
                if let Some(args_str) = args {
                    self.commands_executed
                        .entry("file".to_string())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);

                    let parts: Vec<&str> = args_str.splitn(2, ' ').collect();
                    if parts.len() < 2 {
                        return "❌ Error: Invalid command format. Use :file PATH MESSAGE"
                            .to_string();
                    }

                    let file_path = parts[0];
                    let message = parts[1];

                    return format!("📤 Sending message with file: {}", file_path);
                } else {
                    return "❌ Error: Invalid command format. Use :file PATH MESSAGE".to_string();
                }
            }
            "data" => {
                if let Some(args_str) = args {
                    self.commands_executed
                        .entry("data".to_string())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);

                    let parts: Vec<&str> = args_str.splitn(2, ' ').collect();
                    if parts.len() < 2 {
                        return "❌ Error: Invalid command format. Use :data JSON MESSAGE"
                            .to_string();
                    }

                    let json_str = parts[0];
                    let message = parts[1];

                    // In a real implementation, we'd validate the JSON here

                    return format!("📤 Sending message with JSON data: {}", json_str);
                } else {
                    return "❌ Error: Invalid command format. Use :data JSON MESSAGE".to_string();
                }
            }
            "tool" => {
                if let Some(tool_args) = args {
                    self.commands_executed
                        .entry("tool".to_string())
                        .and_modify(|count| *count += 1)
                        .or_insert(1);

                    let parts: Vec<&str> = tool_args.splitn(2, ' ').collect();
                    if parts.is_empty() {
                        return "❌ Error: No tool name provided. Use :tool TOOL_NAME [JSON_PARAMS]".to_string();
                    }

                    let tool_name = parts[0];
                    let params_str = if parts.len() > 1 { parts[1] } else { "{}" };

                    return format!("Executing tool '{}' with params: {}", tool_name, params_str);
                } else {
                    return "❌ Error: No tool name provided. Use :tool TOOL_NAME [JSON_PARAMS]"
                        .to_string();
                }
            }
            "message" => {
                self.commands_executed
                    .entry("message".to_string())
                    .and_modify(|count| *count += 1)
                    .or_insert(1);

                return format!("Processing message: {}", args.unwrap_or_default());
            }
            _ => {
                return format!("Unknown command: {}", command);
            }
        }

        "Command not fully implemented".to_string()
    }

    fn get_command_count(&self, command: &str) -> usize {
        *self.commands_executed.get(command).unwrap_or(&0)
    }
}

#[test]
fn test_repl_session_commands() {
    // Create a REPL command processor
    let mut processor = ReplCommandProcessor::new();

    // Test session new command
    let response = processor.process_command(":session new");
    assert!(response.contains("Created new session"));
    assert_eq!(processor.get_command_count("session_new"), 1);

    // Test session show command
    let response = processor.process_command(":session show");
    assert!(response.contains("Current session"));
    assert_eq!(processor.get_command_count("session_show"), 1);
}

#[test]
fn test_repl_task_commands() {
    // Create a REPL command processor
    let mut processor = ReplCommandProcessor::new();

    // Create a session first
    processor.process_command(":session new");

    // Test tasks command
    let response = processor.process_command(":tasks");
    assert!(response.contains("Tasks in Current Session"));
    assert_eq!(processor.get_command_count("tasks"), 1);

    // Test task command
    let task_id = Uuid::new_v4().to_string();
    let response = processor.process_command(&format!(":task {}", task_id));
    assert!(response.contains("Task Details"));
    assert!(response.contains(&task_id));
    assert_eq!(processor.get_command_count("task"), 1);

    // Test artifacts command
    let response = processor.process_command(&format!(":artifacts {}", task_id));
    assert!(response.contains("Artifacts for Task"));
    assert!(response.contains(&task_id));
    assert_eq!(processor.get_command_count("artifacts"), 1);

    // Test cancel task command
    let response = processor.process_command(&format!(":cancelTask {}", task_id));
    assert!(response.contains("Successfully canceled task"));
    assert!(response.contains(&task_id));
    assert_eq!(processor.get_command_count("cancelTask"), 1);
}

#[test]
fn test_repl_file_data_commands() {
    // Create a REPL command processor
    let mut processor = ReplCommandProcessor::new();

    // Test file command
    let response = processor.process_command(":file /path/to/file.txt Message with file");
    assert!(response.contains("Sending message with file"));
    assert!(response.contains("/path/to/file.txt"));
    assert_eq!(processor.get_command_count("file"), 1);

    // Test data command
    let response = processor.process_command(":data {\"name\":\"test\"} Message with data");
    assert!(response.contains("Sending message with JSON data"));
    assert!(response.contains("name") && response.contains("test"));
    assert_eq!(processor.get_command_count("data"), 1);
}

#[test]
fn test_repl_tool_command() {
    // Create a REPL command processor
    let mut processor = ReplCommandProcessor::new();

    // Test tool command with no parameters
    let response = processor.process_command(":tool list_agents");
    assert!(response.contains("Executing tool"));
    assert!(response.contains("list_agents"));
    assert_eq!(processor.get_command_count("tool"), 1);

    // Test tool command with parameters
    let response = processor.process_command(":tool list_agents {\"format\":\"simple\"}");
    assert!(response.contains("Executing tool"));
    assert!(response.contains("list_agents"));
    assert!(response.contains("simple"));
    assert_eq!(processor.get_command_count("tool"), 2);
}
